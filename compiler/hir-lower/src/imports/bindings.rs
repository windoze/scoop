use std::collections::BTreeMap;

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::{LocalBindingRole, SourceIdentity};

use crate::{aliases::SourceTypeAliasId, namespace::PackageId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CurrentUnitBindingId(pub(super) usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SourcePropertyId(pub(super) usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SourceVariantId(pub(super) usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CurrentUnitTarget {
    Class(hir::ClassId),
    Interface(hir::InterfaceId),
    Struct(hir::StructId),
    Enum(hir::EnumId),
    Object(hir::ObjectId),
    TypeAlias(SourceTypeAliasId),
    Function(hir::FunctionId),
    Property(hir::PropertyId),
    EnumVariant(hir::EnumVariantRef),
    SourceProperty(SourcePropertyId),
    SourceVariant(SourceVariantId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum StaticNamespace {
    Class(hir::ClassId),
    Interface(hir::InterfaceId),
    Struct(hir::StructId),
    Enum(hir::EnumId),
    Object(hir::ObjectId),
    Companion(hir::CompanionRelationId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ResolvedNamespace {
    Package(PackageId),
    Static(StaticNamespace),
}

/// A declaration-side identity plus complete diagnostic/access provenance.
#[derive(Debug, Clone)]
pub(crate) struct CurrentUnitBinding {
    pub(crate) target: CurrentUnitTarget,
    pub(crate) source: SourceIdentity,
    pub(crate) file: usize,
    pub(crate) span: ast::Span,
    pub(crate) access: hir::EffectiveLookupDomain,
    pub(crate) name: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CurrentUnitImportWitness {
    pub(crate) source_binding: CurrentUnitBindingId,
    pub(crate) site: scoop_identity::SourceIdentity,
    pub(crate) access: hir::EffectiveLookupDomain,
}

/// The binding id dereferences to the typed declaration target. A forwarding
/// static edge retains the same id rather than creating another entity.
#[derive(Debug, Clone)]
pub(crate) struct ImportedBinding {
    pub(crate) binding: CurrentUnitBindingId,
    pub(crate) sources: ast::NonEmptyVec<CurrentUnitImportWitness>,
}

#[derive(Debug, Clone)]
pub(crate) struct ImportSyntaxOrigin {
    pub(crate) source: SourceIdentity,
    pub(crate) span: ast::Span,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedExactImport {
    pub(crate) local_name: String,
    pub(crate) source_role: LocalBindingRole,
    pub(crate) targets: ast::NonEmptyVec<ImportedBinding>,
    pub(crate) origin: ImportSyntaxOrigin,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedStarImport {
    pub(crate) namespace: ResolvedNamespace,
    pub(crate) snapshot: BTreeMap<String, ast::NonEmptyVec<ImportedBinding>>,
    pub(crate) origin: ImportSyntaxOrigin,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FrozenFileImports {
    pub(crate) exact: Vec<ResolvedExactImport>,
    pub(crate) stars: Vec<ResolvedStarImport>,
}
