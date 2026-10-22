use super::*;
use crate::imports::ImportedTargetBinding;
use hir::EditingReferenceTarget as Target;

impl DeclarationIdentities {
    pub(in crate::editing) fn import_target(
        &mut self,
        lowerer: &mut Lowerer,
        binding: &ImportedTargetBinding,
    ) -> Option<Target> {
        use hir::EditingMemberTarget as M;
        if let Some(target) = lowerer.editing_import_type_target(binding) {
            return lowerer.editing_type_identity(target).map(Target::Type);
        }
        match lowerer.editing_import_member(binding)? {
            M::Callable(callable) => self.callable(lowerer, callable).map(Target::Callable),
            M::Property(property) => self
                .property(lowerer, hir::SemanticProperty::Current(property))
                .map(Target::Property),
            M::ImportedProperty(property) => Some(Target::Property(property)),
        }
    }
}
