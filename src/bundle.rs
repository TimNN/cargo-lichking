use std::io;
use std::fs::File;

use cargo::core::Package;
use cargo::CargoResult;

use licensed::Licensed;
use options::Bundle;

pub fn run(root: Package, packages: Vec<Package>, variant: Bundle) -> CargoResult<()> {
    match variant {
        Bundle::Inline { file } => {
            if let Some(file) = file {
                inline(root, packages, &mut File::open(file)?)?;
            } else {
                inline(root, packages, &mut io::stdout())?;
            }
        }
    }

    Ok(())
}

fn inline(root: Package, mut packages: Vec<Package>, mut out: &mut io::Write) -> CargoResult<()> {
    packages.sort_by_key(|package| package.name().to_owned());

    writeln!(out, "The {} package uses some third party libraries under their own license terms:", root.name())?;
    writeln!(out, "")?;
    for package in packages {
        let license = package.license();
        writeln!(out, " * {} under {}:", package.name(), license)?;
        writeln!(out, "    TODO: {} license contents", license)?;
        writeln!(out, "")?;
    }
    Ok(())
}
