use std::fmt;

use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{BudgetMeter, WirePath};

use super::CanonicalExportDefaultTemplatesV1;
use crate::{
    CallableInterfaceSemanticAuthority, CanonicalCallableInterfacesV1,
    CanonicalCallableSourceInterfacesV1, DefaultBodyProviderEnvelopeSemanticValidationError,
    DefaultReferenceSemanticAuthority, DefaultTemplateOriginSemanticAuthority,
    DefaultTemplateRootSemanticAuthority, ExportDefaultReferenceSetSemanticValidationError,
    ExportDefaultTemplateContractSemanticValidationError, ExportDefaultTemplateKeyV1,
    ExportDefaultTemplateOriginSemanticValidationError,
};

impl CanonicalExportDefaultTemplatesV1 {
    /// Validates every template envelope against callable/source tables that
    /// have already passed their own semantic validators, then proves the
    /// provider envelope of every body and declared reference set, then proves
    /// the exact bidirectional source-template closure. Operation typing,
    /// local data flow, nested callable ABI, and the exact body-to-reference
    /// closure remain separate passes.
    pub fn validate_envelope_semantics<A, E>(
        &self,
        callables: &CanonicalCallableInterfacesV1,
        sources: &CanonicalCallableSourceInterfacesV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), ExportDefaultTemplateSetEnvelopeSemanticValidationError<E>>
    where
        A: CallableInterfaceSemanticAuthority<E>
            + DefaultTemplateRootSemanticAuthority<E>
            + DefaultTemplateOriginSemanticAuthority<E>
            + DefaultReferenceSemanticAuthority<E>,
    {
        for (index, template) in self.records().iter().enumerate() {
            let key = template.key();
            let callable = callables.get(key.owner()).ok_or(
                ExportDefaultTemplateSetEnvelopeSemanticValidationError::MissingCallable {
                    index,
                    owner: key.owner(),
                },
            )?;
            let source = sources.get(key.owner()).ok_or(
                ExportDefaultTemplateSetEnvelopeSemanticValidationError::MissingSource {
                    index,
                    owner: key.owner(),
                },
            )?;
            let provider = template
                .validate_contract_semantics_with_provider(callable, source, authority)
                .map_err(|error| {
                    ExportDefaultTemplateSetEnvelopeSemanticValidationError::Contract {
                        index,
                        key,
                        error: Box::new(error),
                    }
                })?;
            template
                .body()
                .validate_provider_envelope_semantics(provider, authority, meter, path)
                .map_err(
                    |error| ExportDefaultTemplateSetEnvelopeSemanticValidationError::Body {
                        index,
                        key,
                        error: Box::new(error),
                    },
                )?;
            template
                .validate_origin_semantics(authority)
                .map_err(|error| {
                    ExportDefaultTemplateSetEnvelopeSemanticValidationError::Origin {
                        index,
                        key,
                        error: Box::new(error),
                    }
                })?;
            template
                .validate_reference_envelope_semantics(callable, provider, authority, meter, path)
                .map_err(|error| {
                    ExportDefaultTemplateSetEnvelopeSemanticValidationError::References {
                        index,
                        key,
                        error: Box::new(error),
                    }
                })?;
        }
        self.validate_source_closure(sources)
            .map_err(ExportDefaultTemplateSetEnvelopeSemanticValidationError::SourceClosure)
    }

