use std::fmt;

use scoop_identity::{
    AccessorRole, ConeIdentity, PropertyOwner, SignatureTypeKey, SourceDeclarationKind,
};

use crate::{
    CallableInterfaceRecordBuildError, CallableInterfaceSetBuildError,
    CallableSourceEffectsBuildError, HirInterfaceSignatureProjectionError,
    PropertyAccessorClosureValidationError, PropertyInterfaceBuildError,
    SourceParameterListBuildError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableProjectionSubject {
    Function(u32),
    StructConstructor(u32),
    ClassConstructor(u32),
    Variant { enumeration: u32, variant: u32 },
    Getter(u32),
    Setter(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableAccessProjectionError {
    NotDeclaredPublic,
    NonUniversalLookup,
    NonUniversalSlot,
    PublicSlotNotAllowed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableEffectProjectionError {
    UnknownExternFunction(u32),
    ConflictingOperatorRoles,
    Build(CallableSourceEffectsBuildError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceParameterProjectionError {
    MissingInterface,
    DuplicateInterface,
    Arity {
        expected: usize,
        actual: usize,
    },
    InvalidName {
        position: u32,
        source: scoop_identity::CanonicalIdentifierError,
    },
    UnknownVarargType {
        position: u32,
        parameter_type: u32,
    },
    Signature {
        position: u32,
        source: HirInterfaceSignatureProjectionError,
    },
    TypeMismatch {
        position: u32,
        expected: Box<SignatureTypeKey>,
        actual: Box<SignatureTypeKey>,
    },
    List(SourceParameterListBuildError),
}

#[derive(Debug)]
pub enum CallableProjectionError {
    UnknownDeclaration,
    MissingIdentity,
    NonSourceIdentity,
    GeneratedConstructor,
    ForeignDeclaration {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    InvalidDeclarationScope,
    InvalidDeclarationKind {
        expected: SourceDeclarationKind,
        actual: SourceDeclarationKind,
    },
    InvalidIdentityKind,
    TypeParameterArity {
        expected: u32,
        actual: u32,
    },
    InvalidOwner,
    MissingNominalOwner,
    ForeignNominalOwner {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    PersistentOwnerMismatch,
    ConstructorOwnerMismatch,
    MissingPropertyInterface(PropertyOwner),
    PropertyOwnerMismatch,
    AccessorRole {
        expected: AccessorRole,
        actual: AccessorRole,
    },
    MissingInterfaceMethod(u32),
    InterfaceMethodMismatch,
    AccessorAccessMismatch {
        expected: crate::PublicLookupAccessV1,
        actual: crate::PublicLookupAccessV1,
    },
    Signature(HirInterfaceSignatureProjectionError),
    Parameters(SourceParameterProjectionError),
    Access(CallableAccessProjectionError),
    Effects(CallableEffectProjectionError),
    Record(CallableInterfaceRecordBuildError),
}

#[derive(Debug)]
pub enum CallableInterfaceBuildError {
    PropertyInterfaces(PropertyInterfaceBuildError),
    Projection {
        subject: CallableProjectionSubject,
        error: CallableProjectionError,
    },
    Table(CallableInterfaceSetBuildError),
    AccessorClosure(PropertyAccessorClosureValidationError),
}

impl CallableInterfaceBuildError {
    pub(super) const fn projection(
        subject: CallableProjectionSubject,
        error: CallableProjectionError,
    ) -> Self {
        Self::Projection { subject, error }
    }
}

impl fmt::Display for CallableProjectionSubject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Function(index) => write!(formatter, "function {index}"),
            Self::StructConstructor(index) => write!(formatter, "struct constructor {index}"),
            Self::ClassConstructor(index) => write!(formatter, "class constructor {index}"),
            Self::Variant {
                enumeration,
                variant,
            } => write!(formatter, "enum {enumeration} variant {variant}"),
            Self::Getter(index) => write!(formatter, "property getter {index}"),
            Self::Setter(index) => write!(formatter, "property setter {index}"),
        }
    }
}

impl fmt::Display for CallableAccessProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotDeclaredPublic => "declaration is not explicitly public",
            Self::NonUniversalLookup => "declaration lookup domain is not universal",
            Self::NonUniversalSlot => "declaration slot domain is not universal",
            Self::PublicSlotNotAllowed => "declaration unexpectedly exposes a public slot",
        })
    }
}

impl fmt::Display for CallableEffectProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownExternFunction(index) => {
                write!(
                    formatter,
                    "function references unknown extern declaration {index}"
                )
            }
            Self::ConflictingOperatorRoles => formatter
                .write_str("callable declares both a language and property-delegate operator role"),
            Self::Build(source) => source.fmt(formatter),
        }
    }
}

