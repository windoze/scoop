//! Definition locations shared by inlined dependency expressions.
//! The dependency reader has already checked source and context records.

use std::fmt;

use scoop_ast::Span;
use scoop_hir as hir;
use scoop_identity::{PersistentSourceContextId, SourceContextKey};

use crate::Lowerer;

impl Lowerer {
    pub(in crate::expr) fn import_dependency_definition_origin(
        &mut self,
        source: &hir::ExportDefinitionSourceV1,
        imported: hir::ImportedDependencyDefinitionSource<'_>,
    ) -> Result<hir::DefinitionOrigin, ImportedDefinitionOriginError> {
        let origin = source.origin();
        let span = origin.span();
        let provider = self.imported_source_provider(origin.source().cone())?;
        let file = self.imported_source_file(provider, imported.record())?;
        self.intern_imported_source_context(SourceContextKey::File {
            source: origin.source().clone(),
        });
        let context = self.intern_imported_source_context(imported.context().clone());
        Ok(hir::DefinitionOrigin {
            provider,
            file,
            span: Span {
                start: u32::try_from(span.start_byte())
                    .map_err(|_| ImportedDefinitionOriginError::SpanOverflow)?,
                end: u32::try_from(span.end_byte())
                    .map_err(|_| ImportedDefinitionOriginError::SpanOverflow)?,
            },
            context,
        })
    }

    fn imported_source_provider(
        &self,
        cone: scoop_identity::ConeIdentity,
    ) -> Result<hir::IntrinsicProviderId, ImportedDefinitionOriginError> {
        if let Some(provider) = self
            .imported_source_files
            .iter()
            .find(|source| source.identity.cone() == cone)
            .map(|source| source.provider)
        {
            return Ok(provider);
        }
        let next = self
            .intrinsic_sources
            .iter()
            .map(|source| source.provider.into_raw())
            .chain(
                self.imported_source_files
                    .iter()
                    .map(|source| source.provider.into_raw()),
            )
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(ImportedDefinitionOriginError::ProviderOverflow)?;
        Ok(hir::IntrinsicProviderId::from_raw(next))
    }

    fn imported_source_file(
        &mut self,
        provider: hir::IntrinsicProviderId,
        record: &hir::SourceRecord,
    ) -> Result<u32, ImportedDefinitionOriginError> {
        if let Some(&file) = self.imported_source_indices.get(record.identity()) {
            return Ok(file);
        }
        let index = self
            .intrinsic_sources
            .len()
            .checked_add(self.imported_source_files.len())
            .and_then(|index| u32::try_from(index).ok())
            .ok_or(ImportedDefinitionOriginError::SourceIndexOverflow)?;
        self.imported_source_files.push(hir::SourceFileMetadata {
            provider,
            identity: record.identity().clone(),
            name: record.identity().logical_path().as_str().to_owned(),
            source: String::new(),
            canonical_record: Some(record.clone()),
        });
        self.imported_source_indices
            .insert(record.identity().clone(), index);
        Ok(index)
    }

    fn intern_imported_source_context(&mut self, key: SourceContextKey) -> hir::SourceContextId {
        let context = hir::SourceContext::new(
            key.source().clone(),
            hir::SourceContextSubject::Imported(key),
        );
        self.source_context_by_value
            .get(&context)
            .copied()
            .unwrap_or_else(|| {
                let id = self.source_contexts.alloc(context.clone());
                self.source_context_by_value.insert(context, id);
                id
            })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::expr) enum ImportedDefinitionOriginError {
    MissingSource { context: PersistentSourceContextId },
    SpanOverflow,
    ProviderOverflow,
    SourceIndexOverflow,
}

impl fmt::Display for ImportedDefinitionOriginError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSource { context } => write!(
                formatter,
                "dependency definition origin context {context:?} has no source metadata"
            ),
            Self::SpanOverflow => {
                formatter.write_str("dependency source span exceeds the HIR u32 domain")
            }
            Self::ProviderOverflow => {
                formatter.write_str("dependency source provider id space is exhausted")
            }
            Self::SourceIndexOverflow => {
                formatter.write_str("dependency source file index space is exhausted")
            }
        }
    }
}

impl std::error::Error for ImportedDefinitionOriginError {}
