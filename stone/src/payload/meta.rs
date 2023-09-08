// SPDX-FileCopyrightText: Copyright © 2020-2023 Serpent OS Developers
//
// SPDX-License-Identifier: MPL-2.0

use std::{fmt::Display, io::Read};

use super::{DecodeError, Record};
use crate::ReadExt;

///
/// The Meta payload contains a series of sequential records with
/// strong types and context tags, i.e. their use such as Name.
/// These record all metadata for every .stone packages and provide
/// no content
///
// TODO: Strong types these fields
#[derive(Debug)]
pub struct Meta {
    pub tag: MetaTag,
    pub kind: MetaKind,
}

#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyKind {
    /// Just the plain name of a package
    PackageName = 0,

    /// A soname based dependency
    SharedLibary,

    /// A pkgconfig `.pc` based dependency
    PkgConfig,

    /// Special interpreter (PT_INTERP/etc) to run the binaries
    Interpreter,

    /// A CMake module
    CMake,

    /// A Python module
    Python,

    /// A binary in /usr/bin
    Binary,

    /// A binary in /usr/sbin
    SystemBinary,

    /// An emul32-compatible pkgconfig .pc dependency (lib32/*.pc)
    PkgConfig32,
}

///
/// Override display for `pkgconfig32(name)` style strings
///
impl Display for DependencyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DependencyKind::PackageName => write!(f, "name"),
            DependencyKind::SharedLibary => write!(f, "soname"),
            DependencyKind::PkgConfig => write!(f, "pkgconfig"),
            DependencyKind::Interpreter => write!(f, "interpreter"),
            DependencyKind::CMake => write!(f, "cmake"),
            DependencyKind::Python => write!(f, "python"),
            DependencyKind::Binary => write!(f, "binary"),
            DependencyKind::SystemBinary => write!(f, "sysbinary"),
            DependencyKind::PkgConfig32 => write!(f, "pkgconfig32"),
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetaKind {
    Int8(i8),
    Uint8(u8),
    Int16(i16),
    Uint16(u16),
    Int32(i32),
    Uint32(u32),
    Int64(i64),
    Uint64(u64),
    String(String),
    Dependency(DependencyKind, String),
    Provider(DependencyKind, String),
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetaTag {
    // Name of the package
    Name = 1,
    // Architecture of the package
    Architecture = 2,
    // Version of the package
    Version = 3,
    // Summary of the package
    Summary = 4,
    // Description of the package
    Description = 5,
    // Homepage for the package
    Homepage = 6,
    // ID for the source package, used for grouping
    SourceID = 7,
    // Runtime dependencies
    Depends = 8,
    // Provides some capability or name
    Provides = 9,
    // Conflicts with some capability or name
    Conflicts = 10,
    // Release number for the package
    Release = 11,
    // SPDX license identifier
    License = 12,
    // Currently recorded build number
    BuildRelease = 13,
    // Repository index specific (relative URI)
    PackageURI = 14,
    // Repository index specific (Package hash)
    PackageHash = 15,
    // Repository index specific (size on disk)
    PackageSize = 16,
    // A Build Dependency
    BuildDepends = 17,
    // Upstream URI for the source
    SourceURI = 18,
    // Relative path for the source within the upstream URI
    SourcePath = 19,
    // Ref/commit of the upstream source
    SourceRef = 20,
}

///
/// Helper to decode a dependency's encoded kind
///
fn decode_dependency(i: u8) -> Result<DependencyKind, DecodeError> {
    let result = match i {
        0 => DependencyKind::PackageName,
        1 => DependencyKind::SharedLibary,
        2 => DependencyKind::PkgConfig,
        3 => DependencyKind::Interpreter,
        4 => DependencyKind::CMake,
        5 => DependencyKind::Python,
        6 => DependencyKind::Binary,
        7 => DependencyKind::SystemBinary,
        8 => DependencyKind::PkgConfig32,
        _ => return Err(DecodeError::UnknownDependency(i)),
    };
    Ok(result)
}

impl Record for Meta {
    fn decode<R: Read>(mut reader: R) -> Result<Self, DecodeError> {
        let length = reader.read_u32()?;

        let tag = match reader.read_u16()? {
            1 => MetaTag::Name,
            2 => MetaTag::Architecture,
            3 => MetaTag::Version,
            4 => MetaTag::Summary,
            5 => MetaTag::Description,
            6 => MetaTag::Homepage,
            7 => MetaTag::SourceID,
            8 => MetaTag::Depends,
            9 => MetaTag::Provides,
            10 => MetaTag::Conflicts,
            11 => MetaTag::Release,
            12 => MetaTag::License,
            13 => MetaTag::BuildRelease,
            14 => MetaTag::PackageURI,
            15 => MetaTag::PackageHash,
            16 => MetaTag::PackageSize,
            17 => MetaTag::BuildDepends,
            18 => MetaTag::SourceURI,
            19 => MetaTag::SourcePath,
            20 => MetaTag::SourceRef,
            t => return Err(DecodeError::UnknownMetaTag(t)),
        };

        let kind = reader.read_u8()?;
        let _padding = reader.read_array::<1>()?;

        let kind = match kind {
            1 => MetaKind::Int8(reader.read_u8()? as i8),
            2 => MetaKind::Uint8(reader.read_u8()?),
            3 => MetaKind::Int16(reader.read_u16()? as i16),
            4 => MetaKind::Uint16(reader.read_u16()?),
            5 => MetaKind::Int32(reader.read_u32()? as i32),
            6 => MetaKind::Uint32(reader.read_u32()?),
            7 => MetaKind::Int64(reader.read_u64()? as i64),
            8 => MetaKind::Uint64(reader.read_u64()?),
            9 => MetaKind::String(reader.read_string(length as u64)?),
            10 => MetaKind::Dependency(
                /* DependencyKind u8 subtracted from length  */
                decode_dependency(reader.read_u8()?)?,
                reader.read_string(length as u64 - 1)?,
            ),
            11 => MetaKind::Provider(
                /* DependencyKind u8 subtracted from length  */
                decode_dependency(reader.read_u8()?)?,
                reader.read_string(length as u64 - 1)?,
            ),
            k => return Err(DecodeError::UnknownMetaKind(k)),
        };

        Ok(Self { tag, kind })
    }
}
