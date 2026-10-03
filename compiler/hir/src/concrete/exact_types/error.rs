use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactTypeRelation {
    ChildType,
    FunctionType,
    FunctionResult,
    Struct,
    Enum,
    Class,
    Interface,
    ObjectBackingClass,
    IntrinsicNominalOwner,
}

impl fmt::Display for ExactTypeRelation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ChildType => "child type",
            Self::FunctionType => "function type",
            Self::FunctionResult => "function result",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Class => "class",
            Self::Interface => "interface",
            Self::ObjectBackingClass => "object backing class",
            Self::IntrinsicNominalOwner => "intrinsic nominal owner",
        })
    }
}

#[derive(Debug)]
pub enum ExactTypeIdentityError {
    UnknownReference {
        ty: Option<u32>,
        relation: ExactTypeRelation,
        target: u32,
    },
    DuplicateObjectBackingClass {
        class: u32,
    },
    NonCanonicalNominalType {
        ty: u32,
        relation: ExactTypeRelation,
        canonical: u32,
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
    InvalidIdentity {
        ty: u32,
        error: scoop_wire::HashError,
    },
    InvalidNominalSpecialization {
        ty: u32,
        error: scoop_wire::HashError,
    },
    DuplicateIdentity {
        ty: u32,
    },
    DuplicateNominalSpecialization {
        ty: u32,
    },
}

impl fmt::Display for ExactTypeIdentityError {
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
            Self::NonCanonicalNominalType {
                ty,
                relation,
                canonical,
            } => write!(
                formatter,
                "type {ty} is a {relation}, but its declaration names type {canonical} as canonical"
            ),
            Self::Cycle { ty } => write!(formatter, "type {ty} recursively contains itself"),
            Self::MissingIdentity { ty } => write!(formatter, "type {ty} has no exact identity"),
            Self::EmptyTuple { ty } => write!(formatter, "type {ty} is an empty tuple"),
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
            Self::InvalidIdentity { ty, error } => {
                write!(
                    formatter,
                    "type {ty} has an invalid exact identity: {error}"
                )
            }
            Self::InvalidNominalSpecialization { ty, error } => write!(
                formatter,
                "type {ty} has an invalid nominal specialization: {error}"
            ),
            Self::DuplicateIdentity { ty } => {
                write!(
                    formatter,
                    "type {ty} duplicates another exact type identity"
                )
            }
            Self::DuplicateNominalSpecialization { ty } => write!(
                formatter,
                "type {ty} duplicates another nominal specialization identity"
            ),
        }
    }
}

impl std::error::Error for ExactTypeIdentityError {}
