// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use log::warn;

use crate::{db, package, Package, Provider, State};

// TODO:
#[derive(Debug, Clone)]
pub struct Active {
    state: Option<State>,
    db: crate::db::meta::Database,
}

impl PartialEq for Active {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state
    }
}

impl Eq for Active {}

impl Active {
    /// Return a new Active plugin for the given state + install database
    pub fn new(state: Option<State>, db: crate::db::meta::Database) -> Self {
        Self { state, db }
    }

    /// Query the given package
    pub fn package(&self, id: &package::Id) -> Option<Package> {
        match self.db.get(id) {
            Ok(meta) => self
                .installed_package(id.clone())
                .map(|(id, flags)| Package { id, meta, flags }),
            Err(db::meta::Error::RowNotFound) => None,
            Err(error) => {
                warn!("failed to query installed package: {error}");
                None
            }
        }
    }

    /// Query, restricted to state
    fn query(&self, flags: package::Flags, filter: Option<db::meta::Filter>) -> Vec<Package> {
        if flags.installed || flags == package::Flags::default() {
            // TODO: Error handling
            let packages = match self.db.query(filter) {
                Ok(packages) => packages,
                Err(error) => {
                    warn!("failed to query repository packages: {error}");
                    return vec![];
                }
            };

            packages
                .into_iter()
                .filter_map(|(id, meta)| {
                    self.installed_package(id)
                        .map(|(id, flags)| Package { id, meta, flags })
                })
                // Filter for explicit only packages, if applicable
                .filter(|package| if flags.explicit { package.flags.explicit } else { true })
                .collect()
        } else {
            vec![]
        }
    }

    /// List, restricted to state
    pub fn list(&self, flags: package::Flags) -> Vec<Package> {
        self.query(flags, None)
    }

    /// Query all packages that match the given provider identity
    pub fn query_provider(&self, provider: &Provider, flags: package::Flags) -> Vec<Package> {
        self.query(flags, Some(db::meta::Filter::Provider(provider.clone())))
    }

    /// Query matching by name
    pub fn query_name(&self, package_name: &package::Name, flags: package::Flags) -> Vec<Package> {
        self.query(
            flags,
            Some(db::meta::Filter::Name {
                keyword: package_name.clone(),
                exact: true,
            }),
        )
    }

    pub fn query_provider_id_only(&self, provider: &Provider, flags: package::Flags) -> Vec<package::Id> {
        if flags.installed || flags == package::Flags::default() {
            // TODO: Error handling
            let packages = match self.db.provider_packages(provider) {
                Ok(packages) => packages,
                Err(error) => {
                    warn!("failed to query repository packages: {error}");
                    return vec![];
                }
            };

            packages
                .into_iter()
                .filter_map(|id| self.installed_package(id))
                // Filter for explicit only packages, if applicable
                .filter_map(|(id, package_flags)| {
                    if flags.explicit {
                        package_flags.explicit.then_some(id)
                    } else {
                        Some(id)
                    }
                })
                .collect()
        } else {
            vec![]
        }
    }

    pub fn priority(&self) -> u64 {
        u64::MAX
    }

    fn installed_package(&self, id: package::Id) -> Option<(package::Id, package::Flags)> {
        match &self.state {
            Some(st) => st
                .selections
                .iter()
                .find(|selection| selection.package == id)
                .map(|selection| {
                    (
                        id,
                        if selection.explicit {
                            package::Flags::new().with_installed().with_explicit()
                        } else {
                            package::Flags::new().with_installed()
                        },
                    )
                }),
            None => None,
        }
    }
}
