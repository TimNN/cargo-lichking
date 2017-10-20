use std::io;
use std::fs::File;

use cargo::core::Package;
use cargo::{human, Config, CargoResult};

use license::License;
use licensed::{Confidence, Licensed, LicenseText};
use options::Bundle;

pub fn run(root: Package, mut packages: Vec<Package>, config: &Config, variant: Bundle) -> CargoResult<()> {
    packages.sort_by_key(|package| package.name().to_owned());

    match variant {
        Bundle::Inline { file } => {
            if let Some(file) = file {
                inline(&root, packages, config, &mut File::open(file)?)?;
            } else {
                inline(&root, packages, config, &mut io::stdout())?;
            }
        }
    }

    Ok(())
}

fn inline(root: &Package, packages: Vec<Package>, config: &Config, mut out: &mut io::Write) -> CargoResult<()> {
    writeln!(out, "The {} package uses some third party libraries under their own license terms:", root.name())?;
    writeln!(out, "")?;
    for package in packages {
        inline_package(&package, config, out)?;
        writeln!(out, "")?;
    }
    Ok(())
}

fn inline_package(package: &Package, config: &Config, mut out: &mut io::Write) -> CargoResult<()> {
    let license = package.license();
    writeln!(out, " * {} under {}:", package.name(), license)?;
    writeln!(out, "")?;
    match license {
        License::Unspecified => {
            config.shell().error(format!("{} does not specify a license", package.name()))?;
        }
        License::Multiple(licenses) => {
            let mut first = true;
            for license in licenses {
                if first {
                    first = false;
                } else {
                    writeln!(out, "    ===============")?;
                }
                inline_license(package, &license, config, out)?;
                writeln!(out, "")?;
            }
        }
        license => {
            inline_license(package, &license, config, out)?;
        }
    }
    Ok(())
}

fn inline_license(package: &Package, license: &License, config: &Config, mut out: &mut io::Write) -> CargoResult<()> {
    let texts = package.license_text(license)?;
    let text = choose(package, license, texts, config)?;
    for line in text.text.lines() {
        write!(out, "    {}", line)?;
    }
    Ok(())
}

fn choose(package: &Package, license: &License, texts: Vec<LicenseText>, config: &Config) -> CargoResult<LicenseText> {
    let (mut confident, texts): (Vec<LicenseText>, Vec<LicenseText>) = texts.into_iter().partition(|text| text.confidence == Confidence::Confident);
    let (mut semi_confident, mut unconfident): (Vec<LicenseText>, Vec<LicenseText>) = texts.into_iter().partition(|text| text.confidence == Confidence::SemiConfident);

    if confident.len() == 1 {
        return Ok(confident.swap_remove(0));
    } else if confident.len() > 1 {
        config.shell().error(format!("{} has multiple candidates for license {}:", package.name(), license))?;
        for text in &confident {
            config.shell().error(format!("    {}", text.path.display()))?;
        }
        return Ok(confident.swap_remove(0));
    }

    if semi_confident.len() == 1 {
        config.shell().warn(format!("{} has only a low-confidence candidate for license {}:", package.name(), license))?;
        return Ok(semi_confident.swap_remove(0));
    } else if semi_confident.len() > 1 {
        config.shell().error(format!("{} has multiple low-confidence candidates for license {}:", package.name(), license))?;
        for text in &semi_confident {
            config.shell().error(format!("    {}", text.path.display()))?;
        }
        return Ok(semi_confident.swap_remove(0));
    }

    if unconfident.len() == 1 {
        config.shell().warn(format!("{} has only a very low-confidence candidate for license {}:", package.name(), license))?;
        return Ok(unconfident.swap_remove(0));
    } else if unconfident.len() > 1 {
        config.shell().error(format!("{} has multiple very low-confidence candidates for license {}:", package.name(), license))?;
        for text in &unconfident {
            config.shell().error(format!("    {}", text.path.display()))?;
        }
        return Ok(unconfident.swap_remove(0));
    }

    return Err(human(format!("{} has no candidate texts for license {}", package.name(), license)));
}
