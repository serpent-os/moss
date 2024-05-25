// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::io;

use clap::Parser;
use clap_complete::{generate, Generator, Shell};

#[derive(Debug, Parser)]
#[command(about = "Generate shell completions")]
pub struct Command {
    #[arg(help = "Shell to generate completions for")]
    shell: Shell,
}

pub fn handle(command: Command, cli: clap::Command) {
    let Command { shell } = command;

    let mut cmd = cli;
    completions(shell, &mut cmd);
}

fn completions<G: Generator>(gen: G, cmd: &mut clap::Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut io::stdout());
}
