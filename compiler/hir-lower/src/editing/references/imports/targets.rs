use super::*;
use crate::imports::lookup::values::{ValueOrigin, ValueTarget};
use crate::imports::{CurrentUnitTarget, ImportedTargetBinding};

impl ReferenceBuilder {
    pub(super) fn import_target(
        &mut self,
        lowerer: &mut Lowerer,
        binding: &ImportedTargetBinding,
    ) -> Option<Target> {
        match binding {
            ImportedTargetBinding::CurrentCone { binding, .. } => {
                match lowerer.imports.binding(*binding).target {
                    CurrentUnitTarget::Function(function) => self
                        .callable(lowerer, hir::SemanticCallable::Current(function))
                        .map(Target::Callable),
                    CurrentUnitTarget::Property(_) | CurrentUnitTarget::SourceProperty(_) => {
                        let ValueTarget::Property(property) = lowerer
                            .materialized_value_target(&ValueOrigin::CurrentUnit(*binding))?
                        else {
                            return None;
                        };
                        self.property(lowerer, hir::SemanticProperty::Current(property))
                            .map(Target::Property)
                    }
                    _ => None,
                }
            }
            ImportedTargetBinding::DirectDependency(binding) => {
                use hir::{DefaultCallableDeclarationV1 as D, ImportedTarget as T};
                Some(match binding.target() {
                    T::Function(id) => {
                        Target::Callable(Callable::Declaration(D::Function(id.persistent())))
                    }
                    T::GenericFunction(id) => {
                        Target::Callable(Callable::Declaration(D::GenericFunction(id.persistent())))
                    }
                    T::Property(id) => {
                        Target::Property(hir::PropertyDeclarationId::Property(id.persistent()))
                    }
                    T::ExtensionProperty(id) => Target::Property(
                        hir::PropertyDeclarationId::ExtensionProperty(id.persistent()),
                    ),
                    _ => return None,
                })
            }
        }
    }
}
