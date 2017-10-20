use std::io::{self, Read as R};
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use cargo::{human, Config, CargoResult};
use cargo::core::{MultiShell, Package};

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
}

pub fn run(root: Package, mut packages: Vec<Package>, config: &Config, variant: Bundle) -> CargoResult<()> {
    packages.sort_by_key(|package| package.name().to_owned());

    let mut context = Context {
        root: root,
        packages: &packages,
        shell: &mut config.shell(),
        missing_license: false,
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

    if context.missing_license {
        Err(human("Generating bundle failed"))
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
                    writeln!(out, "    ===============")?;
                    writeln!(out, "")?;
                }
                inline_license(context, package, &license, out)?;
                writeln!(out, "")?;
            }
        }
        license => {
            inline_license(context, package, &license, out)?;
        }
    }
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
            context.shell.error(format_args!("    {}", text.path.strip_prefix(package.root()).unwrap().display()))?;
        }
        return Ok(Some(confident.swap_remove(0)));
    }

    if semi_confident.len() == 1 {
        context.shell.warn(format_args!("{} has only a low-confidence candidate for license {}:", package.name(), license))?;
        context.shell.warn(format_args!("    {}", semi_confident[0].path.strip_prefix(package.root()).unwrap().display()))?;
        return Ok(Some(semi_confident.swap_remove(0)));
    } else if semi_confident.len() > 1 {
        context.shell.error(format_args!("{} has multiple low-confidence candidates for license {}:", package.name(), license))?;
        for text in &semi_confident {
            context.shell.error(format_args!("    {}", text.path.strip_prefix(package.root()).unwrap().display()))?;
        }
        return Ok(Some(semi_confident.swap_remove(0)));
    }

    if unconfident.len() == 1 {
        context.shell.warn(format_args!("{} has only a very low-confidence candidate for license {}:", package.name(), license))?;
        context.shell.warn(format_args!("    {}", unconfident[0].path.strip_prefix(package.root()).unwrap().display()))?;
        return Ok(Some(unconfident.swap_remove(0)));
    } else if unconfident.len() > 1 {
        context.shell.error(format_args!("{} has multiple very low-confidence candidates for license {}:", package.name(), license))?;
        for text in &unconfident {
            context.shell.error(format_args!("    {}", text.path.strip_prefix(package.root()).unwrap().display()))?;
        }
        return Ok(Some(unconfident.swap_remove(0)));
    }

    context.shell.error(format_args!("{} has no candidate texts for license {} in {}", package.name(), license, package.root().display()))?;
    context.missing_license = true;
    return Ok(None);
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

    fn generic_license_name(name: &str) -> bool {
        name.to_uppercase() == "LICENSE" || name.to_uppercase() == "LICENSE.MD"
    }

    let mut texts = Vec::new();
    for entry in fs::read_dir(package.root())? {
        let entry = entry?;
        let path = entry.path().to_owned();
        let name = entry.file_name().to_string_lossy().into_owned();

        if name_matches(&name, license) {
            if let Ok(text) = read(&path) {
                texts.push(LicenseText {
                    path: path,
                    text: text,
                    confidence: Confidence::SemiConfident,
                });
            }
        } else if generic_license_name(&name) {
            if let Ok(text) = read(&path) {
                texts.push(LicenseText {
                    path: path,
                    text: text,
                    confidence: Confidence::Unsure,
                });
            }
        }
    }

    Ok(texts)
}
