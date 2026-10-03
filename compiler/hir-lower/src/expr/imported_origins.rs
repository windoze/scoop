//! Definition locations shared by inlined dependency expressions.
//! The dependency reader has already checked source and context records.

use std::fmt;

use scoop_ast::Span;
use scoop_hir as hir;
use scoop_identity::{PersistentSourceContextId, SourceContextKey};

use crate::Lowerer;

impl Lowerer {
    pub(crate) fn import_dependency_definition_origin(
        &mut self,
        source: &hir::ExportDefinitionSourceV1,
        imported: hir::ImportedDependencyDefinitionSource<'_>,
    ) -> Result<hir::DefinitionOrigin, ImportedDefinitionOriginError> {
        let (provider, file, span, context) =
            self.import_dependency_source_location(source.origin().span(), imported)?;
        Ok(hir::DefinitionOrigin {
            provider,
            file,
            span,
            context,
        })
    }

    pub(crate) fn import_dependency_evaluation_origin(
        &mut self,
        source: &scoop_identity::EvaluationOrigin,
        imported: hir::ImportedDependencyDefinitionSource<'_>,
    ) -> Result<hir::EvaluationOrigin, ImportedDefinitionOriginError> {
        let (provider, file, span, context) =
            self.import_dependency_source_location(source.span(), imported)?;
        Ok(hir::EvaluationOrigin {
            provider,
            file,
            span,
            context,
        })
    }

    fn import_dependency_source_location(
        &mut self,
        span: scoop_identity::SourceSpan,
        imported: hir::ImportedDependencyDefinitionSource<'_>,
    ) -> Result<
        (hir::IntrinsicProviderId, u32, Span, hir::SourceContextId),
        ImportedDefinitionOriginError,
    > {
        let provider = self.imported_source_provider(imported.record().identity().cone())?;
        let file = self.imported_source_file(provider, imported.record())?;
        self.intern_imported_source_context(
            SourceContextKey::File {
                source: imported.record().identity().clone(),
            },
            hir::SourceContextNames::default(),
        );
        let context = self
            .intern_imported_source_context(imported.context().clone(), imported.names().clone());
        let span = Span {
            start: u32::try_from(span.start_byte())
                .map_err(|_| ImportedDefinitionOriginError::SpanOverflow)?,
            end: u32::try_from(span.end_byte())
                .map_err(|_| ImportedDefinitionOriginError::SpanOverflow)?,
        };
        Ok((provider, file, span, context))
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
        let logical_path = record.identity().logical_path().as_str();
        let name = match self.source_names.get(&record.identity().cone()) {
            Some(coordinate) => format!("{coordinate}/{logical_path}"),
            None => logical_path.to_owned(),
        };
        self.imported_source_files.push(hir::SourceFileMetadata {
            provider,
            identity: record.identity().clone(),
            name,
            source: String::new(),
            canonical_record: Some(record.clone()),
        });
        self.imported_source_indices
            .insert(record.identity().clone(), index);
        Ok(index)
    }

    pub(crate) fn intern_imported_source_context(
        &mut self,
        key: SourceContextKey,
        names: hir::SourceContextNames,
    ) -> hir::SourceContextId {
        let context = hir::SourceContext::new(
            key.source().clone(),
            hir::SourceContextSubject::Imported { key, names },
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
pub(crate) enum ImportedDefinitionOriginError {
    Wire(scoop_wire::WireError),
    MissingSource { context: PersistentSourceContextId },
    SpanOverflow,
    ProviderOverflow,
    SourceIndexOverflow,
}

impl fmt::Display for ImportedDefinitionOriginError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(error) => error.fmt(formatter),
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

impl From<scoop_wire::WireError> for ImportedDefinitionOriginError {
    fn from(error: scoop_wire::WireError) -> Self {
        Self::Wire(error)
    }
}
