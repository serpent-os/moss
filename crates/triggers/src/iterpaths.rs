// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! Utility functions that accept multiple file paths.

use std::collections::BTreeSet;

use crate::{CompiledHandler, Trigger};

pub fn compiled_handlers(trigger: &Trigger, paths: impl Iterator<Item = String>) -> BTreeSet<CompiledHandler> {
    paths.flat_map(|path| trigger.compiled_handlers(path)).collect()
}
