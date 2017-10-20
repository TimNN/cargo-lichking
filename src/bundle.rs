use std::io::{self, Read as R};
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use cargo::{human, Config, CargoResult};
use cargo::core::{MultiShell, Package};
use strsim::{levenshtein, osa_distance};

use license::License;
use licensed::Licensed;
use options::Bundle;

#[derive(Debug, Eq, PartialEq)]
pub enum Confidence {
    Confident,
    SemiConfident,
    Unsure,
}

pub struct LicenseText {
    pub path: PathBuf,
    pub text: String,
    pub confidence: Confidence,
}

struct Context<'a, 'b> {
    root: Package,
    packages: &'a [Package],
    shell: &'b mut MultiShell,

    missing_license: bool,
    low_quality_license: bool,
}

pub fn run(root: Package, mut packages: Vec<Package>, config: &Config, variant: Bundle) -> CargoResult<()> {
    packages.sort_by_key(|package| package.name().to_owned());

    let mut context = Context {
        root: root,
        packages: &packages,
        shell: &mut config.shell(),
        missing_license: false,
        low_quality_license: false,
    };

    match variant {
        Bundle::Inline { file } => {
            if let Some(file) = file {
                inline(&mut context, &mut File::open(file)?)?;
            } else {
                inline(&mut context, &mut io::stdout())?;
            }
        }
    }

    if context.missing_license {
        context.shell.error("\
  Our liches failed to recognise a license in one or more packages.

  We would be very grateful if you could check the corresponding package
  directories (see the package specific message above) to see if there is an
  easily recognisable license file available.

  If there is please submit details to
      https://github.com/Nemo157/cargo-lichking/issues
  so we can make sure this license is recognised in the future.

  If there isn't you could submit an issue to the package's project asking
  them to include the text of their license in the built packages.")?;
    }

    if context.low_quality_license {
        context.shell.error("\
  Our liches are very unsure about one or more licenses that were put into the \
  bundle. Please check the specific error messages above.")?;
    }

    if context.missing_license || context.low_quality_license {
        Err(human("Generating bundle finished with error(s)"))
    } else {
        Ok(())
    }
}

fn inline(context: &mut Context, mut out: &mut io::Write) -> CargoResult<()> {
    writeln!(out, "The {} package uses some third party libraries under their own license terms:", context.root.name())?;
    writeln!(out, "")?;
    for package in context.packages {
        inline_package(context, package, out)?;
        writeln!(out, "")?;
    }
    Ok(())
}

fn inline_package(context: &mut Context, package: &Package, mut out: &mut io::Write) -> CargoResult<()> {
    let license = package.license();
    writeln!(out, " * {} under {}:", package.name(), license)?;
    writeln!(out, "")?;
    if let Some(text) = find_generic_license_text(package, &license)? {
        match text.confidence {
            Confidence::Confident => (),
            Confidence::SemiConfident => {
                context.shell.warn(format_args!("{} has only a low-confidence candidate for license {}:", package.name(), license))?;
                context.shell.warn(format_args!("    {}", text.path.display()))?;
            }
            Confidence::Unsure => {
                context.shell.error(format_args!("{} has only a very low-confidence candidate for license {}:", package.name(), license))?;
                context.shell.error(format_args!("    {}", text.path.display()))?;
            }
        }
        for line in text.text.lines() {
            writeln!(out, "    {}", line)?;
        }
    } else {
        match license {
            License::Unspecified => {
                context.shell.error(format_args!("{} does not specify a license", package.name()))?;
            }
            License::Multiple(licenses) => {
                let mut first = true;
                for license in licenses {
                    if first {
                        first = false;
                    } else {
                        writeln!(out, "")?;
                        writeln!(out, "    ===============")?;
                        writeln!(out, "")?;
                    }
                    inline_license(context, package, &license, out)?;
                }
            }
            license => {
                inline_license(context, package, &license, out)?;
            }
        }
    }
    writeln!(out, "")?;
    Ok(())
}

fn inline_license(context: &mut Context, package: &Package, license: &License, mut out: &mut io::Write) -> CargoResult<()> {
    let texts = find_license_text(package, license)?;
    if let Some(text) = choose(context, package, license, texts)? {
        for line in text.text.lines() {
            writeln!(out, "    {}", line)?;
        }
    }
    Ok(())
}

fn choose(context: &mut Context, package: &Package, license: &License, texts: Vec<LicenseText>) -> CargoResult<Option<LicenseText>> {
    let (mut confident, texts): (Vec<LicenseText>, Vec<LicenseText>) = texts.into_iter().partition(|text| text.confidence == Confidence::Confident);
    let (mut semi_confident, mut unconfident): (Vec<LicenseText>, Vec<LicenseText>) = texts.into_iter().partition(|text| text.confidence == Confidence::SemiConfident);

    if confident.len() == 1 {
        return Ok(Some(confident.swap_remove(0)));
    } else if confident.len() > 1 {
        context.shell.error(format_args!("{} has multiple candidates for license {}:", package.name(), license))?;
        for text in &confident {
            context.shell.error(format_args!("    {}", text.path.display()))?;
        }
        return Ok(Some(confident.swap_remove(0)));
    }

    if semi_confident.len() == 1 {
        context.shell.warn(format_args!("{} has only a low-confidence candidate for license {}:", package.name(), license))?;
        context.shell.warn(format_args!("    {}", semi_confident[0].path.display()))?;
        return Ok(Some(semi_confident.swap_remove(0)));
    } else if semi_confident.len() > 1 {
        context.low_quality_license = true;
        context.shell.error(format_args!("{} has multiple low-confidence candidates for license {}:", package.name(), license))?;
        for text in &semi_confident {
            context.shell.error(format_args!("    {}", text.path.display()))?;
        }
        return Ok(Some(semi_confident.swap_remove(0)));
    }

    if unconfident.len() == 1 {
        context.low_quality_license = true;
        context.shell.warn(format_args!("{} has only a very low-confidence candidate for license {}:", package.name(), license))?;
        context.shell.warn(format_args!("    {}", unconfident[0].path.display()))?;
        return Ok(Some(unconfident.swap_remove(0)));
    } else if unconfident.len() > 1 {
        context.low_quality_license = true;
        context.shell.error(format_args!("{} has multiple very low-confidence candidates for license {}:", package.name(), license))?;
        for text in &unconfident {
            context.shell.error(format_args!("    {}", text.path.display()))?;
        }
        return Ok(Some(unconfident.swap_remove(0)));
    }

    context.shell.error(format_args!("{} has no candidate texts for license {} in {}", package.name(), license, package.root().display()))?;
    context.missing_license = true;
    return Ok(None);
}

fn read(path: &Path) -> CargoResult<String> {
    let mut s = String::new();
    File::open(path)?.read_to_string(&mut s)?;
    Ok(s)
}

// TODO: Choose something better
const MAX_LEVENSHTEIN_RATIO: f32 = 0.1;

fn normalize(text: &str) -> String {
    text.replace("\r", " ").replace("\n", " ").replace("  ", " ").to_uppercase()
}

fn check_against_template(text: &str, license: &License) -> bool {
    let text = normalize(text);
    if let License::Multiple(ref licenses) = *license {
        for license in licenses {
            if let Some(template) = license.template() {
                let template = normalize(template);
                let offset = osa_distance(&text, &template);
                let subtext = &text[offset..(offset + template.len())];
                let score = levenshtein(subtext, &template);
                println!("score {} / {}", score, template.len());
                if (score as f32) / (template.len() as f32) > MAX_LEVENSHTEIN_RATIO {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    } else {
        if let Some(template) = license.template() {
            let template = normalize(&template);
            let score = levenshtein(&text, &template);
            println!("score {} / {}", score, template.len());
            (score as f32) / (template.len() as f32) < MAX_LEVENSHTEIN_RATIO
        } else {
            false
        }
    }
}

fn find_generic_license_text(package: &Package, license: &License) -> CargoResult<Option<LicenseText>> {
    fn generic_license_name(name: &str) -> bool {
        name.to_uppercase() == "LICENSE"
            || name.to_uppercase() == "LICENSE.MD"
            || name.to_uppercase() == "LICENSE.TXT"
    }

    for entry in fs::read_dir(package.root())? {
        let entry = entry?;
        let path = entry.path().to_owned();
        let name = entry.file_name().to_string_lossy().into_owned();

        if generic_license_name(&name) {
            if let Ok(text) = read(&path) {
                println!("checking {} against {}", path.display(), license);
                let matches = check_against_template(&text, license);
                return Ok(Some(LicenseText {
                    path: path,
                    text: text,
                    confidence: if matches {
                        Confidence::Confident
                    } else {
                        Confidence::Unsure
                    },
                }));
            }
        }
    }

    Ok(None)
}

fn find_license_text(package: &Package, license: &License) -> CargoResult<Vec<LicenseText>> {
    fn read(path: &Path) -> CargoResult<String> {
        let mut s = String::new();
        File::open(path)?.read_to_string(&mut s)?;
        Ok(s)
    }

    fn name_matches(name: &str, license: &License) -> bool {
        match *license {
            License::MIT => name == "LICENSE-MIT",
            License::Apache_2_0 => name == "LICENSE-APACHE",
            License::Custom(ref custom) => {
                name.to_uppercase() == custom.to_uppercase() || name.to_uppercase() == format!("LICENSE-{}", custom.to_uppercase())
            }
            _ => false,
        }
    }

    let mut texts = Vec::new();
    for entry in fs::read_dir(package.root())? {
        let entry = entry?;
        let path = entry.path().to_owned();
        let name = entry.file_name().to_string_lossy().into_owned();

        if name_matches(&name, license) {
            if let Ok(text) = read(&path) {
                println!("checking {} against {}", path.display(), license);
                let matches = check_against_template(&text, license);
                texts.push(LicenseText {
                    path: path,
                    text: text,
                    confidence: if matches {
                        Confidence::Confident
                    } else {
                        Confidence::SemiConfident
                    },
                });
            }
        }
    }

    Ok(texts)
}