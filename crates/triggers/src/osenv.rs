// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::os::linux::fs::MetadataExt;
use std::path::Path;
use std::{fs, io};

use serde::Deserialize;

/// The root directory as seen by the init process.
const ROOT_FILE: &str = "proc/1/root";

/// A canary file we create in live images.
const LIVE_FILE: &str = "run/livedev";

/// Describes a special status the operating system may be in.
#[derive(Debug, Deserialize)]
pub enum OsEnv {
    /// The operating system is inside a container.
    Container,

    /// The operating system in running as a live
    /// (and ephemeral) image.
    Live,
}

impl OsEnv {
    /// Detects the environment of the operating system
    /// by analyzing the root directory content ("/").
    pub fn detect() -> io::Result<Option<Self>> {
        Self::detect_from_root(Path::new("/"))
    }

    /// Detects the environment of the operating system
    /// by analyzing a sysroot directory.
    pub fn detect_from_root(root: &Path) -> io::Result<Option<Self>> {
        if Self::is_container(root)? {
            return Ok(Some(Self::Container));
        }
        if Self::is_live(root)? {
            return Ok(Some(Self::Live));
        }
        Ok(None)
    }

    fn is_container(root: &Path) -> io::Result<bool> {
        // The logic above is heuristic and I'm not sure
        // it works in all cases, particularly when containers
        // are designed to be transparent.
        // Anyway, the principle is to check that the "real" root
        // directory and the root seen by the init process are the same.
        let proc_root = fs::metadata(root)?;
        let proc_meta = fs::metadata(root.join(ROOT_FILE))?;
        if proc_root.st_dev() != proc_meta.st_dev() {
            return Ok(true);
        }
        if proc_root.st_ino() != proc_meta.st_ino() {
            return Ok(true);
        }
        Ok(false)
    }

    fn is_live(root: &Path) -> io::Result<bool> {
        root.join(LIVE_FILE).try_exists()
    }
}
