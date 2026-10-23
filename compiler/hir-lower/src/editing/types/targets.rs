use super::*;
use crate::{imports::ImportedTargetBinding, namespace::TopLevelTypeTarget};

impl Lowerer {
    pub(crate) fn editing_lookup_type_target(&self, binding: &TypeLookupTarget) -> TypeTarget {
        match binding {
            TypeLookupTarget::Current(target) => match target {
                TopLevelTypeTarget::Nominal(target) => {
                    TypeTarget::Nominal(self.nominal_identity(target.owner()).declaration_id())
                }
                TopLevelTypeTarget::Alias(id) => TypeTarget::Alias(*id),
                TopLevelTypeTarget::Annotation(id) => TypeTarget::Annotation(*id),
            },
            TypeLookupTarget::Dependency(binding) => TypeTarget::Imported(binding.target()),
        }
    }

    pub(crate) fn editing_import_type_target(
        &self,
        binding: &ImportedTargetBinding,
    ) -> Option<TypeTarget> {
        match binding {
            ImportedTargetBinding::CurrentCone { binding, .. } => {
                let target = self.imports.binding(*binding).target.type_target()?;
                Some(self.editing_lookup_type_target(&TypeLookupTarget::Current(target)))
            }
            ImportedTargetBinding::DirectDependency(binding) => {
                let target = binding.target();
                (target.source_nominal().is_some()
                    || matches!(
                        target,
                        hir::ImportedTarget::TypeAlias(_) | hir::ImportedTarget::Annotation(_)
                    ))
                .then_some(TypeTarget::Imported(target))
            }
        }
    }

    pub(crate) fn editing_type_identity(
        &mut self,
        target: TypeTarget,
    ) -> Option<hir::EditingTypeIdentity> {
        if !self.editing_type_available(target) {
            return None;
        }
        use hir::EditingTypeIdentity as T;
        Some(match target {
            TypeTarget::Nominal(id) => T::Nominal(id),
            TypeTarget::Alias(id) => {
                let id = self.published_type_alias(id)?;
                T::Alias(self.query_alias_identity(id).ok()?.id())
            }
            TypeTarget::Parameter(id) => T::Parameter(id),
            TypeTarget::Annotation(id) => T::Annotation(id),
            TypeTarget::Imported(target) => match target {
                hir::ImportedTarget::TypeAlias(id) => T::Alias(id.persistent()),
                hir::ImportedTarget::Annotation(id) => T::Annotation(id.persistent()),
                _ => T::Nominal(target.source_nominal()?),
            },
        })
    }

    pub(crate) fn editing_type_origin(
        &self,
        target: TypeTarget,
    ) -> Option<hir::SemanticSourceLocation> {
        if let Some(origin) = self.editing_type_definitions.get(&target) {
            return Some(origin.clone());
        }
        let dependencies = self.dependencies.as_ref()?;
        let origin = match target {
            TypeTarget::Imported(target) => dependencies.definition_origin(target)?,
            TypeTarget::Nominal(id) => dependencies.nominal_declaration(id)?.origin.origin(),
            TypeTarget::Alias(_) | TypeTarget::Parameter(_) | TypeTarget::Annotation(_) => {
                return None;
            }
        };
        crate::semantic_queries::imported_location(origin).ok()
    }
}
