// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

//! System trigger management facilities

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fs;
use std::io;
use std::os::linux::fs::MetadataExt;
use std::path::{self, PathBuf};
use std::process;

use fnmatch::Match;
use serde::{de, Deserialize, Deserializer};
use thiserror::Error;

mod format;
pub mod iterpaths;

/// Collection of thematic operations to perform on certain file paths.
#[derive(Debug, Deserialize)]
#[serde(remote = "Self")]
pub struct Trigger {
    /// **Unique** identifier of this trigger.
    pub name: String,

    /// Optional trigger name that must be run before this trigger.
    /// This helps to build a dependency chain of triggers.
    pub before: Option<String>,

    /// Optional trigger name that must be run after this trigger.
    /// This helps to build a dependency chain of triggers.
    pub after: Option<String>,

    /// Optional inhibitors that prevent this trigger to run
    /// under certain conditions.
    #[serde(default, deserialize_with = "format::deserialize_inhibitors")]
    pub inhibitors: Vec<Inhibitor>,

    /// File path patterns involved with this trigger.
    /// Each pattern is associated to a list of handler names
    /// that will perform operations on, or with, it.
    ///
    /// Use [`Self::handlers_by_pattern`] to resolve handler names and get
    /// a list of [`Handler`]s for each [`Pattern`].
    #[serde(rename = "paths", deserialize_with = "format::deserialize_patterns")]
    pub patterns: BTreeMap<Pattern, Vec<String>>,

    /// Name-value pairs of handlers composing this trigger.
    pub handlers: BTreeMap<String, Handler>,
}

impl<'de> Deserialize<'de> for Trigger {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let trigger = Self::deserialize(deserializer)?;
        for handler_name in trigger.patterns.values().flatten() {
            if !trigger.handlers.contains_key(handler_name) {
                return Err(de::Error::custom(Error::MissingHandler(
                    trigger.name,
                    handler_name.to_string(),
                )));
            }
        }
        Ok(trigger)
    }
}

impl PartialEq for Trigger {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Trigger {}

impl PartialOrd for Trigger {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.name.cmp(&other.name))
    }
}

impl Ord for Trigger {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}

