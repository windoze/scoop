use super::*;

impl Lowerer {
    /// Override declarations name slots even when the slot's implementation
    /// condition is not known in the ordinary member's environment. The
    /// conformance pass checks coverage separately under each obligation.
    pub(in crate::class::inheritance) fn member_override_interfaces(
        &mut self,
        owner: Owner,
        condition: Option<hir::ConditionalInterfaceId>,
    ) -> Vec<TypeId> {
        let mut interfaces = if condition.is_none() {
            self.owner_interfaces(owner)
        } else {
            Vec::new()
        };
        if condition.is_none() {
            for (interface, condition) in self.inherited_conditional_interface_domains(owner) {
                if !self.conditional_application_is_impossible(&condition) {
                    self.append_interface_closure(interface, &mut interfaces);
                }
            }
        }
        for block in self.owner_conditional_interfaces(owner).to_vec() {
            if condition.is_none_or(|condition| condition == block.id()) {
                self.append_interface_closure(block.interface, &mut interfaces);
            }
        }
        interfaces
    }

    pub(in crate::class::inheritance) fn conditional_member_has_slot(
        &mut self,
        function: FunctionId,
        interfaces: &[TypeId],
    ) -> bool {
        let signature = InterfaceSignature::local(
            self.source_function_name(function),
            &self.signatures[&function],
            self.function_accessor_kind(function),
        );
        interfaces.iter().any(|&interface| {
            self.conformance_members(interface)
                .iter()
                .any(|member| self.same_interface_signature_shape(&signature, &member.signature))
        })
    }

    pub(crate) fn property_implementation_condition(
        &self,
        property: hir::PropertyId,
    ) -> Option<hir::ConditionalInterfaceId> {
        let getter = self.properties[property].capability.getter();
        match self.property_getters[getter].implementation {
            hir::PropertyAccessorImplementation::Body(function)
            | hir::PropertyAccessorImplementation::StorageBody(function)
            | hir::PropertyAccessorImplementation::AbstractSlot(function) => self.functions
                [function]
                .method
                .and_then(|method| method.availability),
            hir::PropertyAccessorImplementation::Storage
            | hir::PropertyAccessorImplementation::Constant => None,
        }
    }
}
