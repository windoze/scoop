//! Documentation shares the ordinary package and static namespace selectors.

use super::*;
use crate::imports::lookup::calls::NamedCallTarget;

impl CurrentUnitImports {
    pub(crate) fn documentation_qualified_targets(
        &self,
        lowerer: &Lowerer,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
        path: &ast::QualifiedNameSyntax,
    ) -> Result<Vec<NamedCallTarget>, ()> {
        let SelectorResult::Targets(targets) =
            self.selector(lowerer, world, path, false).map_err(|_| ())?
        else {
            unreachable!("an exact selector returns declarations");
        };
        Ok(targets
            .iter()
            .map(|target| match target {
                ImportedTargetBinding::CurrentCone { binding, .. } => {
                    lowerer.current_named_call_target(*binding)
                }
                ImportedTargetBinding::DirectDependency(binding) => {
                    NamedCallTarget::ImportedDependency(binding.target())
                }
            })
            .collect())
    }

    pub(crate) fn documentation_static_targets(
        &self,
        lowerer: &Lowerer,
        owner: crate::Owner,
        name: &str,
    ) -> Vec<NamedCallTarget> {
        let namespace = match owner {
            crate::Owner::Class(id) => StaticNamespace::Class(id),
            crate::Owner::Struct(id) => StaticNamespace::Struct(id),
            crate::Owner::Enum(id) => StaticNamespace::Enum(id),
            crate::Owner::Interface(id) => StaticNamespace::Interface(id),
            crate::Owner::Object(id) => StaticNamespace::Object(id),
        };
        let group = self.cursor_group(
            lowerer,
            &NamespaceCursor::Current(ResolvedNamespace::Static(namespace)),
            name,
        );
        group
            .current
            .into_iter()
            .map(|binding| lowerer.current_named_call_target(binding))
            .collect()
    }
}
