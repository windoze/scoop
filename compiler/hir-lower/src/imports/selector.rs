use std::collections::BTreeMap;

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::{BindingNamespace, CanonicalIdentifier};

use super::*;
use crate::Lowerer;

mod groups;

pub(super) enum SelectorResult {
    Targets(ast::NonEmptyVec<ImportedTargetBinding>),
    Namespace {
        identity: ResolvedImportNamespace,
        snapshot: BTreeMap<String, ast::NonEmptyVec<ImportedTargetBinding>>,
    },
}

pub(super) enum SelectorError {
    ExpectedBindingAfterPackage,
    Missing {
        inaccessible: Vec<CurrentUnitBindingId>,
    },
    AmbiguousBinding,
    AmbiguousStaticOwner,
    NotStaticOwner,
}

impl SelectorError {
    pub(super) const fn message(&self) -> &'static str {
        match self {
            Self::ExpectedBindingAfterPackage => {
                "exact import requires an importable binding, not a package namespace"
            }
            Self::Missing { .. } | Self::NotStaticOwner => {
                "import target is not available in the current compilation unit"
            }
            Self::AmbiguousBinding => "import target is ambiguous in the current compilation unit",
            Self::AmbiguousStaticOwner => {
                "import namespace is ambiguous in the current compilation unit"
            }
        }
    }

    pub(super) fn inaccessible(&self) -> &[CurrentUnitBindingId] {
        match self {
            Self::Missing { inaccessible } => inaccessible,
            Self::ExpectedBindingAfterPackage
            | Self::AmbiguousBinding
            | Self::AmbiguousStaticOwner
            | Self::NotStaticOwner => &[],
        }
    }
}

enum NamespaceCursor<'world, 'input> {
    Current(ResolvedNamespace),
    Direct(hir::DirectNamespaceView<'world, 'input>),
    SplitPackage {
        current: PackageId,
        direct: hir::DirectPackageView<'world, 'input>,
    },
}

#[derive(Default)]
struct SelectorGroup {
    current: Vec<CurrentUnitBindingId>,
    direct: Vec<hir::DirectImportedTargetBinding>,
    inaccessible: Vec<CurrentUnitBindingId>,
}

impl CurrentUnitImports {
    pub(super) fn selector<'world, 'input>(
        &self,
        lowerer: &Lowerer,
        world: Option<&'world hir::ImportedSemanticWorld<'input>>,
        path: &ast::QualifiedNameSyntax,
        star: bool,
    ) -> Result<SelectorResult, SelectorError> {
        if let Some(world) = world {
            debug_assert_eq!(world.current(), lowerer.current_cone());
        }
        let ast_segments = path.segments().cloned().collect::<Vec<_>>();
        let canonical_segments = ast_segments
            .iter()
            .map(|segment| {
                CanonicalIdentifier::new(&segment.text)
                    .expect("the parser accepts only canonical source identifiers")
            })
            .collect::<Vec<_>>();
        let (consumed, mut cursor) =
            self.longest_package_cursor(lowerer, world, &ast_segments, &canonical_segments);

        if !star {
            let Some((last, owners)) = canonical_segments[consumed..].split_last() else {
                return Err(SelectorError::ExpectedBindingAfterPackage);
            };
            for owner in owners {
                cursor = self.descend_static(lowerer, world, cursor, owner.as_str())?;
            }
            let group = self.cursor_group(lowerer, &cursor, last.as_str());
            return self.materialize_non_empty_group(lowerer, group);
        }

        for owner in &canonical_segments[consumed..] {
            cursor = self.descend_static(lowerer, world, cursor, owner.as_str())?;
        }
        Ok(SelectorResult::Namespace {
            identity: cursor.identity(),
            snapshot: self.cursor_snapshot(lowerer, &cursor),
        })
    }

    fn longest_package_cursor<'world, 'input>(
        &self,
        lowerer: &Lowerer,
        world: Option<&'world hir::ImportedSemanticWorld<'input>>,
        ast_segments: &[ast::Ident],
        canonical_segments: &[CanonicalIdentifier],
    ) -> (usize, NamespaceCursor<'world, 'input>) {
        let (current, current_len) = lowerer
            .top_level_namespaces
            .longest_package_prefix(ast_segments);
        let direct =
            world.and_then(|world| world.direct_packages().longest_prefix(canonical_segments));
        match direct {
            Some(direct) if direct.consumed_segments() > current_len => {
                let namespace = world
                    .and_then(|world| world.direct_package(direct.path()))
                    .expect("a direct package match reopens in the same semantic world");
                (
                    direct.consumed_segments(),
                    NamespaceCursor::Direct(hir::DirectNamespaceView::Package(namespace)),
                )
            }
            Some(direct) if direct.consumed_segments() == current_len => {
                let namespace = world
                    .and_then(|world| world.direct_package(direct.path()))
                    .expect("a direct package match reopens in the same semantic world");
                (
                    current_len,
                    NamespaceCursor::SplitPackage {
                        current,
                        direct: namespace,
                    },
                )
            }
            Some(_) | None => (
                current_len,
                NamespaceCursor::Current(ResolvedNamespace::Package(current)),
            ),
        }
    }

    fn descend_static<'world, 'input>(
        &self,
        lowerer: &Lowerer,
        world: Option<&'world hir::ImportedSemanticWorld<'input>>,
        cursor: NamespaceCursor<'world, 'input>,
        name: &str,
    ) -> Result<NamespaceCursor<'world, 'input>, SelectorError> {
        let group = self.cursor_group_for_namespace(lowerer, &cursor, BindingNamespace::Type, name);
        match (group.current.as_slice(), group.direct.as_slice()) {
            ([binding], []) => self
                .static_targets
                .get(binding)
                .copied()
                .map(ResolvedNamespace::Static)
                .map(NamespaceCursor::Current)
                .ok_or(SelectorError::NotStaticOwner),
            ([], [binding]) => {
                let owner = binding
                    .target()
                    .source_nominal()
                    .ok_or(SelectorError::NotStaticOwner)?;
                let namespace = world
                    .and_then(|world| world.imported_static_namespace(owner))
                    .ok_or(SelectorError::NotStaticOwner)?;
                Ok(NamespaceCursor::Direct(hir::DirectNamespaceView::Static(
                    namespace,
                )))
            }
            ([], []) => Err(SelectorError::Missing {
                inaccessible: group.inaccessible,
            }),
            _ => Err(SelectorError::AmbiguousStaticOwner),
        }
    }
}

impl NamespaceCursor<'_, '_> {
    fn identity(&self) -> ResolvedImportNamespace {
        match self {
            Self::Current(namespace) => ResolvedImportNamespace::Current(*namespace),
            Self::Direct(hir::DirectNamespaceView::Package(namespace)) => {
                ResolvedImportNamespace::DirectPackage(namespace.path().clone())
            }
            Self::Direct(hir::DirectNamespaceView::Static(namespace)) => {
                ResolvedImportNamespace::DirectStatic(namespace.owner().declaration().persistent())
            }
            Self::SplitPackage { current, direct } => ResolvedImportNamespace::SplitPackage {
                current: *current,
                direct: direct.path().clone(),
            },
        }
    }
}
