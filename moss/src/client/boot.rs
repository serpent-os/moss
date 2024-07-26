// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! Boot management integration in moss

use std::{
    fs, io,
    path::{Path, PathBuf},
    str::FromStr,
};

use blsforme::os_release::{self, OsRelease};
use fnmatch::Pattern;
use stone::payload::{layout, Layout};
use thiserror::{self, Error};

use crate::{package::Id, Installation};

#[derive(Debug, Error)]
pub enum Error {
    #[error("blsforme: {0}")]
    Blsforme(#[from] blsforme::Error),

    #[error("io: {0}")]
    IO(#[from] io::Error),

    #[error("os_release: {0}")]
    OsRelease(#[from] os_release::Error),

    #[error("incomplete kernel tree: {0}")]
    IncompleteKernel(String),
}

/// Simple mapping type for kernel discovery paths, retaining the layout reference
#[derive(Debug)]
struct KernelCandidate<'a> {
    path: PathBuf,
    _layout: &'a Layout,
}

impl<'a> AsRef<Path> for KernelCandidate<'a> {
    fn as_ref(&self) -> &Path {
        self.path.as_path()
    }
}

/// From a given set of input paths, produce a set of match pairs
/// NOTE: This only works for a *new* blit and doesn't retroactively
/// sync old kernels!
fn kernel_files_from_state<'a>(
    install: &Installation,
    layouts: &'a [(Id, Layout)],
    pattern: &'a Pattern,
) -> Vec<KernelCandidate<'a>> {
    let mut kernel_entries = vec![];

    for (_, path) in layouts.iter() {
        match &path.entry {
            layout::Entry::Regular(_, target) => {
                if pattern.matches(target).is_some() {
                    kernel_entries.push(KernelCandidate {
                        path: install.root.join("usr").join(target),
                        _layout: path,
                    });
                }
            }
            layout::Entry::Symlink(_, target) => {
                if pattern.matches(target).is_some() {
                    kernel_entries.push(KernelCandidate {
                        path: install.root.join("usr").join(target),
                        _layout: path,
                    });
                }
            }
            _ => {}
        }
    }

    kernel_entries
}

/// Find bootloader assets
fn boot_files_from_state<'a>(
    install: &Installation,
    layouts: &'a [(Id, Layout)],
    pattern: &'a Pattern,
) -> Vec<PathBuf> {
    let mut rets = vec![];

    for (_, path) in layouts.iter() {
        if let layout::Entry::Regular(_, target) = &path.entry {
            if pattern.matches(target).is_some() {
                rets.push(install.root.join("usr").join(target));
            }
        }
    }

    rets
}

pub fn synchronize(install: &Installation, layouts: &[(Id, Layout)]) -> Result<(), Error> {
    let root = install.root.clone();
    let is_native = root.to_string_lossy() == "/";
    // Create an appropriate configuration
    let config = blsforme::Configuration {
        root: if is_native {
            blsforme::Root::Native(root.clone())
        } else {
            blsforme::Root::Image(root.clone())
        },
        vfs: "/".into(),
    };

    let pattern = fnmatch::Pattern::from_str("lib/kernel/(version:*)/*").unwrap(); // TODO: Do not return Infallible?
    let systemd = fnmatch::Pattern::from_str("lib*/systemd/boot/efi/*.efi").unwrap(); // TODO: Do not return Infallible?
    let booty_bits = boot_files_from_state(install, layouts, &systemd);

    // No kernels? No bother.
    let kernels = kernel_files_from_state(install, layouts, &pattern);
    if kernels.is_empty() {
        return Ok(());
    }
    // no fun times
    if booty_bits.is_empty() {
        return Ok(());
    }

    // Read the os-release file we created
    let fp = fs::read_to_string(install.root.join("usr").join("lib").join("os-release"))?;
    let os_release = OsRelease::from_str(&fp)?;
    let schema = blsforme::Schema::Blsforme {
        os_release: &os_release,
    };
    let discovered = schema.discover_system_kernels(kernels.iter())?;

    // pipe all of our entries into blsforme
    let entries = discovered.iter().map(blsforme::Entry::new);

    // If we can't get a manager, find, but don't bomb. Its probably a topology failure.
    let manager = match blsforme::Manager::new(&config) {
        Ok(m) => m.with_entries(entries).with_bootloader_assets(booty_bits),
        Err(_) => return Ok(()),
    };

    // Only allow mounting pre-sync for a native run
    if is_native {
        let _mounts = manager.mount_partitions()?;
        manager.sync(&schema)?;
    } else {
        manager.sync(&schema)?;
    }

    Ok(())
}
