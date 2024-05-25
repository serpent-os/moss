// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use clap::{arg, ArgMatches, Command};
use itertools::Itertools;
use thiserror::Error;

use moss::{
    client::{self, Client},
    environment,
    package::Flags,
    Installation,
};
use tui::Styled;

pub fn command() -> Command {
    Command::new("list")
        .about("List packages")
        .long_about("List packages according to a filter")
        .subcommand_required(true)
        .subcommand(list_installed("installed".to_string(), false))
        .subcommand(list_available("available".to_string(), false))
        .subcommand(list_sync("sync".to_string(), false))
}

pub fn list_available(name: String, hide: bool) -> Command {
    Command::new(name.clone())
        .about("List all available packages")
        .hide(hide)
}

pub fn list_installed(name: String, hide: bool) -> Command {
    Command::new(name.clone())
        .about("List all installed packages")
        .arg(arg!(-e --"explicit" "List explicit packages only"))
        .hide(hide)
}

pub fn list_sync(name: String, hide: bool) -> Command {
    Command::new(name.clone())
        .about("List packages with sync changes")
        .arg(arg!(--"upgrade-only" "Only sync packages that have a version upgrade"))
        .hide(hide)
}

enum Sync {
    All,
    Upgrades,
}

/// Handle listing by filter
pub fn handle(args: &ArgMatches, subcommand: Option<&str>, installation: Installation) -> Result<(), Error> {
    let (filter_flags, sync) = match args.subcommand() {
        Some(("available", _)) => (Flags::new().with_available(), None),
        Some(("installed", args)) => {
            let flags = if *args.get_one::<bool>("explicit").unwrap() {
                Flags::new().with_installed().with_explicit()
            } else {
                Flags::new().with_installed()
            };
            (flags, None)
        }
        Some(("sync", args)) => {
            let sync = if *args.get_one::<bool>("upgrade-only").unwrap() {
                Sync::Upgrades
            } else {
                Sync::All
            };

            (Flags::new().with_installed(), Some(sync))
        }
        None => match subcommand {
            Some("available") => (Flags::new().with_available(), None),
            Some("installed") => {
                let flags = if *args.get_one::<bool>("explicit").unwrap() {
                    Flags::new().with_installed().with_explicit()
                } else {
                    Flags::new().with_installed()
                };
                (flags, None)
            }
            Some("sync") => {
                let sync = if *args.get_one::<bool>("upgrade-only").unwrap() {
                    Sync::Upgrades
                } else {
                    Sync::All
                };

                (Flags::new().with_installed(), Some(sync))
            }
            _ => unreachable!(),
        },
        _ => unreachable!(),
    };

    // Grab a client for the target, enumerate packages
    let client = Client::new(environment::NAME, installation)?;
    let pkgs = client.registry.list(filter_flags).collect::<Vec<_>>();

    let sync_available = if sync.is_some() {
        client.registry.list(Flags::new().with_available()).collect::<Vec<_>>()
    } else {
        vec![]
    };

    if pkgs.is_empty() {
        return Err(Error::NoneFound);
    }

    // map to renderable state
    let mut set = pkgs
        .into_iter()
        .map(|p| {
            let sync = sync_available
                .iter()
                // Get first (priority based)
                .find(|u| u.meta.name == p.meta.name)
                // Ensure it's an upgrade (if `upgrades-only`)
                // otherwise check if it's a change
                .filter(|u| {
                    if matches!(sync, Some(Sync::Upgrades)) {
                        u.meta.source_release > p.meta.source_release
                    } else {
                        u.meta.source_release != p.meta.source_release
                    }
                })
                .map(|u| Revision {
                    version: u.meta.version_identifier.clone(),
                    release: u.meta.source_release.to_string(),
                });

            Format {
                name: p.meta.name.to_string(),
                revision: Revision {
                    version: p.meta.version_identifier,
                    release: p.meta.source_release.to_string(),
                },
                summary: p.meta.summary,
                explicit: if filter_flags == Flags::new().with_installed() {
                    p.flags.explicit
                } else {
                    true
                },
                sync,
            }
        })
        .filter(|item| if sync.is_some() { item.sync.is_some() } else { true })
        .collect_vec();

    // Thanks to priorities, first in list is the winning candidate in list available.
    // Therefore sort by name and dedupe is safe as we mask the lower priority items out.
    set.sort_by_key(|s| s.name.clone());
    set.dedup_by_key(|s| s.name.clone());

    // Grab maximum length
    let max_length = set.iter().map(Format::size).max().unwrap_or_default();

    // render
    for item in set {
        let width = max_length - item.size() + 2;
        let name = if item.explicit {
            item.name.bold()
        } else {
            item.name.dim()
        };
        print!("{} {:width$} ", name, " ", width = width);

        let print_revision = |rev: Revision, is_sync| {
            let version = if is_sync {
                rev.version.green()
            } else {
                rev.version.magenta()
            };
            print!("{}-{}", version, rev.release.dim());
        };

        // Print revision
        print_revision(item.revision, false);

        // Print sync version
        if let Some(sync) = item.sync {
            print!(" => ");
            print_revision(sync, true);
        }

        println!(" - {}", item.summary);
    }

    Ok(())
}

#[derive(Debug)]
struct Format {
    name: String,
    summary: String,
    revision: Revision,
    explicit: bool,
    sync: Option<Revision>,
}

impl Format {
    fn size(&self) -> usize {
        self.name.len() + self.revision.size() + self.sync.as_ref().map(Revision::size).unwrap_or_default()
    }
}

#[derive(Debug)]
struct Revision {
    version: String,
    release: String,
}

impl Revision {
    fn size(&self) -> usize {
        self.version.len() + self.release.len()
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("No packages found")]
    NoneFound,
    #[error("client")]
    Client(#[from] client::Error),
}
