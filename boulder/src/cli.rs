// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0
use std::path::PathBuf;

use boulder::{env, Env};
use clap::{Args, CommandFactory, Parser};
use thiserror::Error;

mod build;
mod chroot;
mod completions;
mod profile;
mod recipe;

#[derive(Debug, Parser)]
#[command(version = version())]
pub struct Command {
    #[command(flatten)]
    pub global: Global,
    #[command(subcommand)]
    pub subcommand: Subcommand,
}

#[derive(Debug, Args)]
pub struct Global {
    #[arg(long, global = true)]
    pub cache_dir: Option<PathBuf>,
    #[arg(long, global = true)]
    pub config_dir: Option<PathBuf>,
    #[arg(long, global = true)]
    pub data_dir: Option<PathBuf>,
    #[arg(long, global = true)]
    pub moss_root: Option<PathBuf>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    Build(build::Command),
    Chroot(chroot::Command),
    Completions(completions::Command),
    Profile(profile::Command),
    Recipe(recipe::Command),
}

pub fn process() -> Result<(), Error> {
    let args = replace_aliases(std::env::args());
    let Command { global, subcommand } = Command::parse_from(args);

    let env = Env::new(global.cache_dir, global.config_dir, global.data_dir, global.moss_root)?;

    match subcommand {
        Subcommand::Build(command) => build::handle(command, env)?,
        Subcommand::Chroot(command) => chroot::handle(command, env)?,
        Subcommand::Completions(command) => completions::handle(command, Command::command()),
        Subcommand::Profile(command) => profile::handle(command, env)?,
        Subcommand::Recipe(command) => recipe::handle(command, env)?,
    }

    Ok(())
}

fn replace_aliases(args: std::env::Args) -> Vec<String> {
    const ALIASES: &[(&str, &[&str])] = &[("new", &["recipe", "new"]), ("macros", &["recipe", "macros"])];

    let mut args = args.collect::<Vec<_>>();

    for (alias, replacements) in ALIASES {
        let Some(pos) = args.iter().position(|a| a == *alias) else {
            continue;
        };

        // Escape hatch for alias w/ same name as
        // inner subcommand
        if args.get(pos - 1).map(String::as_str) == replacements.first().copied() {
            continue;
        }

        args.splice(pos..pos + 1, replacements.iter().map(|arg| arg.to_string()));

        break;
    }

    args
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("build")]
    Build(#[from] build::Error),
    #[error("chroot")]
    Chroot(#[from] chroot::Error),
    #[error("profile")]
    Profile(#[from] profile::Error),
    #[error("env")]
    Env(#[from] env::Error),
    #[error("recipe")]
    Recipe(#[from] recipe::Error),
}

fn version() -> String {
    use moss::environment;

    pub const VERSION: &str = env!("CARGO_PKG_VERSION");

    let hash = environment::GIT_HASH
        .map(|hash| format!(" ({hash})"))
        .unwrap_or_default();

    format!("{VERSION}{hash}")
}
