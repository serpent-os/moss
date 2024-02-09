// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::collections::HashSet;
use std::path::Path;

use sqlx::{sqlite::SqliteConnectOptions, Acquire, Pool, Sqlite};
use sqlx::{Executor, QueryBuilder};
use thiserror::Error;

use crate::db::Encoding;
use crate::package::{self, Meta};
use crate::{Dependency, Provider};

#[derive(Debug, Clone, Copy)]
enum Table {
    Meta,
    Licenses,
    Dependencies,
    Providers,
    Conflicts,
}

#[derive(Debug)]
pub enum Filter {
    Provider(Provider),
    Dependency(Dependency),
    Name(package::Name),
}

impl Filter {
    fn append(&self, table: Table, query: &mut QueryBuilder<Sqlite>) {
        match self {
            Filter::Provider(p) => {
                if let Table::Providers = table {
                    query
                        .push(
                            "
                            where provider = 
                            ",
                        )
                        .push_bind(p.encode());
                } else {
                    query
                        .push(
                            "
                            where package in 
                                (select distinct package from meta_providers where provider = 
                            ",
                        )
                        .push_bind(p.encode())
                        .push(")");
                }
            }
            Filter::Dependency(d) => {
                if let Table::Dependencies = table {
                    query
                        .push(
                            "
                            where dependency = 
                            ",
                        )
                        .push_bind(d.encode());
                } else {
                    query
                        .push(
                            "
                            where package in 
                                (select distinct package from meta_dependencies where dependency = 
                            ",
                        )
                        .push_bind(d.encode())
                        .push(")");
                }
            }
            Filter::Name(n) => {
                if let Table::Meta = table {
                    query
                        .push(
                            "
                            where name = 
                            ",
                        )
                        .push_bind(n.encode().to_string());
                } else {
                    query
                        .push(
                            "
                            where package in 
                                (select distinct package from meta where name = 
                            ",
                        )
                        .push_bind(n.encode().to_string())
                        .push(")");
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Database {
    pool: Pool<Sqlite>,
}

impl Database {
    pub async fn new(path: impl AsRef<Path>, read_only: bool) -> Result<Self, Error> {
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .read_only(read_only)
            .foreign_keys(true);

        Self::connect(options).await
    }

    async fn connect(options: SqliteConnectOptions) -> Result<Self, Error> {
        let pool = sqlx::SqlitePool::connect_with(options).await?;

        sqlx::migrate!("src/db/meta/migrations").run(&pool).await?;

        Ok(Self { pool })
    }

    pub async fn wipe(&self) -> Result<(), Error> {
        // Other tables cascade delete so we only need to truncate `meta`
        sqlx::query("DELETE FROM meta;").execute(&self.pool).await?;
        Ok(())
    }

    pub async fn query(&self, filter: Option<Filter>) -> Result<Vec<(package::Id, Meta)>, Error> {
        let mut entry_query = sqlx::QueryBuilder::new(
            "
            SELECT package,
                   name,
                   version_identifier,
                   source_release,
                   build_release,
                   architecture,
                   summary,
                   description,
                   source_id,
                   homepage,
                   uri,
                   hash,
                   download_size
            FROM meta
            ",
        );

        let mut licenses_query = sqlx::QueryBuilder::new(
            "
            SELECT package, license
            FROM meta_licenses
            ",
        );

        let mut dependencies_query = sqlx::QueryBuilder::new(
            "
            SELECT package, dependency
            FROM meta_dependencies
            ",
        );

        let mut providers_query = sqlx::QueryBuilder::new(
            "
            SELECT package, provider
            FROM meta_providers
            ",
        );

        let mut conflicts_query = sqlx::QueryBuilder::new(
            "
            SELECT package, conflict
            FROM meta_conflicts
            ",
        );

        if let Some(filter) = filter {
            filter.append(Table::Meta, &mut entry_query);
            filter.append(Table::Licenses, &mut licenses_query);
            filter.append(Table::Dependencies, &mut dependencies_query);
            filter.append(Table::Providers, &mut providers_query);
            filter.append(Table::Conflicts, &mut conflicts_query)
        }

        let (entries, licenses, dependencies, providers, conflicts) = futures::try_join!(
            entry_query
                .build_query_as::<encoding::Entry>()
                .fetch_all(&self.pool),
            licenses_query
                .build_query_as::<encoding::License>()
                .fetch_all(&self.pool),
            dependencies_query
                .build_query_as::<encoding::Dependency>()
                .fetch_all(&self.pool),
            providers_query
                .build_query_as::<encoding::Provider>()
                .fetch_all(&self.pool),
            conflicts_query
                .build_query_as::<encoding::Conflict>()
                .fetch_all(&self.pool)
        )?;

        Ok(entries
            .into_iter()
            .map(|entry| {
                (
                    entry.id.0.clone(),
                    Meta {
                        name: entry.name.0,
                        version_identifier: entry.version_identifier,
                        source_release: entry.source_release as u64,
                        build_release: entry.build_release as u64,
                        architecture: entry.architecture,
                        summary: entry.summary,
                        description: entry.description,
                        source_id: entry.source_id,
                        homepage: entry.homepage,
                        licenses: licenses
                            .iter()
                            .filter(|l| l.id.0 == entry.id.0)
                            .map(|l| l.license.clone())
                            .collect(),
                        dependencies: dependencies
                            .iter()
                            .filter(|l| l.id.0 == entry.id.0)
                            .map(|d| d.dependency.0.clone())
                            .collect(),
                        providers: providers
                            .iter()
                            .filter(|l| l.id.0 == entry.id.0)
                            .map(|p| p.provider.0.clone())
                            .collect(),
                        conflicts: conflicts
                            .iter()
                            .filter(|l| l.id.0 == entry.id.0)
                            .map(|p| p.conflict.0.clone())
                            .collect(),
                        uri: entry.uri,
                        hash: entry.hash,
                        download_size: entry.download_size.map(|i| i as u64),
                    },
                )
            })
            .collect())
    }

    pub async fn get(&self, package: &package::Id) -> Result<Meta, Error> {
        let entry_query = sqlx::query_as::<_, encoding::Entry>(
            "
            SELECT package, 
                   name,
                   version_identifier,
                   source_release,
                   build_release,
                   architecture,
                   summary,
                   description,
                   source_id,
                   homepage,
                   uri,
                   hash,
                   download_size
            FROM meta
            WHERE package = ?;
            ",
        )
        .bind(package.encode());

        let licenses_query = sqlx::query_as::<_, encoding::License>(
            "
            SELECT package, license
            FROM meta_licenses
            WHERE package = ?;
            ",
        )
        .bind(package.encode());

        let dependencies_query = sqlx::query_as::<_, encoding::Dependency>(
            "
            SELECT package, dependency
            FROM meta_dependencies
            WHERE package = ?;
            ",
        )
        .bind(package.encode());

        let providers_query = sqlx::query_as::<_, encoding::Provider>(
            "
            SELECT package, provider
            FROM meta_providers
            WHERE package = ?;
            ",
        )
        .bind(package.encode());

        let conflicts_query = sqlx::query_as::<_, encoding::Conflict>(
            "
            SELECT package, conflict
            FROM meta_conflicts
            WHERE package = ?;
            ",
        )
        .bind(package.encode());

        let (entry, licenses, dependencies, providers, conflicts) = futures::try_join!(
            entry_query.fetch_one(&self.pool),
            licenses_query.fetch_all(&self.pool),
            dependencies_query.fetch_all(&self.pool),
            providers_query.fetch_all(&self.pool),
            conflicts_query.fetch_all(&self.pool),
        )?;

        Ok(Meta {
            name: entry.name.0,
            version_identifier: entry.version_identifier,
            source_release: entry.source_release as u64,
            build_release: entry.build_release as u64,
            architecture: entry.architecture,
            summary: entry.summary,
            description: entry.description,
            source_id: entry.source_id,
            homepage: entry.homepage,
            licenses: licenses.into_iter().map(|l| l.license).collect(),
            dependencies: dependencies.into_iter().map(|d| d.dependency.0).collect(),
            providers: providers.into_iter().map(|p| p.provider.0).collect(),
            conflicts: conflicts.into_iter().map(|p| p.conflict.0).collect(),
            uri: entry.uri,
            hash: entry.hash,
            download_size: entry.download_size.map(|i| i as u64),
        })
    }

    pub async fn file_hashes(&self) -> Result<HashSet<String>, Error> {
        let hashes = sqlx::query_as::<_, (String,)>(
            "
            SELECT DISTINCT hash
            FROM meta
            WHERE hash IS NOT NULL;
            ",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(hashes.into_iter().map(|(hash,)| hash).collect())
    }

    pub async fn add(&self, id: package::Id, meta: Meta) -> Result<(), Error> {
        self.batch_add(vec![(id, meta)]).await
    }

    pub async fn batch_add(&self, packages: Vec<(package::Id, Meta)>) -> Result<(), Error> {
        let mut transaction = self.pool.begin().await?;

        // Remove package (other tables cascade)
        batch_remove_impl(
            packages.iter().map(|(id, _)| id),
            transaction.acquire().await?,
        )
        .await?;

        // Create entry
        sqlx::QueryBuilder::new(
            "
            INSERT INTO meta (
                package,
                name,
                version_identifier,
                source_release,
                build_release,
                architecture,
                summary,
                description,
                source_id,
                homepage,
                uri,
                hash,
                download_size                
            )
            ",
        )
        .push_values(&packages, |mut b, (id, meta)| {
            let Meta {
                name,
                version_identifier,
                source_release,
                build_release,
                architecture,
                summary,
                description,
                source_id,
                homepage,
                uri,
                hash,
                download_size,
                ..
            } = meta;

            b.push_bind(id.encode())
                .push_bind(name.encode())
                .push_bind(version_identifier)
                .push_bind(*source_release as i64)
                .push_bind(*build_release as i64)
                .push_bind(architecture)
                .push_bind(summary)
                .push_bind(description)
                .push_bind(source_id)
                .push_bind(homepage)
                .push_bind(uri)
                .push_bind(hash)
                .push_bind(download_size.map(|i| i as i64));
        })
        .build()
        .execute(transaction.acquire().await?)
        .await?;

        // Licenses
        let licenses = packages
            .iter()
            .flat_map(|(id, meta)| meta.licenses.iter().map(move |license| (id, license)))
            .collect::<Vec<_>>();
        if !licenses.is_empty() {
            sqlx::QueryBuilder::new(
                "
                INSERT INTO meta_licenses (package, license)
                ",
            )
            .push_values(licenses, |mut b, (id, license)| {
                b.push_bind(id.encode()).push_bind(license);
            })
            .build()
            .execute(transaction.acquire().await?)
            .await?;
        }

        // Dependencies
        let dependencies = packages
            .iter()
            .flat_map(|(id, meta)| {
                meta.dependencies
                    .iter()
                    .map(move |dependency| (id, dependency))
            })
            .collect::<Vec<_>>();
        if !dependencies.is_empty() {
            sqlx::QueryBuilder::new(
                "
                INSERT INTO meta_dependencies (package, dependency)
                ",
            )
            .push_values(dependencies, |mut b, (id, dependency)| {
                b.push_bind(id.encode()).push_bind(dependency.encode());
            })
            .build()
            .execute(transaction.acquire().await?)
            .await?;
        }

        // Providers
        let providers = packages
            .iter()
            .flat_map(|(id, meta)| meta.providers.iter().map(move |provider| (id, provider)))
            .collect::<Vec<_>>();
        if !providers.is_empty() {
            sqlx::QueryBuilder::new(
                "
                INSERT INTO meta_providers (package, provider)
                ",
            )
            .push_values(providers, |mut b, (id, provider)| {
                b.push_bind(id.encode()).push_bind(provider.encode());
            })
            .build()
            .execute(transaction.acquire().await?)
            .await?;
        }

        // Conflicts
        let conflicts = packages
            .iter()
            .flat_map(|(id, meta)| meta.conflicts.iter().map(move |conflict| (id, conflict)))
            .collect::<Vec<_>>();
        println!("conflicts: {conflicts:?}");
        if !conflicts.is_empty() {
            sqlx::QueryBuilder::new(
                "
                INSERT INTO meta_conflicts (package, conflict)
                ",
            )
            .push_values(conflicts, |mut b, (id, conflict)| {
                b.push_bind(id.encode()).push_bind(conflict.encode());
            })
            .build()
            .execute(transaction.acquire().await?)
            .await?;
        }

        transaction.commit().await?;

        Ok(())
    }

    pub async fn remove(&self, package: &package::Id) -> Result<(), Error> {
        self.batch_remove(Some(package)).await
    }

    pub async fn batch_remove(
        &self,
        packages: impl IntoIterator<Item = &package::Id>,
    ) -> Result<(), Error> {
        batch_remove_impl(packages, &self.pool).await
    }
}

async fn batch_remove_impl<'a>(
    packages: impl IntoIterator<Item = &package::Id>,
    connection: impl Executor<'a, Database = Sqlite>,
) -> Result<(), Error> {
    let mut query_builder = sqlx::QueryBuilder::new(
        "
        DELETE FROM meta
        WHERE package IN (
        ",
    );

    let mut separated = query_builder.separated(", ");
    packages.into_iter().for_each(|package| {
        separated.push_bind(package.encode());
    });
    separated.push_unseparated(");");

    query_builder.build().execute(connection).await?;

    Ok(())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Row not found")]
    RowNotFound,
    #[error("sqlx")]
    Sqlx(#[source] sqlx::Error),
    #[error("sqlx migration")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

impl From<sqlx::Error> for Error {
    fn from(error: sqlx::Error) -> Self {
        match error {
            sqlx::Error::RowNotFound => Error::RowNotFound,
            error => Error::Sqlx(error),
        }
    }
}

mod encoding {
    use sqlx::FromRow;

    use crate::db::Decoder;
    use crate::package;

    #[derive(FromRow)]
    pub struct Entry {
        #[sqlx(rename = "package")]
        pub id: Decoder<package::Id>,
        pub name: Decoder<package::Name>,
        pub version_identifier: String,
        pub source_release: i64,
        pub build_release: i64,
        pub architecture: String,
        pub summary: String,
        pub description: String,
        pub source_id: String,
        pub homepage: String,
        pub uri: Option<String>,
        pub hash: Option<String>,
        pub download_size: Option<i64>,
    }

    #[derive(FromRow)]
    pub struct License {
        #[sqlx(rename = "package")]
        pub id: Decoder<package::Id>,
        pub license: String,
    }

    #[derive(FromRow)]
    pub struct Dependency {
        #[sqlx(rename = "package")]
        pub id: Decoder<package::Id>,
        pub dependency: Decoder<crate::Dependency>,
    }

    #[derive(FromRow)]
    pub struct Provider {
        #[sqlx(rename = "package")]
        pub id: Decoder<package::Id>,
        pub provider: Decoder<crate::Provider>,
    }

    #[derive(FromRow)]
    pub struct Conflict {
        #[sqlx(rename = "package")]
        pub id: Decoder<package::Id>,
        pub conflict: Decoder<crate::Conflict>,
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use stone::read::PayloadKind;

    use crate::dependency::Kind;

    use super::*;

    #[tokio::test]
    async fn create_insert_select() {
        let database =
            Database::connect(SqliteConnectOptions::from_str("sqlite::memory:").unwrap())
                .await
                .unwrap();

        let bash_completion =
            include_bytes!("../../../../../test/bash-completion-2.11-1-1-x86_64.stone");

        let mut stone = stone::read_bytes(bash_completion).unwrap();

        let payloads = stone
            .payloads()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let meta_payload = payloads.iter().find_map(PayloadKind::meta).unwrap();
        let meta = Meta::from_stone_payload(&meta_payload.body).unwrap();

        let id = package::Id::from("test".to_string());

        database.add(id.clone(), meta.clone()).await.unwrap();

        assert_eq!(&meta.name, &"bash-completion".to_string().into());
        assert_eq!(meta.conflicts, HashSet::default());

        // Now retrieve by provider.
        let lookup = Filter::Provider(Provider {
            kind: Kind::PackageName,
            name: "bash-completion".to_string(),
        });
        let fetched = database.query(Some(lookup)).await.unwrap();
        assert_eq!(fetched.len(), 1);

        batch_remove_impl([&id], &database.pool).await.unwrap();

        let result = database.get(&id).await;

        assert!(result.is_err());

        // Test wipe
        database.add(id.clone(), meta.clone()).await.unwrap();
        database.wipe().await.unwrap();
        let result = database.get(&id).await;
        assert!(result.is_err());
    }
}