impl Trigger {
    /// Returns all handlers that compiled against the path.
    pub fn compiled_handlers(&self, path: String) -> impl Iterator<Item = CompiledHandler> + '_ {
        self.handlers_by_pattern()
            .flat_map(move |pat| pat.compiled(&path).collect::<Vec<_>>())
    }

    /// Returns all [`Handler`]s associated to each [`Pattern`].
    pub fn handlers_by_pattern(&self) -> impl Iterator<Item = PatternedHandlers<'_>> {
        self.patterns.iter().map(move |(pattern, handler_names)| {
            let handlers = handler_names
                .iter()
                .filter_map(|name| self.handlers.get(name))
                .collect();
            PatternedHandlers { pattern, handlers }
        })
    }

    /// Returns whether the trigger shouldn't be run because
    /// an inhibitor applies for this system.
    pub fn is_inhibited(&self) -> io::Result<bool> {
        for inhibitor in &self.inhibitors {
            if inhibitor.is_effective()? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// A condition that prevents a Trigger from running.
#[derive(Debug, PartialEq, Eq)]
pub enum Inhibitor {
    /// A file path. If this path exists, the Trigger shall not run.
    Path(PathBuf),

    /// An operating system environment.
    /// If the OS in inside this environment, the Trigger shall not run.
    Environment(OsEnv),
}

impl Inhibitor {
    /// Returns whether the inhibitor applies for this system.
    pub fn is_effective(&self) -> io::Result<bool> {
        match self {
            Self::Path(path) => path.try_exists(),
            Self::Environment(env) => Ok(OsEnv::detect()?.is_some_and(|detected| &detected == env)),
        }
    }
}

/// The operating system environment as seen by a Trigger.
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OsEnv {
    /// Indicates that the OS is a guest being run inside a container.
    #[serde(alias = "chroot")]
    Container,
    /// Indicates that the OS is a live image.
    Live,
}

impl OsEnv {
    /// Detects the environment of the operating system
    /// by analyzing the root directory content ("/").
    pub fn detect() -> io::Result<Option<Self>> {
        Self::detect_from_sysroot(path::Path::new("/"))
    }

    /// Detects the environment of the operating system
    /// by analyzing a sysroot directory.
    pub fn detect_from_sysroot(root: &path::Path) -> io::Result<Option<Self>> {
        if Self::is_container(root)? {
            return Ok(Some(Self::Container));
        }
        if Self::is_live(root)? {
            return Ok(Some(Self::Live));
        }
        Ok(None)
    }

    fn is_container(root: &path::Path) -> io::Result<bool> {
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

    fn is_live(root: &path::Path) -> io::Result<bool> {
        root.join(LIVE_FILE).try_exists()
    }
}

/// An extension of [`fnmatch::Pattern`] that takes into consideration
/// the file type too, not just the file name.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pattern {
    /// The file name pattern.
    pub pattern: fnmatch::Pattern,

    /// The file type. If None, any kind is considered valid.
    pub kind: Option<FileKind>,
}

impl Pattern {
    /// Returns whether a path matches this pattern.
    pub fn matches(&self, fspath: impl AsRef<str>) -> Option<Match> {
        self.pattern.matches(fspath).filter(|matc| {
            if let Some(kind) = &self.kind {
                let p = path::Path::new(&matc.path);
                match kind {
                    FileKind::Directory => p.is_dir(),
                    FileKind::Symlink => p.is_symlink(),
                }
            } else {
                true
            }
        })
    }
}

/// Known file types.
#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    /// A directory.
    Directory,

    /// A symbolic link to another file.
    Symlink,
}

/// An operation to perform. One or more operations compose a Trigger.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(untagged)]
pub enum Handler {
    /// Executes a process with an optional list of arguments.
    ///
    /// Arguments may contain the special syntax "`$(variableName)`"
    /// that will be resolved into the corresponding group name contained in a [`Match`].
    Run { run: String, args: Vec<String> },

    /// Removes a list of files (non-recursively).
    Delete { delete: Vec<String> },
}

impl fmt::Display for Handler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Handler::Run { run, args } => write!(f, "command \"{} {}\"", run, args.join(" ")),
            Handler::Delete { delete } => write!(f, "deleting {}", delete.join("; ")),
        }
    }
}

impl Handler {
    /// Replaces variables used in this Handler with group names found by a Match.
    /// If any variable doesn't match a group name, the variable syntax string is retained as it is.
    pub fn compiled(&self, with_match: &fnmatch::Match) -> CompiledHandler {
        match self {
            Handler::Run { run, args } => {
                let mut run = run.clone();
                for (key, value) in &with_match.groups {
                    run = run.replace(&format!("$({key})"), value);
                }
                let args = args
                    .iter()
                    .map(|a| {
                        let mut a = a.clone();
                        for (key, value) in &with_match.groups {
                            a = a.replace(&format!("$({key})"), value);
                        }
                        a
                    })
                    .collect();
                CompiledHandler(Handler::Run { run, args })
            }
            Handler::Delete { delete } => CompiledHandler(Handler::Delete { delete: delete.clone() }),
        }
    }
}

/// A [`Handler`] with variables resolved.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CompiledHandler(Handler);

impl fmt::Display for CompiledHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.handler())
    }
}

impl CompiledHandler {
    /// Returns the underlying Handler with variables resolved.
    pub fn handler(&self) -> &Handler {
        &self.0
    }

    /// Executes the handler with `workdir` as the working directory.
    pub fn run(&self, workdir: &path::Path) -> io::Result<process::Output> {
        match self.handler() {
            Handler::Run { run, args } => process::Command::new(run).args(args).current_dir(workdir).output(),
            Handler::Delete { delete } => {
                for file in delete {
                    fs::remove_file(file)?;
                }
                Ok(process::Output {
                    status: process::ExitStatus::default(),
                    stderr: Vec::default(),
                    stdout: Vec::default(),
                })
            }
        }
    }
}

