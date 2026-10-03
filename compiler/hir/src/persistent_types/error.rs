use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirTypeRelation {
    ChildType,
    FunctionType,
    StructApplication,
    EnumApplication,
    ClassApplication,
    InterfaceApplication,
    ObjectBackingClass,
    IntrinsicNominalOwner,
}

impl fmt::Display for HirTypeRelation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ChildType => "child type",
            Self::FunctionType => "function type",
            Self::StructApplication => "struct application",
            Self::EnumApplication => "enum application",
            Self::ClassApplication => "class application",
            Self::InterfaceApplication => "interface application",
            Self::ObjectBackingClass => "object backing class",
            Self::IntrinsicNominalOwner => "intrinsic nominal owner",
        })
    }
}

#[derive(Debug)]
pub enum HirTypeIdentityError {
    UnknownReference {
        ty: Option<u32>,
        relation: HirTypeRelation,
        target: u32,
    },
    DuplicateObjectBackingClass {
        class: u32,
    },
    Cycle {
        ty: u32,
    },
    MissingIdentity {
        ty: u32,
    },
    EmptyTuple {
        ty: u32,
    },
    InvalidIntrinsicNominal {
        ty: u32,
    },
    InvalidApplication {
        ty: u32,
    },
    NominalArity {
        ty: u32,
        expected: usize,
        actual: usize,
    },
    NominalIdentityKind {
        ty: u32,
    },
    InvalidFunctionType {
        ty: u32,
    },
    ConflictingBinderSlot {
        ty: u32,
        identity: u32,
    },
    InvalidIdentity {
        ty: u32,
        error: scoop_wire::HashError,
    },
    DuplicateExactIdentity {
        ty: u32,
    },
}

impl HirTypeIdentityError {
    pub const fn ty(&self) -> Option<u32> {
        match self {
            Self::UnknownReference { ty, .. } => *ty,
            Self::DuplicateObjectBackingClass { .. } => None,
            Self::Cycle { ty }
            | Self::MissingIdentity { ty }
            | Self::EmptyTuple { ty }
            | Self::InvalidIntrinsicNominal { ty }
            | Self::InvalidApplication { ty }
            | Self::NominalArity { ty, .. }
            | Self::NominalIdentityKind { ty }
            | Self::InvalidFunctionType { ty }
            | Self::ConflictingBinderSlot { ty, .. }
            | Self::InvalidIdentity { ty, .. }
            | Self::DuplicateExactIdentity { ty } => Some(*ty),
        }
    }
}

impl fmt::Display for HirTypeIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownReference {
                ty,
                relation,
                target,
            } => match ty {
                Some(ty) => write!(formatter, "type {ty} refers to unknown {relation} {target}"),
                None => write!(formatter, "unknown {relation} {target}"),
            },
            Self::DuplicateObjectBackingClass { class } => {
                write!(formatter, "class {class} backs more than one source object")
            }
            Self::Cycle { ty } => write!(formatter, "type {ty} recursively contains itself"),
            Self::MissingIdentity { ty } => write!(formatter, "type {ty} has no identity"),
            Self::EmptyTuple { ty } => write!(formatter, "type {ty} is an empty tuple"),
            Self::InvalidIntrinsicNominal { ty } => write!(
                formatter,
                "intrinsic type {ty} is not backed by a concrete source nominal identity"
            ),
            Self::InvalidApplication { ty } => write!(
                formatter,
                "type {ty} has an invalid nominal application or canonical-type relation"
            ),
            Self::NominalArity {
                ty,
                expected,
                actual,
            } => write!(
                formatter,
                "type {ty} nominal application has {actual} arguments, expected {expected}"
            ),
            Self::NominalIdentityKind { ty } => write!(
                formatter,
                "type {ty} has a nominal genericity inconsistent with its persistent identity"
            ),
            Self::InvalidFunctionType { ty } => write!(
                formatter,
                "type {ty} has an invalid function-signature canonical-type relation"
            ),
            Self::ConflictingBinderSlot { ty, identity } => write!(
                formatter,
                "type {ty} contains type-parameter identity {identity} with conflicting substitution slots"
            ),
            Self::InvalidIdentity { ty, error } => {
                write!(
                    formatter,
                    "type {ty} has an invalid exact identity: {error}"
                )
            }
            Self::DuplicateExactIdentity { ty } => {
                write!(
                    formatter,
                    "type {ty} duplicates another exact type identity"
                )
            }
        }
    }
}

impl std::error::Error for HirTypeIdentityError {}
