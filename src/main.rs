#[macro_use] extern crate clap;
extern crate cargo;
extern crate void;
extern crate strsim;

mod bundle;
mod check;
mod license;
mod licensed;
mod list;
mod load;
mod options;

use std::process;

use cargo::{ Config, CliResult };

use options::{ Options, Cmd };

fn main() {
    let matches = Options::app(false).get_matches();
    let options = Options::from_matches(&matches);
    let config = Config::default().expect("No idea why this would fail");
    let result = real_main(options, &config);
    if let Err(err) = result {
        config.shell().error(err).expect("Can't do much");
        process::exit(1);
    }
}

fn real_main(options: Options, config: &Config) -> CliResult {
    config.configure(
        options.verbose,
        Some(options.quiet),
        &options.color,
        options.frozen,
        options.locked)?;

    config.shell().warn("IANAL: This is not legal advice and is not guaranteed to be correct.")?;

    let (root, packages) = load::resolve_packages(options.manifest_path, config)?;

    match options.cmd {
        Cmd::Check => check::run(root, packages, config)?,
        Cmd::List { by } => {
            list::run(packages, config, by)?
        }
        Cmd::Bundle { variant } => bundle::run(root, packages, config, variant)?,
    }

    Ok(())
}
