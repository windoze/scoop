//! Projection of request-local HIR source positions into persistent origins.

use std::fmt;

use scoop_identity::{ConeIdentity, DefinitionOrigin, SourceOriginError, SourceSpan};

use crate::{ExportDefinitionSourceV1, ExportHir, IntrinsicProviderId};

pub(super) fn project_definition_source(
    export: &ExportHir,
    origin: crate::DefinitionOrigin,
) -> Result<ExportDefinitionSourceV1, HirDefinitionSourceProjectionError> {
    let file_index = usize::try_from(origin.file)
        .map_err(|_| HirDefinitionSourceProjectionError::UnknownFile(origin.file))?;
    let source = export
        .source_files
        .get(file_index)
        .ok_or(HirDefinitionSourceProjectionError::UnknownFile(origin.file))?;
    if source.provider != origin.provider {
        return Err(HirDefinitionSourceProjectionError::ProviderMismatch {
            file: origin.file,
            expected: source.provider,
            actual: origin.provider,
        });
    }
    let actual_cone = source.identity.cone();
    if actual_cone != export.cone {
        return Err(HirDefinitionSourceProjectionError::ForeignSource {
            expected: export.cone,
            actual: actual_cone,
        });
    }
    let context_index = raw_index(origin.context);
    let local_context = arena_get(&export.source_contexts, origin.context).ok_or(
        HirDefinitionSourceProjectionError::UnknownContext(context_index),
    )?;
    if local_context.source() != &source.identity {
        return Err(
            HirDefinitionSourceProjectionError::LocalContextSourceMismatch {
                file: origin.file,
                context: context_index,
            },
        );
    }
    let context = export
        .source_context_identities
        .get(origin.context)
        .ok_or(HirDefinitionSourceProjectionError::MissingPersistentContext(context_index))?;
    if context.key().source() != &source.identity {
        return Err(
            HirDefinitionSourceProjectionError::PersistentContextSourceMismatch {
                file: origin.file,
                context: context_index,
            },
        );
    }
    let span = SourceSpan::new(u64::from(origin.span.start), u64::from(origin.span.end))
        .map_err(HirDefinitionSourceProjectionError::Span)?;
    DefinitionOrigin::new(source.identity.clone(), span, context.key())
        .map(ExportDefinitionSourceV1::new)
        .map_err(HirDefinitionSourceProjectionError::Origin)
}

fn arena_get<T>(arena: &la_arena::Arena<T>, id: la_arena::Idx<T>) -> Option<&T> {
    ((raw_index(id) as usize) < arena.len()).then(|| &arena[id])
}

fn raw_index<T>(id: la_arena::Idx<T>) -> u32 {
    id.into_raw().into_u32()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirDefinitionSourceProjectionError {
    UnknownFile(u32),
    ProviderMismatch {
        file: u32,
        expected: IntrinsicProviderId,
        actual: IntrinsicProviderId,
    },
    ForeignSource {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    UnknownContext(u32),
    MissingPersistentContext(u32),
    LocalContextSourceMismatch {
        file: u32,
        context: u32,
    },
    PersistentContextSourceMismatch {
        file: u32,
        context: u32,
    },
    Span(scoop_identity::SourceSpanError),
    Origin(SourceOriginError),
}

impl fmt::Display for HirDefinitionSourceProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFile(file) => write!(formatter, "source file {file} is unknown"),
            Self::ProviderMismatch {
                file,
                expected,
                actual,
            } => write!(
                formatter,
                "source file {file} belongs to provider {}, not provider {}",
                expected.into_raw(),
                actual.into_raw()
            ),
            Self::ForeignSource { expected, actual } => write!(
                formatter,
                "definition source belongs to Cone {actual}, not current Cone {expected}"
            ),
            Self::UnknownContext(context) => {
                write!(formatter, "source context {context} is unknown")
            }
            Self::MissingPersistentContext(context) => write!(
                formatter,
                "source context {context} has no persistent identity"
            ),
            Self::LocalContextSourceMismatch { file, context } => write!(
                formatter,
                "source context {context} does not belong to source file {file}"
            ),
            Self::PersistentContextSourceMismatch { file, context } => write!(
                formatter,
                "persistent source context {context} does not belong to source file {file}"
            ),
            Self::Span(source) => write!(formatter, "invalid source span: {source}"),
            Self::Origin(source) => write!(formatter, "invalid persistent source origin: {source}"),
        }
    }
}

impl std::error::Error for HirDefinitionSourceProjectionError {}
