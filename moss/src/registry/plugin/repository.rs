// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use log::warn;

use crate::{
    db,
    package::{self, Package},
    repository, Provider,
};

#[derive(Debug)]
pub struct Repository {
    active: repository::Active,
}

impl Repository {
    pub fn new(active: repository::Active) -> Self {
        Self { active }
    }

    pub fn priority(&self) -> u64 {
        self.active.repository.priority.into()
    }

    pub fn package(&self, id: &package::Id) -> Option<Package> {
        let result = self.active.db.get(id);

        match result {
            Ok(meta) => Some(Package {
                id: id.clone(),
                meta: package::Meta {
                    // TODO: Is there a more type-safe way to do this vs mutation? Can
                    // a new type help here?
                    uri: meta
                        .uri
                        .and_then(|relative| self.active.repository.uri.join(&relative).ok())
                        .map(|url| url.to_string()),
                    ..meta
                },
                flags: package::Flags::new().with_available(),
            }),
            Err(db::meta::Error::RowNotFound) => None,
            Err(error) => {
                warn!("failed to query repository package: {error}");
                None
            }
        }
    }

    fn query(&self, flags: package::Flags, filter: Option<db::meta::Filter>) -> Vec<Package> {
        if flags.available || flags == package::Flags::default() {
            // TODO: Error handling
            let packages = match self.active.db.query(filter) {
                Ok(packages) => packages,
                Err(error) => {
                    warn!("failed to query repository packages: {error}");
                    return vec![];
                }
            };

            packages
                .into_iter()
                .map(|(id, meta)| Package {
                    id,
                    meta,
                    flags: package::Flags::new().with_available(),
                })
                .collect()
        } else {
            vec![]
        }
    }

    pub fn list(&self, flags: package::Flags) -> Vec<Package> {
        self.query(flags, None)
    }

    /// Query all packages that match the given provider identity
    pub fn query_provider(&self, provider: &Provider, flags: package::Flags) -> Vec<Package> {
        self.query(flags, Some(db::meta::Filter::Provider(provider.clone())))
    }

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
        if flags.available || flags == package::Flags::default() {
            // TODO: Error handling
            match self.active.db.provider_packages(provider) {
                Ok(packages) => packages,
                Err(error) => {
                    warn!("failed to query repository packages: {error}");
                    vec![]
                }
            }
        } else {
            vec![]
        }
    }
}

impl PartialEq for Repository {
    fn eq(&self, other: &Self) -> bool {
        self.active.id.eq(&other.active.id)
    }
}

impl Eq for Repository {}
