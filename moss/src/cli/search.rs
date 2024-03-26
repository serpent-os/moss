// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use clap::{arg, value_parser, ArgMatches, Command};

use moss::client;
use moss::{environment, Client, Installation};

/// Returns the Clap struct for this command.
pub fn command() -> Command {
    Command::new("search")
        .visible_alias("sr")
        .about("Search packages")
        .long_about("Search packages by looking into package names and summaries")
        .arg(arg!(<KEYWORD> ... "keywords").value_parser(value_parser!(String)))
        .arg(arg!(-i --installed "search among installed packages only"))
}

pub fn handle(args: &ArgMatches, installation: Installation) -> Result<(), Error> {
    let mut client = Client::new(environment::NAME, installation)?;
    client.install_db.query(filter)
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("client")]
    Client(#[from] client::Error),
}
