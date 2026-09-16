use std::fmt;

use scoop_identity::{PersistentGenericTypeId, PersistentTypeId, SignatureTypeKey};

use super::{
    CanonicalBinderListV1, SignatureBinderScopeError, SignatureBinderScopeV1,
    TypeParameterBoundLocation, TypeParameterBoundsV1,
};
use crate::PublicNominalKindV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicNominalShapeV1 {
    kind: PublicNominalKindV1,
    type_parameter_arity: u32,
}

impl PublicNominalShapeV1 {
    pub const fn new(kind: PublicNominalKindV1, type_parameter_arity: u32) -> Self {
        Self {
            kind,
            type_parameter_arity,
        }
    }

    pub const fn kind(self) -> PublicNominalKindV1 {
        self.kind
    }

    pub const fn type_parameter_arity(self) -> u32 {
        self.type_parameter_arity
    }
}

pub trait NominalInterfaceShapeAuthority<E> {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, E>;

    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, E>;
}

impl SignatureBinderScopeV1 {
    pub fn validate_signature_semantics<A, E>(
        &self,
        signature: &SignatureTypeKey,
        authority: &mut A,
    ) -> Result<(), SignatureTypeSemanticError<E>>
    where
        A: NominalInterfaceShapeAuthority<E>,
    {
        match signature {
            SignatureTypeKey::Nominal(declaration) => {
                validate_concrete_nominal(*declaration, authority).map(|_| ())
            }
            SignatureTypeKey::NominalApplication { origin, arguments } => {
                validate_generic_nominal(*origin, arguments.as_slice(), self, authority).map(|_| ())
            }
            SignatureTypeKey::Tuple(elements) => {
                self.validate_sequence_semantics(elements.as_slice(), authority)
            }
            SignatureTypeKey::Function {
                parameters, result, ..
            }
            | SignatureTypeKey::NativeFunctionPointer {
                parameters, result, ..
            } => {
                self.validate_sequence_semantics(parameters, authority)?;
                self.validate_signature_semantics(result, authority)
            }
            SignatureTypeKey::RawPointer(pointee) => {
                self.validate_signature_semantics(pointee, authority)
            }
            SignatureTypeKey::Binder { .. } => self
                .validate(signature)
                .map_err(SignatureTypeSemanticError::BinderScope),
        }
    }

    pub fn validate_nominal_signature_semantics<A, E>(
        &self,
        signature: &SignatureTypeKey,
        authority: &mut A,
    ) -> Result<PublicNominalShapeV1, NominalSignatureSemanticError<E>>
    where
        A: NominalInterfaceShapeAuthority<E>,
    {
        match signature {
            SignatureTypeKey::Nominal(declaration) => {
                validate_concrete_nominal(*declaration, authority)
            }
            SignatureTypeKey::NominalApplication { origin, arguments } => {
                validate_generic_nominal(*origin, arguments.as_slice(), self, authority)
            }
            signature => {
                return Err(NominalSignatureSemanticError::NonNominal {
                    actual: SignatureTypeFormV1::of(signature),
                });
            }
        }
        .map_err(NominalSignatureSemanticError::Signature)
    }

    fn validate_sequence_semantics<A, E>(
        &self,
        signatures: &[SignatureTypeKey],
        authority: &mut A,
    ) -> Result<(), SignatureTypeSemanticError<E>>
    where
        A: NominalInterfaceShapeAuthority<E>,
    {
        for signature in signatures {
            self.validate_signature_semantics(signature, authority)?;
        }
        Ok(())
    }
}

impl CanonicalBinderListV1 {
    pub fn validate_bound_semantics<A, E>(
        &self,
        nominal_owner_arity: Option<u32>,
        authority: &mut A,
    ) -> Result<(), TypeParameterBinderSemanticValidationError<E>>
    where
        A: NominalInterfaceShapeAuthority<E>,
    {
        let scope = self.signature_scope(nominal_owner_arity);
        for (binder_index, binder) in self.binders().iter().enumerate() {
            let TypeParameterBoundsV1::Nominal(bounds) = binder.bounds() else {
                continue;
            };
            if let Some(class) = bounds.class() {
                validate_nominal_bound(class, PublicNominalKindV1::Class, &scope, authority)
                    .map_err(|error| TypeParameterBinderSemanticValidationError {
                        binder_index,
                        bound: TypeParameterBoundLocation::Class,
                        error,
                    })?;
            }
            for (interface_index, interface) in bounds.interfaces().values().iter().enumerate() {
                validate_nominal_bound(
                    interface,
                    PublicNominalKindV1::Interface,
                    &scope,
                    authority,
                )
                .map_err(|error| TypeParameterBinderSemanticValidationError {
                    binder_index,
                    bound: TypeParameterBoundLocation::Interface { interface_index },
                    error,
                })?;
            }
        }
        Ok(())
    }
}

fn validate_nominal_bound<A, E>(
    signature: &SignatureTypeKey,
    expected: PublicNominalKindV1,
    scope: &SignatureBinderScopeV1,
    authority: &mut A,
) -> Result<(), NominalBoundSemanticError<E>>
where
    A: NominalInterfaceShapeAuthority<E>,
{
    let shape = scope
        .validate_nominal_signature_semantics(signature, authority)
        .map_err(|error| match error {
            NominalSignatureSemanticError::NonNominal { actual } => {
                NominalBoundSemanticError::NonNominal { actual }
            }
            NominalSignatureSemanticError::Signature(error) => {
                NominalBoundSemanticError::Signature(error)
            }
        })?;

    if shape.kind() != expected {
        return Err(NominalBoundSemanticError::Kind {
            expected,
            actual: shape.kind(),
        });
    }
    Ok(())
}

