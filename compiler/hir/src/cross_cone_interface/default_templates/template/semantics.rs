use std::fmt;

use scoop_identity::{CallableTemplateOrigin, Effect, SignatureTypeKey};

use super::ExportDefaultTemplateV1;
use crate::{
    BinderUseListSemanticValidationError, CallableInterfaceRecordV1,
    CallableInterfaceSemanticAuthority, CallableInterfaceSemanticValidationError,
    CallableSourceInterfaceV1, CanonicalBooleanV1, DefaultTemplateRootSemanticAuthority,
    DefaultTemplateRootSemanticValidationError, DefaultTemplateTypeSubstitutionError,
    SignatureTypeSemanticError, TemplateLocalScopeValidationError,
    TemplateLocalTypeSemanticValidationError, TemplateReceiverSemanticValidationError,
    TemplateValueParameterSemanticValidationError,
};

impl ExportDefaultTemplateV1 {
    /// Validates the declaration, provider-scope type, local-link, result, and
    /// suspend contract against callable/source interfaces that have already
    /// passed their table-level semantic validation. Definition origins, body
    /// operations, and the exact reference closure are validated separately.
    pub fn validate_contract_semantics<A, E>(
        &self,
        callable: &CallableInterfaceRecordV1,
        source: &CallableSourceInterfaceV1,
        authority: &mut A,
    ) -> Result<(), ExportDefaultTemplateContractSemanticValidationError<E>>
    where
        A: CallableInterfaceSemanticAuthority<E> + DefaultTemplateRootSemanticAuthority<E>,
    {
        let key = self.key();
        if callable.declaration() != key.owner() {
            return Err(
                ExportDefaultTemplateContractSemanticValidationError::CallableOwner {
                    expected: key.owner(),
                    actual: callable.declaration(),
                },
            );
        }
        if source.owner() != key.owner() {
            return Err(
                ExportDefaultTemplateContractSemanticValidationError::SourceOwner {
                    expected: key.owner(),
                    actual: source.owner(),
                },
            );
        }

        let position = key.parameter_position();
        let current_parameter = usize::try_from(position)
            .ok()
            .and_then(|index| source.parameters().parameters().get(index))
            .ok_or(
                ExportDefaultTemplateContractSemanticValidationError::ParameterOutOfRange {
                    position,
                    arity: source.parameters().len_u32(),
                },
            )?;

        let identity = callable
            .validate_identity_semantics(authority)
            .map_err(ExportDefaultTemplateContractSemanticValidationError::CallableInterface)?;
        let provider = self
            .definition_root()
            .validate_semantics(key, self.definition_path(), authority)
            .map_err(ExportDefaultTemplateContractSemanticValidationError::DefinitionRoot)?;
        let outer_arity = (identity.outer_type_parameter_arity() != 0)
            .then_some(identity.outer_type_parameter_arity());
        let key_owner_scope = callable.type_parameters().signature_scope(outer_arity);

        self.type_parameters()
            .validate_semantics(provider.binder_arity(), &key_owner_scope, authority)
            .map_err(ExportDefaultTemplateContractSemanticValidationError::TypeParameters)?;
        self.locals()
            .validate_definition_path(self.definition_path())
            .map_err(ExportDefaultTemplateContractSemanticValidationError::LocalScope)?;
        self.locals()
            .validate_type_semantics(provider, authority)
            .map_err(ExportDefaultTemplateContractSemanticValidationError::LocalTypes)?;
        self.receiver()
            .validate_semantics(callable, self.locals(), provider, self.type_parameters())
            .map_err(ExportDefaultTemplateContractSemanticValidationError::Receiver)?;
        self.value_parameters()
            .validate_semantics(key, source, self.locals(), provider, self.type_parameters())
            .map_err(ExportDefaultTemplateContractSemanticValidationError::ValueParameters)?;

        provider
            .signature_scope()
            .validate_signature_semantics(self.result(), authority)
            .map_err(ExportDefaultTemplateContractSemanticValidationError::ResultType)?;
        let mapped_result = self
            .type_parameters()
            .substitute_provider_type(provider, self.result())
            .map_err(ExportDefaultTemplateContractSemanticValidationError::ResultSubstitution)?;
        if &mapped_result != current_parameter.value_type() {
            return Err(
                ExportDefaultTemplateContractSemanticValidationError::ResultMismatch {
                    expected: Box::new(current_parameter.value_type().clone()),
                    actual: Box::new(mapped_result),
                },
            );
        }

        let expected_suspend =
            CanonicalBooleanV1::from(callable.effects().execution() == Effect::Suspend);
        if self.allows_suspend() != expected_suspend {
            return Err(
                ExportDefaultTemplateContractSemanticValidationError::SuspendPermission {
                    expected: expected_suspend,
                    actual: self.allows_suspend(),
                },
            );
        }
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultTemplateContractSemanticValidationError<E> {
    CallableOwner {
        expected: CallableTemplateOrigin,
        actual: CallableTemplateOrigin,
    },
    SourceOwner {
        expected: CallableTemplateOrigin,
        actual: CallableTemplateOrigin,
    },
    ParameterOutOfRange {
        position: u32,
        arity: u32,
    },
    CallableInterface(CallableInterfaceSemanticValidationError<E>),
    DefinitionRoot(DefaultTemplateRootSemanticValidationError<E>),
    TypeParameters(BinderUseListSemanticValidationError<E>),
    LocalScope(TemplateLocalScopeValidationError),
    LocalTypes(TemplateLocalTypeSemanticValidationError<E>),
    Receiver(TemplateReceiverSemanticValidationError),
    ValueParameters(TemplateValueParameterSemanticValidationError),
    ResultType(SignatureTypeSemanticError<E>),
    ResultSubstitution(DefaultTemplateTypeSubstitutionError),
    ResultMismatch {
        expected: Box<SignatureTypeKey>,
        actual: Box<SignatureTypeKey>,
    },
    SuspendPermission {
        expected: CanonicalBooleanV1,
        actual: CanonicalBooleanV1,
    },
}

impl<E: fmt::Display> fmt::Display for ExportDefaultTemplateContractSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CallableOwner { expected, actual } => write!(
                formatter,
                "default-template owner {expected:?} does not match callable interface {actual:?}"
            ),
            Self::SourceOwner { expected, actual } => write!(
                formatter,
                "default-template owner {expected:?} does not match source interface {actual:?}"
            ),
            Self::ParameterOutOfRange { position, arity } => write!(
                formatter,
                "default-template parameter position {position} is outside source arity {arity}"
            ),
            Self::CallableInterface(error) => {
                write!(
                    formatter,
                    "invalid default-template owner interface: {error}"
                )
            }
            Self::DefinitionRoot(error) => {
                write!(
                    formatter,
                    "invalid default-template definition root: {error}"
                )
            }
            Self::TypeParameters(error) => {
                write!(formatter, "invalid default-template type mapping: {error}")
            }
            Self::LocalScope(error) => {
                write!(formatter, "invalid default-template local scope: {error}")
            }
            Self::LocalTypes(error) => {
                write!(formatter, "invalid default-template local type: {error}")
            }
            Self::Receiver(error) => {
                write!(formatter, "invalid default-template receiver: {error}")
            }
            Self::ValueParameters(error) => {
                write!(
                    formatter,
                    "invalid default-template value parameters: {error}"
                )
            }
            Self::ResultType(error) => {
                write!(
                    formatter,
                    "invalid default-template provider result type: {error}"
                )
            }
            Self::ResultSubstitution(error) => {
                write!(
                    formatter,
                    "cannot map default-template result into owner scope: {error}"
                )
            }
            Self::ResultMismatch { expected, actual } => write!(
                formatter,
                "mapped default-template result type {actual:?} differs from source parameter type {expected:?}"
            ),
            Self::SuspendPermission { expected, actual } => write!(
                formatter,
                "default-template suspend permission is {actual:?}, owner callable requires {expected:?}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExportDefaultTemplateContractSemanticValidationError<E>
{
}

#[cfg(test)]
mod tests;