/// A collection of handlers with a [`Pattern`] in common.
pub struct PatternedHandlers<'a> {
    pub pattern: &'a Pattern,
    pub handlers: Vec<&'a Handler>,
}

impl PatternedHandlers<'_> {
    /// Compiles all handlers in this collection against a given path.
    /// The list will be empty if [`Self::pattern`] doesn't match against the path.
    pub fn compiled(&self, fspath: impl AsRef<str>) -> impl Iterator<Item = CompiledHandler> + '_ {
        self.pattern
            .matches(fspath)
            .into_iter()
            .flat_map(|matc| self.handlers.iter().map(move |hnd| hnd.compiled(&matc)))
    }
}

#[derive(Default)]
/// Dependency graph of a pool of triggers.
pub struct DepGraph<'a> {
    graph: dag::Dag<&'a Trigger>,
}

impl<'a> DepGraph<'a> {
    /// Creates an empty dependency graph. Identical to [`Self::default()`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns an iterator over triggers in the right execution order.
    pub fn iter(&'a self) -> impl Iterator<Item = &'a Trigger> {
        self.graph.topo().copied()
    }
}

impl<'a> FromIterator<&'a Trigger> for DepGraph<'a> {
    fn from_iter<T: IntoIterator<Item = &'a Trigger>>(triggers: T) -> Self {
        let triggers: HashMap<&str, &Trigger> = triggers
            .into_iter()
            .map(|trigger| (trigger.name.as_str(), trigger))
            .collect();

        let mut graph = dag::Dag::with_capacity(triggers.len(), 0);
        for trigger in triggers.values() {
            let node = graph.add_node_or_get_index(*trigger);

            if let Some(before) = trigger
                .before
                .as_ref()
                .and_then(|before_name| triggers.get(before_name.as_str()))
                .map(|linked_trigger| graph.add_node_or_get_index(linked_trigger))
            {
                graph.add_edge(node, before);
            }
            if let Some(after) = trigger
                .after
                .as_ref()
                .and_then(|after_name| triggers.get(after_name.as_str()))
                .map(|linked_trigger| graph.add_node_or_get_index(linked_trigger))
            {
                graph.add_edge(after, node);
            }
        }

        Self { graph }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("missing handler reference in {0}: {1}")]
    MissingHandler(String, String),
}

/// The root directory as seen by the init process.
const ROOT_FILE: &str = "proc/1/root";

/// A canary file we create in live images.
const LIVE_FILE: &str = "run/livedev";

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::PathBuf};

    use crate::{FileKind, Handler, Inhibitor, OsEnv, Pattern, Trigger};

    #[test]
    fn deserialize_trigger() {
        let trigger: Trigger = serde_yaml::from_str(include_str!("../../../test/trigger_valid.yml")).unwrap();
        assert_eq!(trigger.name, "trigger".to_string());
        assert_eq!(trigger.before, Some("before_another_trigger".to_string()));
        assert_eq!(trigger.after, Some("after_another_trigger".to_string()));
        assert_eq!(
            trigger.inhibitors,
            Vec::from([
                Inhibitor::Path(PathBuf::from("/etc/file1")),
                Inhibitor::Path(PathBuf::from("/etc/file2")),
                Inhibitor::Environment(OsEnv::Container),
                Inhibitor::Environment(OsEnv::Live),
            ])
        );
        assert_eq!(
            trigger.patterns,
            BTreeMap::from([(
                Pattern {
                    pattern: fnmatch::Pattern::new("/usr/lib/modules/(version:*)/kernel"),
                    kind: Some(FileKind::Directory),
                },
                vec!["used_handler".to_string()]
            )])
        );
        assert_eq!(
            trigger.handlers,
            BTreeMap::from([
                (
                    "used_handler".to_string(),
                    Handler::Run {
                        run: "/usr/bin/used".to_string(),
                        args: vec!["used1".to_string(), "used2".to_string()]
                    }
                ),
                (
                    "unwanted_files".to_string(),
                    Handler::Delete {
                        delete: vec!["/".to_string()]
                    }
                )
            ])
        );
    }
}
