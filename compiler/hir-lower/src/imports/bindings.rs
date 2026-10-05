use std::collections::BTreeMap;

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::{BindingNamespace, LocalBindingRole, PackagePath, SourceIdentity};

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
    Annotation(scoop_identity::PersistentAnnotationId),
    Function(hir::FunctionId),
    Property(hir::PropertyId),
    EnumVariant(hir::EnumVariantRef),
    SourceProperty(SourcePropertyId),
    SourceVariant(SourceVariantId),
}

impl CurrentUnitTarget {
    pub(crate) const fn occupies_namespace(self, namespace: BindingNamespace) -> bool {
        match namespace {
            BindingNamespace::Type => matches!(
                self,
                Self::Class(_)
                    | Self::Interface(_)
                    | Self::Struct(_)
                    | Self::Enum(_)
                    | Self::Object(_)
                    | Self::TypeAlias(_)
                    | Self::Annotation(_)
            ),
            BindingNamespace::Value => matches!(
                self,
                Self::Object(_)
                    | Self::Function(_)
                    | Self::Property(_)
                    | Self::EnumVariant(_)
                    | Self::SourceProperty(_)
                    | Self::SourceVariant(_)
            ),
        }
    }
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

#[derive(Debug, Clone)]
pub(crate) enum ImportedTargetBinding {
    /// The binding id dereferences to the typed declaration target. A
    /// forwarding static edge retains the same id rather than creating
    /// another entity.
    CurrentCone {
        binding: CurrentUnitBindingId,
        sources: ast::NonEmptyVec<CurrentUnitImportWitness>,
    },
    DirectDependency(hir::DirectImportedTargetBinding),
}

impl ImportedTargetBinding {
    pub(crate) fn current(
        binding: CurrentUnitBindingId,
        sources: ast::NonEmptyVec<CurrentUnitImportWitness>,
    ) -> Self {
        Self::CurrentCone { binding, sources }
    }

    pub(crate) const fn current_binding(&self) -> Option<CurrentUnitBindingId> {
        match self {
            Self::CurrentCone { binding, .. } => Some(*binding),
            Self::DirectDependency(_) => None,
        }
    }

    pub(crate) const fn direct_binding(&self) -> Option<&hir::DirectImportedTargetBinding> {
        match self {
            Self::CurrentCone { .. } => None,
            Self::DirectDependency(binding) => Some(binding),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ImportSyntaxOrigin {
    pub(crate) source: SourceIdentity,
    pub(crate) span: ast::Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolvedImportExposure {
    Local,
    PublicReexport,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedExactImport {
    pub(crate) exposure: ResolvedImportExposure,
    pub(crate) local_name: String,
    pub(crate) source_role: LocalBindingRole,
    pub(crate) targets: ast::NonEmptyVec<ImportedTargetBinding>,
    pub(crate) origin: ImportSyntaxOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolvedImportNamespace {
    Current(ResolvedNamespace),
    DirectPackage(PackagePath),
    DirectStatic(hir::SourceNominalId),
    SplitPackage {
        current: PackageId,
        direct: PackagePath,
    },
}

impl ResolvedImportNamespace {
    pub(crate) const fn current(&self) -> Option<ResolvedNamespace> {
        match self {
            Self::Current(namespace) => Some(*namespace),
            Self::SplitPackage { current, .. } => Some(ResolvedNamespace::Package(*current)),
            Self::DirectPackage(_) | Self::DirectStatic(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedStarImport {
    pub(crate) exposure: ResolvedImportExposure,
    pub(crate) namespace: ResolvedImportNamespace,
    pub(crate) snapshot: BTreeMap<String, ast::NonEmptyVec<ImportedTargetBinding>>,
    pub(crate) origin: ImportSyntaxOrigin,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FrozenFileImports {
    pub(crate) exact: Vec<ResolvedExactImport>,
    pub(crate) current_package_dependencies:
        BTreeMap<String, Vec<hir::DirectImportedTargetBinding>>,
    pub(crate) stars: Vec<ResolvedStarImport>,
}
