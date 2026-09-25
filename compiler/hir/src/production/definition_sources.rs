//! Projection of request-local HIR source positions into persistent origins.

use std::collections::BTreeSet;
use std::fmt;

use scoop_identity::{DefinitionOrigin, SourceOriginError, SourceSpan};
use scoop_wire::{WireError, WirePath};

use crate::{
    CanonicalCallableSourceInterfacesV1, CanonicalExportConstValuesV1,
    CanonicalExportDefaultTemplatesV1, CanonicalExportDefinitionSourcesV1,
    CanonicalTypeAliasInterfacesV1, ExportDefaultReferenceV1, ExportDefinitionSourceSetBuildError,
    ExportDefinitionSourceV1, ExportHir, IntrinsicProviderId, TemplateLocalDefinitionV1,
};

impl CanonicalExportDefinitionSourcesV1 {
    /// Collects the exact canonical set of definition sources embedded in
    /// cross-Cone interface fields 1 through 8.
    pub fn from_interface_parts(
        type_aliases: &CanonicalTypeAliasInterfacesV1,
        source_interfaces: &CanonicalCallableSourceInterfacesV1,
        default_templates: &CanonicalExportDefaultTemplatesV1,
        constants: &CanonicalExportConstValuesV1,
    ) -> Result<Self, ExportDefinitionSourceProductionError> {
        let mut sources = BTreeSet::new();

        sources.extend(
            type_aliases
                .records()
                .iter()
                .map(|record| record.definition_origin().clone()),
        );
        for interface in source_interfaces.records() {
            sources.extend(
                interface
                    .parameters()
                    .parameters()
                    .iter()
                    .map(|parameter| parameter.definition_origin().clone()),
            );
        }

        for (wire_index, (template_index, template)) in
            (0_u64..).zip(default_templates.records().iter().enumerate())
        {
            sources.insert(template.definition_origin().clone());
            for local in template.locals().records() {
                if let TemplateLocalDefinitionV1::Source(source) = local.definition() {
                    sources.insert(source.clone());
                }
            }
            template
                .body()
                .visit_definition_sources(
                    &mut |source, _, _| {
                        sources.insert(source.clone());
                        Ok(())
                    },
                    &WirePath::root().field(7).index(wire_index).field(5),
                )
                .map_err(
                    |source| ExportDefinitionSourceProductionError::DefaultBody {
                        template_index,
                        source,
                    },
                )?;

            let references = template.references();
            collect_reference_sources(&mut sources, references.callables());
            collect_reference_sources(&mut sources, references.constructors());
            collect_reference_sources(&mut sources, references.types());
            collect_reference_sources(&mut sources, references.globals());
            collect_reference_sources(&mut sources, references.singleton_values());
            collect_reference_sources(&mut sources, references.fields());
        }

        sources.extend(
            constants
                .records()
                .iter()
                .map(|record| record.definition_origin().clone()),
        );

        Self::try_new(sources.into_iter().collect())
            .map_err(ExportDefinitionSourceProductionError::Table)
    }
}

fn collect_reference_sources<T>(
    sources: &mut BTreeSet<ExportDefinitionSourceV1>,
    references: &[ExportDefaultReferenceV1<T>],
) {
    sources.extend(
        references
            .iter()
            .map(|reference| reference.definition_origin().clone()),
    );
}

pub(crate) fn project_definition_source(
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

#[derive(Debug)]
pub enum ExportDefinitionSourceProductionError {
    DefaultBody {
        template_index: usize,
        source: WireError,
    },
    Table(ExportDefinitionSourceSetBuildError),
}

impl fmt::Display for ExportDefinitionSourceProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DefaultBody {
                template_index,
                source,
            } => write!(
                formatter,
                "cannot collect definition sources from default template {template_index}: {source}"
            ),
            Self::Table(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for ExportDefinitionSourceProductionError {}
