// SPDX-FileCopyrightText: Copyright © 2020-2024 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{fmt, str::FromStr};

use stone::payload;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Name based dependency
    PackageName,

    /// Shared library (soname)
    SharedLibary,

    /// Exported pkg-config provider
    PkgConfig,

    /// PT_INTERP, or specialist shell support
    Interpreter,

    /// CMake module dependency
    CMake,

    /// Python dependency (unused)
    Python,

    /// Executable in /usr/bin
    Binary,

    /// Executable in /usr/sbin
    SystemBinary,

    /// Exported 32-bit pkgconfig provider
    PkgConfig32,
}

/// Custom pretty-print, i.e `pkgconfig(zlib)`
impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::PackageName => write!(f, "name"),
            Kind::SharedLibary => write!(f, "soname"),
            Kind::PkgConfig => write!(f, "pkgconfig"),
            Kind::Interpreter => write!(f, "interpreter"),
            Kind::CMake => write!(f, "cmake"),
            Kind::Python => write!(f, "python"),
            Kind::Binary => write!(f, "binary"),
            Kind::SystemBinary => write!(f, "sysbinary"),
            Kind::PkgConfig32 => write!(f, "pkgconfig32"),
        }
    }
}

/// Decode a name into a Kind (yaml helper)
impl FromStr for Kind {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "name" => Kind::PackageName,
            "soname" => Kind::SharedLibary,
            "pkgconfig" => Kind::PkgConfig,
            "interpreter" => Kind::Interpreter,
            "cmake" => Kind::CMake,
            "python" => Kind::Python,
            "binary" => Kind::Binary,
            "sysbinary" => Kind::SystemBinary,
            "pkgconfig32" => Kind::PkgConfig32,
            _ => return Err(ParseError(s.to_string())),
        })
    }
}

/// Convert payload dependency types to our internal representation
impl From<payload::meta::Dependency> for Kind {
    fn from(dependency: payload::meta::Dependency) -> Self {
        match dependency {
            payload::meta::Dependency::PackageName => Kind::PackageName,
            payload::meta::Dependency::SharedLibary => Kind::SharedLibary,
            payload::meta::Dependency::PkgConfig => Kind::PkgConfig,
            payload::meta::Dependency::Interpreter => Kind::Interpreter,
            payload::meta::Dependency::CMake => Kind::CMake,
            payload::meta::Dependency::Python => Kind::Python,
            payload::meta::Dependency::Binary => Kind::Binary,
            payload::meta::Dependency::SystemBinary => Kind::SystemBinary,
            payload::meta::Dependency::PkgConfig32 => Kind::PkgConfig32,
        }
    }
}

impl From<Kind> for payload::meta::Dependency {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::PackageName => Self::PackageName,
            Kind::SharedLibary => Self::SharedLibary,
            Kind::PkgConfig => Self::PkgConfig,
            Kind::Interpreter => Self::Interpreter,
            Kind::CMake => Self::CMake,
            Kind::Python => Self::Python,
            Kind::Binary => Self::Binary,
            Kind::SystemBinary => Self::SystemBinary,
            Kind::PkgConfig32 => Self::PkgConfig32,
        }
    }
}

/// A Dependency in moss is simplistic in that it only contains
/// a target and a Kind, ie. `pkgconfig(zlib)`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Dependency {
    /// Tag for the table-type of dependency
    pub kind: Kind,

    /// Bare target
    pub name: String,
}

/// Pretty-printing of dependencies (e.g.: `binary(whoami)`)
impl fmt::Display for Dependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.kind, self.name)
    }
}

impl FromStr for Dependency {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (kind, name) = parse(s)?;

        Ok(Self { kind, name })
    }
}

/// A Conflict in moss is a provider that cannot be co-installed with the
/// current package
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Conflict {
    /// Tag for the table-type of dependency
    pub kind: Kind,

    /// Bare target
    pub name: String,
}

/// Pretty-printing of dependencies (e.g.: `binary(whoami)`)
impl fmt::Display for Conflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.kind, self.name)
    }
}

impl FromStr for Conflict {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (kind, name) = parse(s)?;

        Ok(Self { kind, name })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Provider {
    pub kind: Kind,
    pub name: String,
}

impl Provider {
    pub fn from_name(name: &str) -> Result<Self, ParseError> {
        if name.contains('(') {
            Provider::from_str(name)
        } else {
            Ok(Provider {
                kind: Kind::PackageName,
                name: name.to_owned(),
            })
        }
    }
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.kind, self.name)
    }
}

impl FromStr for Provider {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (kind, name) = parse(s)?;

        Ok(Self { kind, name })
    }
}

fn parse(s: &str) -> Result<(Kind, String), ParseError> {
    let (kind, rest) = s.split_once('(').ok_or(ParseError(s.to_string()))?;

    if !rest.ends_with(')') {
        return Err(ParseError(s.to_string()));
    }

    let kind = kind.parse()?;
    // Safe since we checked `ends_with(')')`
    let name = rest[0..rest.len() - 1].to_string();

    Ok((kind, name))
}

#[derive(Debug, Error)]
#[error("Invalid dependency type: {0}")]
pub struct ParseError(String);
