// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Deserializer};

use crate::{FileKind, Inhibitor, OsEnv, Pattern};

/// Deserializes the "inhibitors" field of a [`Trigger`].
pub fn deserialize_inhibitors<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Inhibitor>, D::Error> {
    #[derive(Default, Deserialize)]
    struct Inhibitors {
        pub paths: Vec<PathBuf>,
        pub environment: Vec<OsEnv>,
    }

    let de = Inhibitors::deserialize(deserializer)?;
    let mut inhibitors = vec![];
    for path in de.paths {
        inhibitors.push(Inhibitor::Path(path));
    }
    for env in de.environment {
        inhibitors.push(Inhibitor::Environment(env));
    }
    Ok(inhibitors)
}

/// Deserializes the "paths" field of a [`Trigger`].
pub fn deserialize_patterns<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<Pattern, Vec<String>>, D::Error> {
    #[derive(Deserialize)]
    struct PathDefinition {
        pub handlers: Vec<String>,
        #[serde(rename = "type")]
        pub kind: Option<FileKind>,
    }

    let de = BTreeMap::<fnmatch::Pattern, PathDefinition>::deserialize(deserializer)?;
    let mut paths = BTreeMap::new();
    for (pattern, path_definition) in de {
        paths.insert(
            Pattern {
                kind: path_definition.kind,
                pattern,
            },
            path_definition.handlers,
        );
    }
    Ok(paths)
}
