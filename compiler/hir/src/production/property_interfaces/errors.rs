use std::fmt;

use scoop_identity::{AccessorRole, ConeIdentity, PropertyOwner as PersistentPropertyOwner};

use crate::{
    HirInterfaceSignatureProjectionError, PropertyCapabilityBuildError,
    PropertyInterfaceRecordBuildError, PropertyInterfaceSetBuildError, PropertyPublicAccessV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PropertyNominalOwnerKind {
    Class,
    Struct,
    Enum,
    Interface,
    Object,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportPropertyAccessorBuildError {
    Unknown(u32),
    MissingIdentity(u32),
    PropertyMismatch {
        actual: u32,
    },
    PersistentOwnerMismatch,
    RoleMismatch {
        actual: AccessorRole,
    },
    MissingPublicSurface,
    UnexpectedPublicSurface,
    NotPublic,
    AccessMismatch {
        expected: PropertyPublicAccessV1,
        actual: PropertyPublicAccessV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PropertyInterfaceBuildError {
    UnknownPublicProperty(u32),
    MissingPropertyIdentity(u32),
    ForeignDeclaration {
        property: PersistentPropertyOwner,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    InvalidDeclarationScope(PersistentPropertyOwner),
    InvalidPublicAccess(PersistentPropertyOwner),
    UnknownExtensionOwner {
        property: PersistentPropertyOwner,
        extension: u32,
    },
    ExtensionOwnerMismatch {
        property: PersistentPropertyOwner,
        actual_property: u32,
    },
    UnknownNominalOwner {
        property: PersistentPropertyOwner,
        kind: PropertyNominalOwnerKind,
        owner: u32,
    },
    GeneratedNominalOwner {
        property: PersistentPropertyOwner,
        kind: PropertyNominalOwnerKind,
        owner: u32,
    },
    ForeignNominalOwner {
        property: PersistentPropertyOwner,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    Signature {
        property: PersistentPropertyOwner,
        source: HirInterfaceSignatureProjectionError,
    },
    Accessor {
        property: PersistentPropertyOwner,
        role: AccessorRole,
        detail: ExportPropertyAccessorBuildError,
    },
    Capability {
        property: PersistentPropertyOwner,
        source: PropertyCapabilityBuildError,
    },
    Representation {
        property: PersistentPropertyOwner,
        detail: &'static str,
    },
    Record {
        property: PersistentPropertyOwner,
        source: PropertyInterfaceRecordBuildError,
    },
    Table(PropertyInterfaceSetBuildError),
}

impl fmt::Display for PropertyInterfaceBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPublicProperty(property) => {
                write!(
                    formatter,
                    "public property id {property} is outside the HIR arena"
                )
            }
            Self::MissingPropertyIdentity(property) => write!(
                formatter,
                "public property id {property} has no persistent identity"
            ),
            Self::ForeignDeclaration {
                property,
                expected,
                actual,
            } => write!(
                formatter,
                "property interface {property:?} belongs to Cone {actual}, not current Cone {expected}"
            ),
            Self::InvalidDeclarationScope(property) => write!(
                formatter,
                "property interface {property:?} does not have ConeWide declaration scope"
            ),
            Self::InvalidPublicAccess(property) => write!(
                formatter,
                "property interface {property:?} does not have a foreign-public access witness"
            ),
            Self::UnknownExtensionOwner {
                property,
                extension,
            } => write!(
                formatter,
                "property interface {property:?} references unknown extension owner {extension}"
            ),
            Self::ExtensionOwnerMismatch {
                property,
                actual_property,
            } => write!(
                formatter,
                "property interface {property:?} extension owner points to local property {actual_property}"
            ),
            Self::UnknownNominalOwner {
                property,
                kind,
                owner,
            } => write!(
                formatter,
                "property interface {property:?} references unknown {kind:?} owner {owner}"
            ),
            Self::GeneratedNominalOwner {
                property,
                kind,
                owner,
            } => write!(
                formatter,
                "property interface {property:?} uses generated {kind:?} owner {owner}"
            ),
            Self::ForeignNominalOwner {
                property,
                expected,
                actual,
            } => write!(
                formatter,
                "property interface {property:?} nominal owner belongs to Cone {actual}, not {expected}"
            ),
            Self::Signature { property, source } => {
                write!(
                    formatter,
                    "cannot project property interface {property:?}: {source}"
                )
            }
            Self::Accessor {
                property,
                role,
                detail,
            } => write!(
                formatter,
                "invalid {role:?} for property interface {property:?}: {detail}"
            ),
            Self::Capability { property, source } => write!(
                formatter,
                "invalid capability for property interface {property:?}: {source}"
            ),
            Self::Representation { property, detail } => write!(
                formatter,
                "invalid representation for property interface {property:?}: {detail}"
            ),
            Self::Record { property, source } => {
                write!(
                    formatter,
                    "invalid property interface {property:?}: {source}"
                )
            }
            Self::Table(source) => source.fmt(formatter),
        }
    }
}

impl fmt::Display for ExportPropertyAccessorBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(accessor) => write!(formatter, "unknown local accessor {accessor}"),
            Self::MissingIdentity(accessor) => {
                write!(
                    formatter,
                    "local accessor {accessor} has no persistent identity"
                )
            }
            Self::PropertyMismatch { actual } => {
                write!(formatter, "accessor points to local property {actual}")
            }
            Self::PersistentOwnerMismatch => {
                formatter.write_str("accessor persistent owner does not match its property")
            }
            Self::RoleMismatch { actual } => {
                write!(formatter, "accessor identity has role {actual:?}")
            }
            Self::MissingPublicSurface => {
                formatter.write_str("public accessor is absent from the public semantic surface")
            }
            Self::UnexpectedPublicSurface => {
                formatter.write_str("restricted setter appears in the public semantic surface")
            }
            Self::NotPublic => formatter.write_str("getter does not have foreign-public access"),
            Self::AccessMismatch { expected, actual } => write!(
                formatter,
                "accessor access {actual:?} does not match property access {expected:?}"
            ),
        }
    }
}

impl std::error::Error for PropertyInterfaceBuildError {}
