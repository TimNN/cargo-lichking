use std::str::FromStr;

use clap::{ App, Arg, SubCommand, AppSettings, ArgMatches };

pub enum By {
    License,
    Crate,
}

pub enum Bundle {
    Inline {
        file: Option<String>,
    }
}

pub enum Cmd {
    List {
        by: By
    },
    Check,
    Bundle {
        variant: Bundle,
    },
}

pub struct Options {
    pub verbose: u32,
    pub quiet: bool,
    pub manifest_path: Option<String>,
    pub color: Option<String>,
    pub frozen: bool,
    pub locked: bool,
    pub cmd: Cmd,
}

impl Bundle {
    pub fn args() -> Vec<Arg<'static, 'static>> {
        vec![
            Arg::with_name("variant")
                .long("variant")
                .takes_value(true)
                .possible_value("inline")
                .default_value("inline")
                .help("What sort of bundle to produce"),
            Arg::with_name("file")
                .long("file")
                .takes_value(true).value_name("FILE")
                .help("The file to output to (standard out if not specified)"),
        ]
    }

    pub fn from_matches(matches: &ArgMatches) -> Bundle {
        match matches.value_of("variant").expect("defaulted") {
            "inline" => Bundle::Inline {
                file: matches.value_of("file").map(ToOwned::to_owned),
            },
            _ => unreachable!(),
        }
    }
}

impl Options {
    pub fn app(subcommand_required: bool) -> App<'static, 'static> {
        App::new("cargo")
            .bin_name("cargo")
            .subcommand(Options::subapp(subcommand_required))
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .global_settings(&[
                AppSettings::ColorAuto,
                AppSettings::ColoredHelp,
                AppSettings::VersionlessSubcommands,
                AppSettings::DeriveDisplayOrder,
                AppSettings::UnifiedHelpMessage,
            ])
    }

    // For some reason setting SubcommandRequired on the "lichking" sub command
    // propogates down to its subcommands as well, need to work out what's
    // happening and open a clap ticket so this argument is not needed.
    //
    // For now, try parsing the args without the subcommand being required,
    // then if we don't get a subcommand re-parse with it required to get the
    // error output.
    pub fn subapp(subcommand_required: bool) -> App<'static, 'static> {
        let mut app = SubCommand::with_name("lichking")
            .author(crate_authors!())
            .version(crate_version!())
            .about(crate_description!())
            .args(&Options::args())
            .subcommands(Options::subcommands());
        if subcommand_required {
            app = app.setting(AppSettings::SubcommandRequiredElseHelp);
        }
        app
    }

    pub fn args() -> Vec<Arg<'static, 'static>> {
        vec![
            Arg::with_name("verbose")
                .short("v").long("verbose")
                .multiple(true)
                .help("Use verbose output (-vv very verbose output)"),
            Arg::with_name("quiet")
                .short("q").long("quiet")
                .help("Use quiet output"),
            Arg::with_name("manifest-path")
                .long("manifest-path")
                .takes_value(true).value_name("PATH")
                .help("Path to the manifest to analyze"),
            Arg::with_name("color")
                .long("color")
                .takes_value(true).value_name("COLOR")
                .possible_values(&["auto", "always", "never"])
                .help("Coloring"),
            Arg::with_name("frozen")
                .long("frozen")
                .help("Require Cargo.lock and cache are up to date"),
            Arg::with_name("locked")
                .long("locked")
                .help("Require Cargo.lock is up to date"),
        ]
    }

    pub fn subcommands() -> Vec<App<'static, 'static>> {
        vec![
            SubCommand::with_name("check")
                .about("Check that all dependencies have a compatible license with this crate"),
            SubCommand::with_name("list")
                .about("List licensing of all dependencies")
                .args(&[
                    Arg::with_name("by")
                        .long("by")
                        .takes_value(true)
                        .possible_values(&["license", "crate"])
                        .default_value("license")
                        .help("Whether to list crates per license or licenses per crate")
                ]),
            SubCommand::with_name("bundle")
                .about("Bundle all dependencies licenses ready for distribution")
                .args(&Bundle::args()),
        ]
    }

    pub fn from_matches(matches: &ArgMatches) -> Options {
        let matches = matches.subcommand_matches("lichking").expect("required");
        Options {
            verbose: matches.occurrences_of("verbose") as u32,
            quiet: matches.is_present("quiet"),
            manifest_path: matches.value_of("manifest-path").map(ToOwned::to_owned),
            color: matches.value_of("color").map(ToOwned::to_owned),
            frozen: matches.is_present("frozen"),
            locked: matches.is_present("locked"),
            cmd: match matches.subcommand() {
                ("check", Some(_)) => Cmd::Check,
                ("list", Some(matches)) => {
                    Cmd::List {
                        by: matches.value_of("by")
                            .expect("defaulted")
                            .parse()
                            .expect("constrained"),
                    }
                }
                ("bundle", Some(matches)) => {
                    Cmd::Bundle {
                        variant: Bundle::from_matches(matches),
                    }
                }
                (_, _) => {
                    Options::app(true).get_matches();
                    unreachable!()
                }
            },
        }
    }
}

impl FromStr for By {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "license" => Ok(By::License),
            "crate" => Ok(By::Crate),
            s => Err(format!("Cannot parse By from '{}'", s)),
        }
    }
}
