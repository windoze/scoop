use std::fmt;

use crate::{
    NominalInterfaceShapeAuthority, SignatureBinderScopeError, SignatureBinderScopeV1,
    SignatureTypeSemanticError,
};

use super::CanonicalBinderUseListV1;

impl CanonicalBinderUseListV1 {
    pub fn validate_semantics<A, E>(
        &self,
        provider_binder_arity: u32,
        key_owner_scope: &SignatureBinderScopeV1,
        authority: &mut A,
    ) -> Result<(), BinderUseListSemanticValidationError<E>>
    where
        A: NominalInterfaceShapeAuthority<E>,
    {
        if self.len_u32() != provider_binder_arity {
            return Err(BinderUseListSemanticValidationError::Arity {
                expected: provider_binder_arity,
                actual: self.len_u32(),
            });
        }
        for (index, argument) in self.arguments().iter().enumerate() {
            key_owner_scope
                .validate_signature_semantics(argument, authority)
                .map_err(|error| BinderUseListSemanticValidationError::Argument { index, error })?;
        }
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum BinderUseListSemanticValidationError<E> {
    Arity {
        expected: u32,
        actual: u32,
    },
    Argument {
        index: usize,
        error: SignatureTypeSemanticError<E>,
    },
}

impl<E: fmt::Display> fmt::Display for BinderUseListSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arity { expected, actual } => write!(
                formatter,
                "default-template provider has {expected} binders, mapping has {actual} arguments"
            ),
            Self::Argument { index, error } => write!(
                formatter,
                "invalid default-template binder mapping argument {index}: {error}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for BinderUseListSemanticValidationError<E> {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DefaultTemplateTypeSubstitutionError {
    Wire(scoop_wire::WireError),
    MappingArity { expected: u32, actual: u32 },
    ProviderBinder(SignatureBinderScopeError),
    MissingMapping { position: u32, len: u32 },
}

impl fmt::Display for DefaultTemplateTypeSubstitutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(error) => error.fmt(formatter),
            Self::MappingArity { expected, actual } => write!(
                formatter,
                "default-template provider has {expected} binders, mapping has {actual} arguments"
            ),
            Self::ProviderBinder(error) => {
                write!(formatter, "invalid provider binder reference: {error}")
            }
            Self::MissingMapping { position, len } => write!(
                formatter,
                "default-template provider binder position {position} is outside mapping length {len}"
            ),
        }
    }
}

impl std::error::Error for DefaultTemplateTypeSubstitutionError {}

#[cfg(test)]
mod tests;

impl From<scoop_wire::WireError> for DefaultTemplateTypeSubstitutionError {
    fn from(error: scoop_wire::WireError) -> Self {
        Self::Wire(error)
    }
}