    pub fn validate_source_closure(
        &self,
        sources: &CanonicalCallableSourceInterfacesV1,
    ) -> Result<(), ExportDefaultTemplateSourceClosureValidationError> {
        for (template_index, template) in self.records().iter().enumerate() {
            let key = template.key();
            let source = sources.get(key.owner()).ok_or(
                ExportDefaultTemplateSourceClosureValidationError::MissingSourceInterface {
                    template_index,
                    owner: key.owner(),
                },
            )?;
            let Some(parameter) = usize::try_from(key.parameter_position())
                .ok()
                .and_then(|position| source.parameters().parameters().get(position))
            else {
                return Err(
                    ExportDefaultTemplateSourceClosureValidationError::ParameterOutOfRange {
                        template_index,
                        key,
                        arity: source.parameters().len_u32(),
                    },
                );
            };
            let actual = parameter.calling().template();
            if actual != Some(key) {
                return Err(
                    ExportDefaultTemplateSourceClosureValidationError::ParameterReference {
                        template_index,
                        key,
                        actual,
                    },
                );
            }
        }

        for (source_index, source) in sources.records().iter().enumerate() {
            for (parameter_position, parameter) in (0_u32..).zip(source.parameters().parameters()) {
                let Some(key) = parameter.calling().template() else {
                    continue;
                };
                if self.get(key).is_none() {
                    return Err(
                        ExportDefaultTemplateSourceClosureValidationError::MissingTemplate {
                            source_index,
                            owner: source.owner(),
                            parameter_position,
                            key,
                        },
                    );
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExportDefaultTemplateSourceClosureValidationError {
    MissingSourceInterface {
        template_index: usize,
        owner: CallableTemplateOrigin,
    },
    ParameterOutOfRange {
        template_index: usize,
        key: ExportDefaultTemplateKeyV1,
        arity: u32,
    },
    ParameterReference {
        template_index: usize,
        key: ExportDefaultTemplateKeyV1,
        actual: Option<ExportDefaultTemplateKeyV1>,
    },
    MissingTemplate {
        source_index: usize,
        owner: CallableTemplateOrigin,
        parameter_position: u32,
        key: ExportDefaultTemplateKeyV1,
    },
}

impl fmt::Display for ExportDefaultTemplateSourceClosureValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSourceInterface {
                template_index,
                owner,
            } => write!(
                formatter,
                "export default template {template_index} has no source interface for owner {owner:?}"
            ),
            Self::ParameterOutOfRange {
                template_index,
                key,
                arity,
            } => write!(
                formatter,
                "export default template {template_index} key {key:?} is outside source arity {arity}"
            ),
            Self::ParameterReference {
                template_index,
                key,
                actual,
            } => write!(
                formatter,
                "export default template {template_index} key {key:?} is referenced as {actual:?} by its source parameter"
            ),
            Self::MissingTemplate {
                source_index,
                owner,
                parameter_position,
                key,
            } => write!(
                formatter,
                "source interface {source_index} owner {owner:?} parameter {parameter_position} references missing export default template {key:?}"
            ),
        }
    }
}

impl std::error::Error for ExportDefaultTemplateSourceClosureValidationError {}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultTemplateSetEnvelopeSemanticValidationError<E> {
    MissingCallable {
        index: usize,
        owner: CallableTemplateOrigin,
    },
    MissingSource {
        index: usize,
        owner: CallableTemplateOrigin,
    },
    Contract {
        index: usize,
        key: ExportDefaultTemplateKeyV1,
        error: Box<ExportDefaultTemplateContractSemanticValidationError<E>>,
    },
    Origin {
        index: usize,
        key: ExportDefaultTemplateKeyV1,
        error: Box<ExportDefaultTemplateOriginSemanticValidationError<E>>,
    },
    Body {
        index: usize,
        key: ExportDefaultTemplateKeyV1,
        error: Box<DefaultBodyProviderEnvelopeSemanticValidationError<E>>,
    },
    References {
        index: usize,
        key: ExportDefaultTemplateKeyV1,
        error: Box<ExportDefaultReferenceSetSemanticValidationError<E>>,
    },
    SourceClosure(ExportDefaultTemplateSourceClosureValidationError),
}

impl<E: fmt::Display> fmt::Display for ExportDefaultTemplateSetEnvelopeSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCallable { index, owner } => write!(
                formatter,
                "export default template {index} owner {owner:?} has no callable interface"
            ),
            Self::MissingSource { index, owner } => write!(
                formatter,
                "export default template {index} owner {owner:?} has no source interface"
            ),
            Self::Contract { index, key, error } => write!(
                formatter,
                "invalid export default template {key:?} contract at index {index}: {error}"
            ),
            Self::Origin { index, key, error } => write!(
                formatter,
                "invalid export default template {key:?} origin at index {index}: {error}"
            ),
            Self::Body { index, key, error } => write!(
                formatter,
                "invalid export default template {key:?} body at index {index}: {error}"
            ),
            Self::References { index, key, error } => write!(
                formatter,
                "invalid export default template {key:?} references at index {index}: {error}"
            ),
            Self::SourceClosure(error) => {
                write!(
                    formatter,
                    "invalid default-template source closure: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExportDefaultTemplateSetEnvelopeSemanticValidationError<E>
{
}

#[cfg(test)]
mod tests;
