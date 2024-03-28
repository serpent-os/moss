// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::BTreeMap;

use clap::builder::NonEmptyStringValueParser;
use clap::{Arg, ArgMatches, Command};

use moss::db::meta;
use moss::{client, db};
use moss::{environment, Client, Installation};

const ARG_KEYWORD: &str = "KEYWORD";
const FLAG_INSTALLED: &str = "installed";

/// Returns the Clap struct for this command.
pub fn command() -> Command {
    Command::new("search")
        .visible_alias("sr")
        .about("Search packages")
        .long_about("Search packages by looking into package names and summaries.")
        .arg(
            Arg::new(ARG_KEYWORD)
                .required(true)
                .num_args(1..)
                .value_parser(NonEmptyStringValueParser::new()),
        )
        .arg(
            Arg::new(FLAG_INSTALLED)
                .short('i')
                .long(FLAG_INSTALLED)
                .num_args(0)
                .help("Search among installed packages only"),
        )
}

pub fn handle(args: &ArgMatches, installation: Installation) -> Result<(), Error> {
    let keywords = args.get_many::<String>(ARG_KEYWORD).unwrap();
    let only_installed: bool = *args.get_one(FLAG_INSTALLED).unwrap_or(&false);

    let client = Client::new(environment::NAME, installation)?;
    let mut package_info = BTreeMap::new();
    for keyword in keywords {
        let results = client.install_db.query(Some(meta::Filter::Name {
            keyword: keyword.to_string().into(),
            exact: false,
        }))?;
        for (id, meta) in results {
            package_info.insert(id, meta);
        }
    }
    for (_, meta) in package_info {
        println!("{}\t\t\t{}", meta.name, meta.summary);
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("client")]
    Client(#[from] client::Error),

    #[error("database")]
    Database(#[from] db::Error),
}
