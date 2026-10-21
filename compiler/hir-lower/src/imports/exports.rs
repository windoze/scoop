//! Read the established public source namespace, including frozen re-exports.

use super::*;
use crate::Lowerer;
use scoop_ast::{Diagnostic, Span};
use scoop_identity::{BindingNamespace, CanonicalIdentifier};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExportLookupTarget {
    Current(CurrentUnitBindingId),
    Dependency(hir::ImportedTarget),
}

enum ExportNamespace {
    Current(ResolvedNamespace),
    Dependency(hir::SourceNominalId),
}

impl CurrentUnitImports {
    pub(crate) fn query_public_exports(
        &self,
        lowerer: &Lowerer,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
        path: &[CanonicalIdentifier],
        namespace: BindingNamespace,
    ) -> Result<Vec<ExportLookupTarget>, Diagnostic> {
        let (package, consumed) = lowerer.top_level_namespaces.longest_package_prefix_names(
            lowerer.current_file,
            path.iter().map(CanonicalIdentifier::as_str),
        );
        let Some((last, owners)) = path[consumed..].split_last() else {
            return Ok(Vec::new());
        };
        let mut cursor = ExportNamespace::Current(ResolvedNamespace::Package(package));
        for owner in owners {
            let group = self.export_group(
                lowerer,
                world,
                &cursor,
                BindingNamespace::Type,
                owner.as_str(),
            );
            cursor = match group.as_slice() {
                [ExportLookupTarget::Current(binding)] => {
                    let Some(namespace) = self.static_targets.get(binding) else {
                        return Ok(Vec::new());
                    };
                    ExportNamespace::Current(ResolvedNamespace::Static(*namespace))
                }
                [ExportLookupTarget::Dependency(target)] => {
                    let Some(owner) = target.source_nominal() else {
                        return Ok(Vec::new());
                    };
                    ExportNamespace::Dependency(owner)
                }
                [] => return Ok(Vec::new()),
                _ => {
                    return Err(Diagnostic::at(
                        Span::new(0, 0),
                        format!("export static owner `{}` is ambiguous", owner.as_str()),
                    ));
                }
            };
        }
        Ok(self.export_group(lowerer, world, &cursor, namespace, last.as_str()))
    }

    fn export_group(
        &self,
        lowerer: &Lowerer,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
        cursor: &ExportNamespace,
        namespace: BindingNamespace,
        name: &str,
    ) -> Vec<ExportLookupTarget> {
        match cursor {
            ExportNamespace::Dependency(owner) => world
                .and_then(|world| world.imported_static_namespace(*owner))
                .and_then(|view| view.binding_group(namespace, name))
                .map(|group| {
                    group
                        .targets()
                        .map(|binding| ExportLookupTarget::Dependency(binding.target()))
                        .collect()
                })
                .unwrap_or_default(),
            ExportNamespace::Current(current) => {
                let mut targets = self
                    .canonicalize(
                        self.namespaces
                            .get(current)
                            .and_then(|names| names.get(name))
                            .into_iter()
                            .flatten()
                            .copied()
                            .filter(|binding| {
                                let binding = self.binding(*binding);
                                binding.target.occupies_namespace(namespace)
                                    && binding.access.0.is_universal()
                                    && binding.source.cone() == lowerer.current_cone()
                                    && lowerer.declaration_source_in_scope(binding.file)
                            }),
                    )
                    .into_iter()
                    .map(ExportLookupTarget::Current)
                    .collect::<Vec<_>>();
                if let ResolvedNamespace::Package(package) = current {
                    let segments = lowerer.top_level_namespaces.package_segments(*package);
                    for reexport in &self.reexports {
                        let key = reexport.identity.key();
                        if key.namespace() == namespace
                            && key.name().as_str() == name
                            && key
                                .package()
                                .segments()
                                .iter()
                                .map(CanonicalIdentifier::as_str)
                                .eq(segments.iter().copied())
                        {
                            let target = ExportLookupTarget::Dependency(reexport.target);
                            if !targets.contains(&target) {
                                targets.push(target);
                            }
                        }
                    }
                }
                targets
            }
        }
    }
}
