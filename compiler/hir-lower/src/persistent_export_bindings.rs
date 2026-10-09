//! Atomic construction of the current Cone's complete public binding table.

use std::fmt;

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::{BindableEntity, SourceIdentity};

use crate::Lowerer;

mod merge;

#[derive(Debug)]
pub(crate) struct PersistentExportBindings {
    pub(crate) identities: hir::HirExportBindingIdentities,
    pub(crate) surface: hir::CanonicalPublicExportBindingsV1,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build(
    lowerer: &Lowerer,
    surface: &hir::PublicSemanticSurface,
    nominals: &hir::HirNominalIdentities,
    enum_members: &hir::HirEnumMemberIdentities,
    object_values: &hir::HirObjectValueIdentities,
    functions: &hir::HirFunctionIdentities,
    properties: &hir::HirPropertyIdentities,
    aliases: &hir::HirTypeAliasIdentities,
) -> Result<PersistentExportBindings, PersistentExportBindingIdentityError> {
    let direct =
        hir::HirExportBindingIdentities::from_public_surface(hir::HirExportBindingIdentityInputs {
            surface,
            structs: &lowerer.structs,
            enums: &lowerer.enums,
            classes: &lowerer.classes,
            interfaces: &lowerer.interfaces,
            objects: &lowerer.objects,
            singleton_values: &lowerer.singleton_values,
            functions: lowerer.functions.as_arena(),
            properties: &lowerer.properties,
            type_aliases: &lowerer.type_aliases,
            nominal_identities: nominals,
            enum_member_identities: enum_members,
            object_value_identities: object_values,
            function_identities: functions,
            property_identities: properties,
            type_alias_identities: aliases,
            annotations: &lowerer.annotation_metadata,
        })
        .map_err(PersistentExportBindingIdentityError::Direct)?;

    merge::merge(
        lowerer,
        surface,
        nominals,
        enum_members,
        object_values,
        functions,
        properties,
        direct,
    )
}

#[derive(Debug)]
pub(crate) enum PersistentExportBindingIdentityError {
    Direct(hir::HirExportBindingIdentityError),
    UnknownOverloadSignature {
        target: BindableEntity,
    },
    InvalidReexportTarget {
        file: usize,
        span: ast::Span,
    },
    UnknownReexportSource {
        source: SourceIdentity,
        span: ast::Span,
    },
    DestinationConflict {
        file: usize,
        span: ast::Span,
    },
    DuplicateBindingSource {
        file: usize,
        span: ast::Span,
    },
    Identities(hir::HirExportBindingIdentityError),
    Surface(hir::PublicExportBindingBuildError),
}

impl PersistentExportBindingIdentityError {
    pub(crate) const fn file(&self) -> usize {
        match self {
            Self::InvalidReexportTarget { file, .. }
            | Self::DestinationConflict { file, .. }
            | Self::DuplicateBindingSource { file, .. } => *file,
            Self::Direct(_)
            | Self::UnknownOverloadSignature { .. }
            | Self::UnknownReexportSource { .. }
            | Self::Identities(_)
            | Self::Surface(_) => 0,
        }
    }

    pub(crate) const fn span(&self) -> ast::Span {
        match self {
            Self::InvalidReexportTarget { span, .. }
            | Self::UnknownReexportSource { span, .. }
            | Self::DestinationConflict { span, .. }
            | Self::DuplicateBindingSource { span, .. } => *span,
            Self::Direct(_)
            | Self::UnknownOverloadSignature { .. }
            | Self::Identities(_)
            | Self::Surface(_) => ast::Span { start: 0, end: 0 },
        }
    }
}

impl fmt::Display for PersistentExportBindingIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Direct(error) | Self::Identities(error) => write!(
                formatter,
                "cannot derive persistent export binding identity: {error}"
            ),
            Self::UnknownOverloadSignature { target } => write!(
                formatter,
                "public overload binding {target:?} has no source signature"
            ),
            Self::InvalidReexportTarget { .. } => formatter
                .write_str("a frozen re-export disagrees with its terminal target or binding kind"),
            Self::UnknownReexportSource { source, .. } => write!(
                formatter,
                "a frozen re-export refers to unknown current source {source:?}"
            ),
            Self::DestinationConflict { .. } => {
                formatter.write_str("re-export destination conflicts with a current public binding")
            }
            Self::DuplicateBindingSource { .. } => formatter.write_str(
                "one public export binding cannot be both declared here and re-exported",
            ),
            Self::Surface(error) => {
                write!(
                    formatter,
                    "cannot build canonical public binding surface: {error}"
                )
            }
        }
    }
}

impl std::error::Error for PersistentExportBindingIdentityError {}

#[cfg(test)]
mod tests;
