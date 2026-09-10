use std::fmt;

use scoop_identity::{BindingTargetError, PersistentExportBindingId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirExportBindingEntityKind {
    Struct,
    Enum,
    Class,
    Interface,
    Object,
    Function,
    Property,
    TypeAlias,
}

impl fmt::Display for HirExportBindingEntityKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Object => "object",
            Self::Function => "function",
            Self::Property => "property",
            Self::TypeAlias => "type alias",
        })
    }
}

#[derive(Debug)]
pub enum HirExportBindingIdentityError {
    UnknownSurfaceEntity {
        kind: HirExportBindingEntityKind,
        index: u32,
    },
    NonSourceEntity {
        kind: HirExportBindingEntityKind,
        index: u32,
    },
    UnknownObjectValue {
        object: u32,
        value: u32,
    },
    ObjectValueRelation {
        object: u32,
        value: u32,
    },
    NonPublicPackageDeclaration {
        kind: HirExportBindingEntityKind,
        index: u32,
    },
    UnnamedDeclaration {
        kind: HirExportBindingEntityKind,
        index: u32,
    },
    InvalidTarget {
        kind: HirExportBindingEntityKind,
        index: u32,
        error: BindingTargetError,
    },
    InvalidIdentity {
        kind: HirExportBindingEntityKind,
        index: u32,
        error: scoop_wire::HashError,
    },
    DuplicateIdentity {
        identity: PersistentExportBindingId,
    },
}

impl fmt::Display for HirExportBindingIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSurfaceEntity { kind, index } => {
                write!(
                    formatter,
                    "public surface references unknown {kind} {index}"
                )
            }
            Self::NonSourceEntity { kind, index } => {
                write!(
                    formatter,
                    "public surface {kind} {index} is not a source declaration"
                )
            }
            Self::UnknownObjectValue { object, value } => {
                write!(
                    formatter,
                    "public object {object} references unknown value {value}"
                )
            }
            Self::ObjectValueRelation { object, value } => write!(
                formatter,
                "public object {object} does not own singleton value {value}"
            ),
            Self::NonPublicPackageDeclaration { kind, index } => write!(
                formatter,
                "public package {kind} {index} has a source-scoped identity"
            ),
            Self::UnnamedDeclaration { kind, index } => {
                write!(
                    formatter,
                    "public package {kind} {index} has no source name"
                )
            }
            Self::InvalidTarget { kind, index, error } => {
                write!(
                    formatter,
                    "invalid public {kind} {index} binding target: {error}"
                )
            }
            Self::InvalidIdentity { kind, index, error } => {
                write!(
                    formatter,
                    "invalid public {kind} {index} binding identity: {error}"
                )
            }
            Self::DuplicateIdentity { identity } => {
                write!(
                    formatter,
                    "public export binding identity {identity} is duplicated"
                )
            }
        }
    }
}

impl std::error::Error for HirExportBindingIdentityError {}
