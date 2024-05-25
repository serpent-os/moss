// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io;

use clap::{arg, ArgMatches};
use clap_complete::{generate, Generator, Shell};

pub fn command() -> clap::Command {
    clap::Command::new("completions")
        .about("Generate shell completions")
        .arg(arg!(<SHELL> ... "Shell to generate completions for").value_parser(clap::value_parser!(Shell)))
}

pub fn handle(args: &ArgMatches, cli: clap::Command) {
    let shell = *args.get_one::<Shell>("SHELL").unwrap();

    let mut cmd = cli;
    completions(shell, &mut cmd);
}

fn completions<G: Generator>(gen: G, cmd: &mut clap::Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut io::stdout());
}
