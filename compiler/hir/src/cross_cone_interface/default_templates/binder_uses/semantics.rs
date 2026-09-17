use std::fmt;

use scoop_identity::{NonEmptyVec, SignatureTypeKey};

use crate::{
    DefaultTemplateProviderShapeV1, NominalInterfaceShapeAuthority, SignatureBinderScopeError,
    SignatureBinderScopeV1, SignatureTypeSemanticError,
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

    pub fn substitute_provider_type(
        &self,
        provider: DefaultTemplateProviderShapeV1,
        signature: &SignatureTypeKey,
    ) -> Result<SignatureTypeKey, DefaultTemplateTypeSubstitutionError> {
        if self.len_u32() != provider.binder_arity() {
            return Err(DefaultTemplateTypeSubstitutionError::MappingArity {
                expected: provider.binder_arity(),
                actual: self.len_u32(),
            });
        }
        substitute_type(self.arguments(), provider, signature)
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultTemplateTypeSubstitutionError {
    MappingArity { expected: u32, actual: u32 },
    ProviderBinder(SignatureBinderScopeError),
    MissingMapping { position: u32, len: u32 },
}

impl fmt::Display for DefaultTemplateTypeSubstitutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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

fn substitute_type(
    arguments: &[SignatureTypeKey],
    provider: DefaultTemplateProviderShapeV1,
    signature: &SignatureTypeKey,
) -> Result<SignatureTypeKey, DefaultTemplateTypeSubstitutionError> {
    match signature {
        SignatureTypeKey::Nominal(declaration) => Ok(SignatureTypeKey::Nominal(*declaration)),
        SignatureTypeKey::NominalApplication {
            origin,
            arguments: type_arguments,
        } => Ok(SignatureTypeKey::NominalApplication {
            origin: *origin,
            arguments: substitute_non_empty(arguments, provider, type_arguments)?,
        }),
        SignatureTypeKey::Tuple(elements) => Ok(SignatureTypeKey::Tuple(substitute_non_empty(
            arguments, provider, elements,
        )?)),
        SignatureTypeKey::Function {
            effect,
            parameters,
            result,
        } => Ok(SignatureTypeKey::Function {
            effect: *effect,
            parameters: substitute_sequence(arguments, provider, parameters)?,
            result: Box::new(substitute_type(arguments, provider, result)?),
        }),
        SignatureTypeKey::RawPointer(pointee) => Ok(SignatureTypeKey::RawPointer(Box::new(
            substitute_type(arguments, provider, pointee)?,
        ))),
        SignatureTypeKey::NativeFunctionPointer {
            calling_convention,
            parameters,
            result,
        } => Ok(SignatureTypeKey::NativeFunctionPointer {
            calling_convention: *calling_convention,
            parameters: substitute_sequence(arguments, provider, parameters)?,
            result: Box::new(substitute_type(arguments, provider, result)?),
        }),
        SignatureTypeKey::Binder { depth, index } => {
            let position = provider
                .flattened_binder_position(*depth, *index)
                .map_err(DefaultTemplateTypeSubstitutionError::ProviderBinder)?;
            usize::try_from(position)
                .ok()
                .and_then(|position| arguments.get(position))
                .cloned()
                .ok_or(DefaultTemplateTypeSubstitutionError::MissingMapping {
                    position,
                    len: provider.binder_arity(),
                })
        }
    }
}

fn substitute_sequence(
    arguments: &[SignatureTypeKey],
    provider: DefaultTemplateProviderShapeV1,
    signatures: &[SignatureTypeKey],
) -> Result<Vec<SignatureTypeKey>, DefaultTemplateTypeSubstitutionError> {
    signatures
        .iter()
        .map(|signature| substitute_type(arguments, provider, signature))
        .collect()
}

fn substitute_non_empty(
    arguments: &[SignatureTypeKey],
    provider: DefaultTemplateProviderShapeV1,
    signatures: &NonEmptyVec<SignatureTypeKey>,
) -> Result<NonEmptyVec<SignatureTypeKey>, DefaultTemplateTypeSubstitutionError> {
    let values = signatures.as_slice();
    let first = substitute_type(arguments, provider, &values[0])?;
    let rest = substitute_sequence(arguments, provider, &values[1..])?;
    Ok(NonEmptyVec::from_first(first, rest))
}

#[cfg(test)]
mod tests;