fn validate_concrete_nominal<A, E>(
    declaration: PersistentTypeId,
    authority: &mut A,
) -> Result<PublicNominalShapeV1, SignatureTypeSemanticError<E>>
where
    A: NominalInterfaceShapeAuthority<E>,
{
    let shape = authority
        .concrete_nominal_shape(declaration)
        .map_err(SignatureTypeSemanticError::Reference)?;
    if shape.type_parameter_arity() != 0 {
        return Err(SignatureTypeSemanticError::ConcreteNominalArity {
            declaration,
            actual: shape.type_parameter_arity(),
        });
    }
    Ok(shape)
}

fn validate_generic_nominal<A, E>(
    declaration: PersistentGenericTypeId,
    arguments: &[SignatureTypeKey],
    scope: &SignatureBinderScopeV1,
    authority: &mut A,
) -> Result<PublicNominalShapeV1, SignatureTypeSemanticError<E>>
where
    A: NominalInterfaceShapeAuthority<E>,
{
    let shape = authority
        .generic_nominal_shape(declaration)
        .map_err(SignatureTypeSemanticError::Reference)?;
    if usize::try_from(shape.type_parameter_arity()).ok() != Some(arguments.len()) {
        return Err(SignatureTypeSemanticError::GenericNominalArity {
            declaration,
            expected: shape.type_parameter_arity(),
            actual: arguments.len(),
        });
    }
    scope.validate_sequence_semantics(arguments, authority)?;
    Ok(shape)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignatureTypeFormV1 {
    Nominal,
    NominalApplication,
    Tuple,
    Function,
    RawPointer,
    NativeFunctionPointer,
    Binder,
}

impl SignatureTypeFormV1 {
    pub const fn of(signature: &SignatureTypeKey) -> Self {
        match signature {
            SignatureTypeKey::Nominal(_) => Self::Nominal,
            SignatureTypeKey::NominalApplication { .. } => Self::NominalApplication,
            SignatureTypeKey::Tuple(_) => Self::Tuple,
            SignatureTypeKey::Function { .. } => Self::Function,
            SignatureTypeKey::RawPointer(_) => Self::RawPointer,
            SignatureTypeKey::NativeFunctionPointer { .. } => Self::NativeFunctionPointer,
            SignatureTypeKey::Binder { .. } => Self::Binder,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum NominalSignatureSemanticError<E> {
    NonNominal { actual: SignatureTypeFormV1 },
    Signature(SignatureTypeSemanticError<E>),
}

impl<E: fmt::Display> fmt::Display for NominalSignatureSemanticError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonNominal { actual } => {
                write!(formatter, "expected a nominal signature, found {actual:?}")
            }
            Self::Signature(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for NominalSignatureSemanticError<E> {}

#[derive(Debug, Eq, PartialEq)]
pub enum SignatureTypeSemanticError<E> {
    BinderScope(SignatureBinderScopeError),
    Reference(E),
    ConcreteNominalArity {
        declaration: PersistentTypeId,
        actual: u32,
    },
    GenericNominalArity {
        declaration: PersistentGenericTypeId,
        expected: u32,
        actual: usize,
    },
}

impl<E: fmt::Display> fmt::Display for SignatureTypeSemanticError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BinderScope(error) => error.fmt(formatter),
            Self::Reference(error) => write!(formatter, "invalid nominal reference: {error}"),
            Self::ConcreteNominalArity {
                declaration,
                actual,
            } => write!(
                formatter,
                "concrete nominal {declaration} declares {actual} type parameters"
            ),
            Self::GenericNominalArity {
                declaration,
                expected,
                actual,
            } => write!(
                formatter,
                "generic nominal {declaration} expects {expected} type arguments, found {actual}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for SignatureTypeSemanticError<E> {}

#[derive(Debug, Eq, PartialEq)]
pub enum NominalBoundSemanticError<E> {
    NonNominal {
        actual: SignatureTypeFormV1,
    },
    Signature(SignatureTypeSemanticError<E>),
    Kind {
        expected: PublicNominalKindV1,
        actual: PublicNominalKindV1,
    },
}

impl<E: fmt::Display> fmt::Display for NominalBoundSemanticError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonNominal { actual } => {
                write!(formatter, "nominal bound has non-nominal form {actual:?}")
            }
            Self::Signature(error) => error.fmt(formatter),
            Self::Kind { expected, actual } => write!(
                formatter,
                "nominal bound has kind {actual:?}, expected {expected:?}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for NominalBoundSemanticError<E> {}

#[derive(Debug, Eq, PartialEq)]
pub struct TypeParameterBinderSemanticValidationError<E> {
    pub binder_index: usize,
    pub bound: TypeParameterBoundLocation,
    pub error: NominalBoundSemanticError<E>,
}

impl<E: fmt::Display> fmt::Display for TypeParameterBinderSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "type parameter {} has an invalid {}: {}",
            self.binder_index, self.bound, self.error
        )
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for TypeParameterBinderSemanticValidationError<E>
{
}

#[cfg(test)]
mod tests;
