//! Definition-origin projection for persistent subjects established by Export HIR.

use std::fmt;

use scoop_ast::Span;
use scoop_hir as hir;
use scoop_identity::{
    DefinitionOrigin, DefinitionOriginRecord, DefinitionOriginSubject, SourceOriginError,
    SourceSpan, SourceSpanError,
};

use crate::Lowerer;

mod declarations;
mod members;
mod supplemental;

#[allow(clippy::too_many_arguments)]
pub(crate) fn build(
    lowerer: &Lowerer,
    nominals: &hir::HirNominalIdentities,
    properties: &hir::HirPropertyIdentities,
    accessors: &hir::HirPropertyAccessorIdentities,
    aliases: &hir::HirTypeAliasIdentities,
    enum_members: &hir::HirEnumMemberIdentities,
    fields: &hir::HirFieldIdentities,
    initialization_units: &hir::HirInitializationUnitIdentities,
    constructor_identities: &hir::HirConstructorIdentities,
    functions: &hir::HirFunctionIdentities,
    callbacks: &hir::HirCallbackRegistrationIdentities,
    local_bindings: &hir::HirLocalBindingIdentities,
    native_contracts: &hir::HirSourceNativeContracts,
    source_contexts: &hir::HirSourceContextIdentities,
) -> Result<hir::HirExportDefinitionOrigins, PersistentDefinitionOriginError> {
    let mut builder = DefinitionOriginBuilder {
        lowerer,
        source_contexts,
        records: Vec::new(),
    };
    builder.collect_nominals(nominals)?;
    builder.collect_functions(functions)?;
    builder.collect_constructors(constructor_identities)?;
    builder.collect_properties(properties)?;
    builder.collect_accessors(accessors)?;
    builder.collect_aliases(aliases)?;
    builder.collect_fields(fields)?;
    builder.collect_enum_members(enum_members)?;
    builder.collect_initialization_units(initialization_units)?;
    builder.collect_local_bindings(local_bindings);
    builder.collect_callbacks(callbacks)?;
    builder.collect_native_contracts(native_contracts)?;
    builder.finish()
}

struct DefinitionOriginBuilder<'a> {
    lowerer: &'a Lowerer,
    source_contexts: &'a hir::HirSourceContextIdentities,
    records: Vec<DefinitionOriginRecord>,
}

impl DefinitionOriginBuilder<'_> {
    fn append(
        &mut self,
        subject: DefinitionOriginSubject,
        file: usize,
        span: Span,
    ) -> Result<(), PersistentDefinitionOriginError> {
        let source = self.lowerer.intrinsic_sources.get(file).ok_or_else(|| {
            PersistentDefinitionOriginError::new(
                0,
                span,
                PersistentDefinitionOriginErrorDetail::UnknownSourceFile { subject, file },
            )
        })?;
        let context = self
            .lowerer
            .file_source_contexts
            .get(file)
            .copied()
            .ok_or_else(|| {
                PersistentDefinitionOriginError::new(
                    file,
                    span,
                    PersistentDefinitionOriginErrorDetail::MissingFileContext { subject },
                )
            })?;
        let persistent_context = self.source_contexts[context].key();
        let diagnostic_span = span;
        let span =
            SourceSpan::new(u64::from(span.start), u64::from(span.end)).map_err(|error| {
                PersistentDefinitionOriginError::new(
                    file,
                    span,
                    PersistentDefinitionOriginErrorDetail::InvalidSpan { subject, error },
                )
            })?;
        let origin = DefinitionOrigin::new(source.identity.clone(), span, persistent_context)
            .map_err(|error| {
                PersistentDefinitionOriginError::new(
                    file,
                    diagnostic_span,
                    PersistentDefinitionOriginErrorDetail::InvalidOrigin { subject, error },
                )
            })?;
        self.records
            .push(DefinitionOriginRecord::new(subject, origin));
        Ok(())
    }

    fn append_existing(&mut self, subject: DefinitionOriginSubject, origin: &DefinitionOrigin) {
        self.records
            .push(DefinitionOriginRecord::new(subject, origin.clone()));
    }

    fn finish(self) -> Result<hir::HirExportDefinitionOrigins, PersistentDefinitionOriginError> {
        hir::HirExportDefinitionOrigins::canonicalize(self.records).map_err(|error| {
            PersistentDefinitionOriginError::new(
                0,
                Span { start: 0, end: 0 },
                PersistentDefinitionOriginErrorDetail::Relation(error),
            )
        })
    }

    fn source_file(
        &self,
        subject: DefinitionOriginSubject,
        span: Span,
        file: Option<usize>,
    ) -> Result<usize, PersistentDefinitionOriginError> {
        file.ok_or_else(|| {
            PersistentDefinitionOriginError::new(
                0,
                span,
                PersistentDefinitionOriginErrorDetail::MissingSourceFile { subject },
            )
        })
    }
}

#[derive(Debug)]
pub(crate) struct PersistentDefinitionOriginError {
    file: usize,
    span: Span,
    detail: PersistentDefinitionOriginErrorDetail,
}

impl PersistentDefinitionOriginError {
    fn new(file: usize, span: Span, detail: PersistentDefinitionOriginErrorDetail) -> Self {
        Self { file, span, detail }
    }

    pub(crate) const fn file(&self) -> usize {
        self.file
    }

    pub(crate) const fn span(&self) -> Span {
        self.span
    }
}

impl fmt::Display for PersistentDefinitionOriginError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot derive persistent definition origin: {}",
            self.detail
        )
    }
}

impl std::error::Error for PersistentDefinitionOriginError {}

#[derive(Debug)]
enum PersistentDefinitionOriginErrorDetail {
    MissingSourceFile {
        subject: DefinitionOriginSubject,
    },
    UnknownSourceFile {
        subject: DefinitionOriginSubject,
        file: usize,
    },
    MissingFileContext {
        subject: DefinitionOriginSubject,
    },
    MissingMemberSpan {
        subject: DefinitionOriginSubject,
    },
    InvalidSpan {
        subject: DefinitionOriginSubject,
        error: SourceSpanError,
    },
    InvalidOrigin {
        subject: DefinitionOriginSubject,
        error: SourceOriginError,
    },
    Relation(hir::HirExportDefinitionOriginError),
}

impl fmt::Display for PersistentDefinitionOriginErrorDetail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSourceFile { subject } => {
                write!(formatter, "subject {subject:?} has no source file")
            }
            Self::UnknownSourceFile { subject, file } => {
                write!(
                    formatter,
                    "subject {subject:?} refers to unknown source file {file}"
                )
            }
            Self::MissingFileContext { subject } => {
                write!(formatter, "subject {subject:?} has no file source context")
            }
            Self::MissingMemberSpan { subject } => {
                write!(formatter, "subject {subject:?} has no checked source span")
            }
            Self::InvalidSpan { subject, error } => {
                write!(
                    formatter,
                    "subject {subject:?} has an invalid source span: {error}"
                )
            }
            Self::InvalidOrigin { subject, error } => {
                write!(
                    formatter,
                    "subject {subject:?} has an invalid source origin: {error}"
                )
            }
            Self::Relation(error) => error.fmt(formatter),
        }
    }
}
