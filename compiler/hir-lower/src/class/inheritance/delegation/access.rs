use super::*;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(super) fn interface_forwarder_visibility(
        &mut self,
        owner: Owner,
        class: ClassId,
        signature: &InterfaceSignature,
    ) -> ast::DeclaredVisibility {
        let internal = self.member_declared_domain(
            hir::DeclaredVisibility::Internal,
            owner,
            self.current_file,
        );
        for interface in self.owner_interfaces(owner) {
            for member in self.conformance_members(interface) {
                if !self.same_delegated_call_signature(signature, &member.signature) {
                    continue;
                }
                let hir::InterfaceMethodReference::Local(method) = member.member else {
                    // Dependency interface slots are public API.
                    return ast::DeclaredVisibility::Public;
                };
                let function = self.interface_method_entities[method].function;
                if self.functions[function]
                    .access
                    .slot
                    .as_ref()
                    .is_some_and(|slot| !self.access_domain_is_subset(&slot.0, &internal))
                {
                    return ast::DeclaredVisibility::Public;
                }
            }
        }
        for candidate in self.base_chain_methods(class) {
            if !self.function_is_accessible(candidate.function, None)
                || self.functions[candidate.function].method_type_param_count() != 0
            {
                continue;
            }
            let arguments = self.callable_candidate_owner_arguments(&candidate);
            let inherited = self.instantiated_signature(candidate.function, &arguments, &[]);
            let inherited = InterfaceSignature::local(
                self.source_function_name(candidate.function),
                &inherited,
                self.function_accessor_kind(candidate.function),
            );
            if self.same_delegated_call_signature(signature, &inherited)
                && self.functions[candidate.function]
                    .access
                    .slot
                    .as_ref()
                    .is_some_and(|slot| !self.access_domain_is_subset(&slot.0, &internal))
            {
                return ast::DeclaredVisibility::Public;
            }
        }
        for method in self.inherited_imported_class_methods(
            class,
            &signature.name,
            signature.accessor,
            self.classes[class].span,
        ) {
            if self.same_delegated_call_signature(signature, &method.signature)
                && self
                    .imported_callable_slot_domain(method.declaration.interface())
                    .is_some_and(|slot| !self.access_domain_is_subset(&slot.0, &internal))
            {
                return ast::DeclaredVisibility::Public;
            }
        }
        ast::DeclaredVisibility::Internal
    }
}
