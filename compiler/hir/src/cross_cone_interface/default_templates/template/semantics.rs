use std::fmt;

use scoop_identity::{
    CallableTemplateOrigin, Effect, LocalValueSelector, SignatureTypeKey, StructuralDefinitionPath,
};

use super::ExportDefaultTemplateV1;
use crate::{
    BinderUseListSemanticValidationError, CallableInterfaceRecordV1,
    CallableInterfaceSemanticAuthority, CallableInterfaceSemanticValidationError,
    CallableSourceInterfaceV1, CanonicalBooleanV1, DefaultTemplateProviderShapeV1,
    DefaultTemplateRootSemanticAuthority, DefaultTemplateRootSemanticValidationError,
    DefaultTemplateTypeSubstitutionError, ExportDefaultTemplateKeyV1,
    ExportDefinitionSourceSemanticAuthority, ExportDefinitionSourceSemanticValidationError,
    ExportDefinitionSourceV1, PersistentLexicalRootV1, SignatureTypeSemanticError,
    TemplateLocalDefinitionV1, TemplateLocalScopeValidationError,
    TemplateLocalTypeSemanticValidationError, TemplateReceiverSemanticValidationError,
    TemplateValueParameterSemanticValidationError,
};

/// Supplies foundation subject relations for definition origins owned by one
/// exported default template.
pub trait DefaultTemplateOriginSemanticAuthority<E>:
    ExportDefinitionSourceSemanticAuthority<E>
{
    fn validate_default_template_origin(
        &mut self,
        key: ExportDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        origin: &ExportDefinitionSourceV1,
    ) -> Result<(), E>;

    fn validate_default_template_local_origin(
        &mut self,
        key: ExportDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        selector: &LocalValueSelector,
        origin: &ExportDefinitionSourceV1,
    ) -> Result<(), E>;
}

impl ExportDefaultTemplateV1 {
    /// Validates the template-level origin and source-backed local origins.
    /// Body-node and reference-record origins are validated by their own
    /// recursive semantic passes.
    pub fn validate_origin_semantics<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), ExportDefaultTemplateOriginSemanticValidationError<E>>
    where
        A: DefaultTemplateOriginSemanticAuthority<E>,
    {
        self.definition_origin()
            .validate_semantics(authority)
            .map_err(ExportDefaultTemplateOriginSemanticValidationError::DefinitionSource)?;
        authority
            .validate_default_template_origin(
                self.key(),
                self.definition_root(),
                self.definition_path(),
                self.definition_origin(),
            )
            .map_err(ExportDefaultTemplateOriginSemanticValidationError::DefinitionRelation)?;

        for (index, local) in self.locals().records().iter().enumerate() {
            let TemplateLocalDefinitionV1::Source(origin) = local.definition() else {
                continue;
            };
            origin.validate_semantics(authority).map_err(|error| {
                ExportDefaultTemplateOriginSemanticValidationError::LocalSource {
                    index,
                    selector: local.selector().clone(),
                    error,
                }
            })?;
            authority
                .validate_default_template_local_origin(
                    self.key(),
                    self.definition_root(),
                    self.definition_path(),
                    local.selector(),
                    origin,
                )
                .map_err(|error| {
                    ExportDefaultTemplateOriginSemanticValidationError::LocalRelation {
                        index,
                        selector: local.selector().clone(),
                        error,
                    }
                })?;
        }
        Ok(())
    }

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
        self.validate_contract_semantics_with_provider(callable, source, authority)
            .map(|_| ())
    }

    pub(crate) fn validate_contract_semantics_with_provider<A, E>(
        &self,
        callable: &CallableInterfaceRecordV1,
        source: &CallableSourceInterfaceV1,
        authority: &mut A,
    ) -> Result<
        DefaultTemplateProviderShapeV1,
        ExportDefaultTemplateContractSemanticValidationError<E>,
    >
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
        Ok(provider)
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

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultTemplateOriginSemanticValidationError<E> {
    DefinitionSource(ExportDefinitionSourceSemanticValidationError<E>),
    DefinitionRelation(E),
    LocalSource {
        index: usize,
        selector: LocalValueSelector,
        error: ExportDefinitionSourceSemanticValidationError<E>,
    },
    LocalRelation {
        index: usize,
        selector: LocalValueSelector,
        error: E,
    },
}

impl<E: fmt::Display> fmt::Display for ExportDefaultTemplateOriginSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DefinitionSource(error) => {
                write!(
                    formatter,
                    "invalid default-template definition source: {error}"
                )
            }
            Self::DefinitionRelation(error) => write!(
                formatter,
                "default-template definition source does not match its foundation subject: {error}"
            ),
            Self::LocalSource {
                index,
                selector,
                error,
            } => write!(
                formatter,
                "invalid default-template local {selector:?} definition source at index {index}: {error}"
            ),
            Self::LocalRelation {
                index,
                selector,
                error,
            } => write!(
                formatter,
                "default-template local {selector:?} definition source at index {index} does not match its foundation subject: {error}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExportDefaultTemplateOriginSemanticValidationError<E>
{
}

#[cfg(test)]
mod tests;
