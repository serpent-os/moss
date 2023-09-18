// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::path::Path;

use clap::{arg, ArgMatches, Command};
use moss::{repository, Installation, Repository};
use thiserror::Error;
use url::Url;

/// Control flow for the subcommands
enum Action<'a> {
    // Root
    List(&'a Path),
    // Root, Id, Url
    Add(&'a Path, String, Url),
    // Root, Id
    Remove(&'a Path, String),
}

/// Return a command for handling `repo` subcommands
pub fn command() -> Command {
    Command::new("repo")
        .about("Manage software repositories")
        .long_about("Manage the available software repositories visible to the installed system")
        .subcommand_required(true)
        .subcommand(
            Command::new("add")
                .arg(arg!(<NAME> "repo name").value_parser(clap::value_parser!(String)))
                .arg(arg!(<URI> "repo uri").value_parser(clap::value_parser!(Url))),
        )
        .subcommand(
            Command::new("list")
                .about("List system software repositories")
                .long_about("List all of the system repositories and their status"),
        )
        .subcommand(
            Command::new("remove")
                .about("Remove a repository for the system")
                .arg(arg!(<NAME> "repo name").value_parser(clap::value_parser!(String))),
        )
}

/// Handle subcommands to `repo`
pub async fn handle(args: &ArgMatches, root: &Path) -> Result<(), Error> {
    let handler = match args.subcommand() {
        Some(("add", cmd_args)) => Action::Add(
            root,
            cmd_args.get_one::<String>("NAME").cloned().unwrap(),
            cmd_args.get_one::<Url>("URI").cloned().unwrap(),
        ),
        Some(("list", _)) => Action::List(root),
        Some(("remove", cmd_args)) => {
            Action::Remove(root, cmd_args.get_one::<String>("NAME").cloned().unwrap())
        }
        _ => unreachable!(),
    };

    // dispatch to runtime handler function
    match handler {
        Action::List(root) => list(root).await,
        Action::Add(root, name, uri) => add(root, name, uri).await,
        Action::Remove(_, _) => unimplemented!(),
    }
}

// Actual implementation of moss repo add, asynchronous
async fn add(root: &Path, name: String, uri: Url) -> Result<(), Error> {
    let installation = Installation::open(root);

    let mut manager = repository::Manager::new(installation).await?;

    manager
        .add_repository(
            repository::Id::new(name),
            Repository {
                description: "...".into(),
                uri,
                priority: 0,
            },
        )
        .await?;

    manager.refresh_all().await?;

    Ok(())
}

/// List the repositories and pretty print them
async fn list(root: &Path) -> Result<(), Error> {
    let installation = Installation::open(root);
    let manager = repository::Manager::new(installation).await?;

    let mut configured_repos = manager.list();
    if configured_repos.is_empty() {
        println!("No repositories have been configured yet");
        return Ok(());
    }

    configured_repos.sort_by_key(|(_, repo)| repo.priority);

    for (id, repo) in configured_repos {
        println!(" - {} = {} [{}]", id, repo.uri, repo.priority);
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("repo error: {0}")]
    RepositoryManager(#[from] repository::manager::Error),
}
