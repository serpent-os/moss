// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{env, path::PathBuf};

use clap::{Arg, ArgAction, Command};
use moss::{installation, runtime, Installation};
use thiserror::Error;

mod completions;
mod extract;
mod index;
mod info;
mod inspect;
mod install;
mod list;
mod remove;
mod repo;
mod search;
mod state;
mod sync;
mod version;

/// Generate the CLI command structure
fn command() -> Command {
    Command::new("moss")
        .about("Next generation package manager")
        .arg(
            Arg::new("version")
                .short('v')
                .long("version")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("root")
                .short('D')
                .long("directory")
                .global(true)
                .help("Root directory")
                .action(ArgAction::Set)
                .default_value("/")
                .value_parser(clap::value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("cache")
                .long("cache")
                .global(true)
                .help("Cache directory")
                .action(ArgAction::Set)
                .value_parser(clap::value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("yes")
                .short('y')
                .long("yes-all")
                .global(true)
                .help("Assume yes for all questions")
                .action(ArgAction::SetTrue),
        )
        .arg_required_else_help(true)
        .subcommand(completions::command())
        .subcommand(extract::command())
        .subcommand(index::command())
        .subcommand(info::command())
        .subcommand(inspect::command())
        .subcommand(install::command())
        .subcommand(list::command())
        .subcommand(list::list_available("la".to_string(), true))
        .subcommand(list::list_installed("li".to_string(), true))
        .subcommand(list::list_sync("ls".to_string(), true))
        .subcommand(list::list_sync("lu".to_string(), true))
        .subcommand(remove::command())
        .subcommand(repo::command())
        .subcommand(repo::repo_add("ar".to_string(), true))
        .subcommand(repo::repo_list("lr".to_string(), true))
        .subcommand(repo::repo_remove("rr".to_string(), true))
        .subcommand(repo::repo_update("ur".to_string(), true))
        .subcommand(search::command())
        .subcommand(state::command())
        .subcommand(sync::command())
        .subcommand(version::command())
}

/// Process all CLI arguments
pub fn process() -> Result<(), Error> {
    let matches = command().get_matches_from(env::args());

    if matches.get_flag("version") {
        version::print();
        return Ok(());
    }

    let root = matches.get_one::<PathBuf>("root").unwrap();
    let cache = matches.get_one::<PathBuf>("cache");

    // Make async runtime available to all of moss
    let _guard = runtime::init();

    let mut installation = Installation::open(root)?;
    if let Some(dir) = cache {
        installation = installation.with_cache_dir(dir)?;
    }

    match matches.subcommand() {
        Some(("completions", args)) => {
            completions::handle(args, command());
            Ok(())
        }
        Some(("extract", args)) => extract::handle(args).map_err(Error::Extract),
        Some(("index", args)) => index::handle(args).map_err(Error::Index),
        Some(("info", args)) => info::handle(args, installation).map_err(Error::Info),
        Some(("inspect", args)) => inspect::handle(args).map_err(Error::Inspect),
        Some(("install", args)) => install::handle(args, installation).map_err(Error::Install),
        Some(("list", args)) => list::handle(args, None, installation).map_err(Error::List),
        Some(("la", args)) => list::handle(args, Some("available"), installation).map_err(Error::List),
        Some(("li", args)) => list::handle(args, Some("installed"), installation).map_err(Error::List),
        Some(("ls", args)) => list::handle(args, Some("sync"), installation).map_err(Error::List),
        Some(("lu", args)) => list::handle(args, Some("sync"), installation).map_err(Error::List),
        Some(("remove", args)) => remove::handle(args, installation).map_err(Error::Remove),
        Some(("repo", args)) => repo::handle(args, None, installation).map_err(Error::Repo),
        Some(("ar", args)) => repo::handle(args, Some("add"), installation).map_err(Error::Repo),
        Some(("lr", args)) => repo::handle(args, Some("list"), installation).map_err(Error::Repo),
        Some(("rr", args)) => repo::handle(args, Some("remove"), installation).map_err(Error::Repo),
        Some(("ur", args)) => repo::handle(args, Some("update"), installation).map_err(Error::Repo),
        Some(("search", args)) => search::handle(args, installation).map_err(Error::Search),
        Some(("state", args)) => state::handle(args, installation).map_err(Error::State),
        Some(("sync", args)) => sync::handle(args, installation).map_err(Error::Sync),
        Some(("version", _)) => {
            version::print();
            Ok(())
        }
        None => {
            command().print_help().unwrap();
            Ok(())
        }
        _ => unreachable!(),
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("index")]
    Index(#[from] index::Error),

    #[error("info")]
    Info(#[from] info::Error),

    #[error("install")]
    Install(#[from] install::Error),

    #[error("list")]
    List(#[from] list::Error),

    #[error("inspect")]
    Inspect(#[from] inspect::Error),

    #[error("extract")]
    Extract(#[from] extract::Error),

    #[error("remove")]
    Remove(#[from] remove::Error),

    #[error("repo")]
    Repo(#[from] repo::Error),

    #[error("search")]
    Search(#[from] search::Error),

    #[error("state")]
    State(#[from] state::Error),

    #[error("sync")]
    Sync(#[from] sync::Error),

    #[error("installation")]
    Installation(#[from] installation::Error),
}
