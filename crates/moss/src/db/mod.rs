// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

pub use self::encoding::{Decoder, Encoding};

pub mod layout;
pub mod meta;
pub mod state;

mod encoding {
    //! Decode from sql types to rust types
    use std::convert::Infallible;

    use sqlx::{Sqlite, Type};
    use thiserror::Error;

    use crate::{dependency, package, state, Conflict, Dependency, Provider};

    /// Decode from a database type using [`Encoding::decode`]
    #[derive(Debug, Clone, Copy)]
    pub struct Decoder<T>(pub T);

    /// A trait to define an encoding between a sql type and rust type
    pub trait Encoding<'a>: Sized {
        type Encoded: ToOwned;
        type Error;

        fn decode(encoded: Self::Encoded) -> Result<Self, Self::Error>;
        fn encode(&'a self) -> Self::Encoded;
    }

    impl<'r, T, U, E> sqlx::Decode<'r, Sqlite> for Decoder<T>
    where
        T: Encoding<'r, Encoded = U, Error = E>,
        U: sqlx::Decode<'r, Sqlite> + ToOwned,
        E: std::error::Error + Send + Sync + 'static,
    {
        fn decode(
            value: <Sqlite as sqlx::database::HasValueRef<'r>>::ValueRef,
        ) -> Result<Self, sqlx::error::BoxDynError> {
            Ok(T::decode(U::decode(value)?).map(Decoder)?)
        }
    }

    impl<T, U, E> Type<Sqlite> for Decoder<T>
    where
        T: Encoding<'static, Encoded = U, Error = E>,
        U: ToOwned + Type<Sqlite>,
    {
        fn type_info() -> <Sqlite as sqlx::Database>::TypeInfo {
            U::type_info()
        }

        fn compatible(ty: &<Sqlite as sqlx::Database>::TypeInfo) -> bool {
            U::compatible(ty)
        }
    }

    /** Encoding on external types */

    /// Encoding of package identity (String)
    impl<'a> Encoding<'a> for package::Id {
        type Encoded = &'a str;
        type Error = Infallible;

        fn decode(encoded: &'a str) -> Result<Self, Self::Error> {
            Ok(package::Id::from(encoded.to_owned()))
        }

        fn encode(&'a self) -> &'a str {
            self.as_ref()
        }
    }

    /// Encoding of package name (String)
    impl<'a> Encoding<'a> for package::Name {
        type Encoded = &'a str;
        type Error = Infallible;

        fn decode(encoded: &'a str) -> Result<Self, Self::Error> {
            Ok(package::Name::from(encoded.to_owned()))
        }

        fn encode(&'a self) -> &'a str {
            self.as_ref()
        }
    }

    /// Encoding of Dependency type
    impl<'a> Encoding<'a> for Dependency {
        type Encoded = String;
        type Error = dependency::ParseError;

        fn decode(encoded: String) -> Result<Self, Self::Error> {
            encoded.parse()
        }

        fn encode(&self) -> String {
            self.to_string()
        }
    }

    /// Encoding of Provider type
    impl<'a> Encoding<'a> for Provider {
        type Encoded = String;
        type Error = dependency::ParseError;

        fn decode(encoded: String) -> Result<Self, Self::Error> {
            encoded.parse()
        }

        fn encode(&self) -> String {
            self.to_string()
        }
    }

    /// Encoding of Conflict type
    impl<'a> Encoding<'a> for Conflict {
        type Encoded = String;
        type Error = dependency::ParseError;

        fn decode(encoded: String) -> Result<Self, Self::Error> {
            encoded.parse()
        }

        fn encode(&self) -> String {
            self.to_string()
        }
    }

    impl<'a> Encoding<'a> for state::Id {
        type Encoded = i64;
        type Error = Infallible;

        fn decode(value: i64) -> Result<Self, Self::Error> {
            Ok(Self::from(value))
        }

        fn encode(&self) -> i64 {
            (*self).into()
        }
    }

    impl<'a> Encoding<'a> for state::Kind {
        type Encoded = &'a str;
        type Error = DecodeStateKindError;

        fn decode(value: &'a str) -> Result<Self, Self::Error> {
            match value {
                "transaction" => Ok(Self::Transaction),
                _ => Err(DecodeStateKindError(value.to_string())),
            }
        }

        fn encode(&self) -> Self::Encoded {
            match self {
                state::Kind::Transaction => "transaction",
            }
        }
    }

    #[derive(Debug, Error)]
    #[error("Invalid state type: {0}")]
    pub struct DecodeStateKindError(String);
}
