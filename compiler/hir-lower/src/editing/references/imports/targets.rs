use super::*;
use crate::imports::ImportedTargetBinding;

impl ReferenceBuilder {
    pub(super) fn import_target(
        &mut self,
        lowerer: &mut Lowerer,
        binding: &ImportedTargetBinding,
    ) -> Option<Target> {
        use hir::EditingMemberTarget as M;
        match lowerer.editing_import_member(binding)? {
            M::Callable(callable) => self.callable(lowerer, callable).map(Target::Callable),
            M::Property(property) => self
                .property(lowerer, hir::SemanticProperty::Current(property))
                .map(Target::Property),
            M::ImportedProperty(property) => Some(Target::Property(property)),
        }
    }
}