impl fmt::Display for SourceParameterProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingInterface => formatter.write_str("source parameter interface is missing"),
            Self::DuplicateInterface => {
                formatter.write_str("source parameter interface is duplicated")
            }
            Self::Arity { expected, actual } => write!(
                formatter,
                "source parameter count {actual} does not match identity count {expected}"
            ),
            Self::InvalidName { position, source } => {
                write!(
                    formatter,
                    "invalid source parameter name at {position}: {source}"
                )
            }
            Self::UnknownVarargType {
                position,
                parameter_type,
            } => write!(
                formatter,
                "source parameter {position} references unknown vararg type {parameter_type}"
            ),
            Self::Signature { position, source } => {
                write!(
                    formatter,
                    "cannot project source parameter {position}: {source}"
                )
            }
            Self::TypeMismatch {
                position,
                expected,
                actual,
            } => write!(
                formatter,
                "source parameter {position} type {actual:?} does not match identity type {expected:?}"
            ),
            Self::List(source) => source.fmt(formatter),
        }
    }
}

impl fmt::Display for CallableProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownDeclaration => formatter.write_str("local declaration is unknown"),
            Self::MissingIdentity => formatter.write_str("persistent identity is missing"),
            Self::NonSourceIdentity => formatter.write_str("identity is not source-declared"),
            Self::GeneratedConstructor => {
                formatter.write_str("generated constructor is not a source callable")
            }
            Self::ForeignDeclaration { expected, actual } => write!(
                formatter,
                "declaration belongs to Cone {actual}, not current Cone {expected}"
            ),
            Self::InvalidDeclarationScope => {
                formatter.write_str("declaration does not have ConeWide scope")
            }
            Self::InvalidDeclarationKind { expected, actual } => write!(
                formatter,
                "declaration kind is {actual:?}, expected {expected:?}"
            ),
            Self::InvalidIdentityKind => {
                formatter.write_str("persistent identity kind does not match genericity")
            }
            Self::TypeParameterArity { expected, actual } => write!(
                formatter,
                "type-parameter arity is {actual}, expected {expected}"
            ),
            Self::InvalidOwner => formatter.write_str("callable owner shape is invalid"),
            Self::MissingNominalOwner => {
                formatter.write_str("nominal owner has no source identity")
            }
            Self::ForeignNominalOwner { expected, actual } => write!(
                formatter,
                "nominal owner belongs to Cone {actual}, not current Cone {expected}"
            ),
            Self::PersistentOwnerMismatch => {
                formatter.write_str("persistent owner chain does not match HIR owner")
            }
            Self::ConstructorOwnerMismatch => {
                formatter.write_str("constructor is not attached to its declared nominal owner")
            }
            Self::MissingPropertyInterface(property) => {
                write!(formatter, "property {property:?} has no public interface")
            }
            Self::PropertyOwnerMismatch => {
                formatter.write_str("accessor identity does not match its logical property")
            }
            Self::AccessorRole { expected, actual } => write!(
                formatter,
                "accessor identity role is {actual:?}, expected {expected:?}"
            ),
            Self::MissingInterfaceMethod(index) => {
                write!(formatter, "interface method entity {index} is unknown")
            }
            Self::InterfaceMethodMismatch => {
                formatter.write_str("interface method entity does not reference this function")
            }
            Self::AccessorAccessMismatch { expected, actual } => write!(
                formatter,
                "accessor access is {actual:?}, expected {expected:?} from its property"
            ),
            Self::Signature(source) => source.fmt(formatter),
            Self::Parameters(source) => source.fmt(formatter),
            Self::Access(source) => source.fmt(formatter),
            Self::Effects(source) => source.fmt(formatter),
            Self::Record(source) => source.fmt(formatter),
        }
    }
}

impl fmt::Display for CallableInterfaceBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PropertyInterfaces(source) => {
                write!(
                    formatter,
                    "cannot project callable property authority: {source}"
                )
            }
            Self::Projection { subject, error } => {
                write!(formatter, "cannot project public {subject}: {error}")
            }
            Self::Table(source) => source.fmt(formatter),
            Self::AccessorClosure(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CallableInterfaceBuildError {}
