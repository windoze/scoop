use super::*;
use crate::imports::ImportedTargetBinding;
use hir::EditingReferenceTarget as Target;

impl DeclarationIdentities {
    pub(in crate::editing) fn import_target(
        &mut self,
        lowerer: &mut Lowerer,
        binding: &ImportedTargetBinding,
    ) -> Option<Target> {
        if let ImportedTargetBinding::DirectDependency(binding) = binding {
            return Self::imported_target(lowerer, binding.target());
        }
        use hir::EditingMemberTarget as M;
        if let Some(target) = lowerer.editing_import_type_target(binding) {
            return lowerer.editing_type_identity(target).map(Target::Type);
        }
        if let Some(target) = lowerer.editing_import_variant(binding) {
            return self.callee(lowerer, target).map(Target::Callable);
        }
        match lowerer.editing_import_member(binding)? {
            M::Callable(callable) => self.callable(lowerer, callable).map(Target::Callable),
            M::Property(property) => self
                .property(lowerer, hir::SemanticProperty::Current(property))
                .map(Target::Property),
            M::ImportedProperty(property) => Some(Target::Property(property)),
        }
    }

    pub(in crate::editing) fn imported_target(
        lowerer: &Lowerer,
        target: hir::ImportedTarget,
    ) -> Option<Target> {
        use hir::{
            DefaultCallableDeclarationV1 as D, EditingCallableIdentity as C,
            EditingTypeIdentity as T, ImportedTarget as I,
        };
        Some(match target {
            I::Function(id) => Target::Callable(C::Declaration(D::Function(id.persistent()))),
            I::GenericFunction(id) => {
                Target::Callable(C::Declaration(D::GenericFunction(id.persistent())))
            }
            I::Property(id) => {
                Target::Property(hir::PropertyDeclarationId::Property(id.persistent()))
            }
            I::ExtensionProperty(id) => Target::Property(
                hir::PropertyDeclarationId::ExtensionProperty(id.persistent()),
            ),
            I::EnumVariant(id) => Target::Callable(C::Variant(id.persistent())),
            I::TypeAlias(id) => Target::Type(T::Alias(id.persistent())),
            I::Annotation(id) => Target::Type(T::Annotation(id.persistent())),
            I::Type(id) => {
                Target::Type(T::Nominal(hir::SourceNominalId::Concrete(id.persistent())))
            }
            I::GenericType(id) => Target::Type(T::Nominal(hir::SourceNominalId::GenericTemplate(
                id.persistent(),
            ))),
            I::ObjectValue(id) => Target::Type(T::Nominal(
                lowerer
                    .dependencies
                    .as_ref()?
                    .singleton_owner(id.persistent())?
                    .interface
                    .declaration(),
            )),
        })
    }
}
