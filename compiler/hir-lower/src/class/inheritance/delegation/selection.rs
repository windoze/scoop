use super::*;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(super) fn select_interface_delegates(
        &mut self,
        owner: Owner,
        class: ClassId,
    ) -> Vec<DelegateSlot> {
        let delegates = self
            .interface_delegates
            .get(&class)
            .cloned()
            .unwrap_or_default();
        let mut groups: Vec<Vec<DelegateCandidate>> = Vec::new();
        for delegate in delegates {
            for member in self.conformance_members(delegate.interface) {
                let candidate = DelegateCandidate {
                    field: delegate.field,
                    member,
                };
                if let Some(group) = groups.iter_mut().find(|group| {
                    self.same_delegated_call_signature(
                        &group[0].member.signature,
                        &candidate.member.signature,
                    )
                }) {
                    if !group.iter().any(|earlier| earlier.field == candidate.field) {
                        group.push(candidate);
                    }
                } else {
                    groups.push(vec![candidate]);
                }
            }
        }
        let mut reported_properties = std::collections::HashSet::new();
        let mut slots = Vec::new();
        for group in groups {
            let candidate = group[0].clone();
            let signature = &candidate.member.signature;
            let own_property = signature.accessor.is_some()
                && self.classes[class]
                    .properties
                    .iter()
                    .any(|property| self.properties[*property].name == property_name(signature));
            let existing = if own_property {
                Some(DelegateSelection::Own)
            } else {
                self.existing_delegate_implementation(owner, class, signature)
            };
            let selection = if let Some(existing) = existing {
                existing
            } else if group.len() == 1 {
                DelegateSelection::Forward(ForwardingReceiver::Delegate(candidate.field))
            } else {
                let name = if signature.accessor.is_some() {
                    property_name(signature)
                } else {
                    &signature.name
                };
                if signature.accessor.is_none() || reported_properties.insert(name.to_owned()) {
                    self.error(self.class_fields[candidate.field].span, format!(
                        "multiple interface delegates provide `{name}`; declare an explicit override",
                    ));
                }
                DelegateSelection::Conflict
            };
            slots.push(DelegateSlot {
                candidate,
                selection,
            });
        }
        slots
    }

    fn existing_delegate_implementation(
        &mut self,
        owner: Owner,
        class: ClassId,
        required: &InterfaceSignature,
    ) -> Option<DelegateSelection> {
        let own_owner = self.method_owner_application(owner, self.owner_type_args(owner));
        let mut candidates = self.classes[class]
            .methods
            .iter()
            .map(|function| crate::CallableCandidate::method(*function, own_owner))
            .collect::<Vec<_>>();
        candidates.extend(self.base_chain_methods(class));
        for candidate in candidates {
            if !self.function_is_accessible(candidate.function, None)
                || self.functions[candidate.function].method_type_param_count() != 0
            {
                continue;
            }
            let arguments = self.callable_candidate_owner_arguments(&candidate);
            let signature = self.instantiated_signature(candidate.function, &arguments, &[]);
            let signature = InterfaceSignature::local(
                self.source_function_name(candidate.function),
                &signature,
                self.function_accessor_kind(candidate.function),
            );
            if !self.same_delegated_call_signature(&signature, required) {
                continue;
            }
            if self.function_owner[&candidate.function] == owner {
                return Some(DelegateSelection::Own);
            }
            if self.is_abstract_method(candidate.function) {
                return None;
            }
            let crate::CallableCandidateOwner::Method(application) = candidate.owner else {
                unreachable!("base candidates are methods")
            };
            let owner = self.method_owner_type(application);
            let callee = hir::Callable::Method(
                self.record_method_application(candidate.function, application),
            )
            .into();
            return Some(DelegateSelection::Forward(ForwardingReceiver::Base {
                owner,
                callee,
            }));
        }
        let methods = self.inherited_imported_class_methods(
            class,
            &required.name,
            required.accessor,
            self.classes[class].span,
        );
        let method = methods
            .into_iter()
            .find(|method| self.same_delegated_call_signature(&method.signature, required))?;
        if method.declaration.interface().modality() == hir::CallableModalityV1::Abstract {
            return None;
        }
        let callee = self.interface_delegate_imported_callee(
            method.declaration,
            method.owner,
            hir::MemberCallKind::DirectSuper,
            self.classes[class].span,
        )?;
        Some(DelegateSelection::Forward(ForwardingReceiver::Base {
            owner: method.owner,
            callee,
        }))
    }

    pub(super) fn same_delegated_call_signature(
        &self,
        left: &InterfaceSignature,
        right: &InterfaceSignature,
    ) -> bool {
        left.name == right.name
            && left.accessor == right.accessor
            && left.parameters.len() == right.parameters.len()
            && (left.parameter_pending
                || right.parameter_pending
                || left
                    .parameters
                    .iter()
                    .zip(&right.parameters)
                    .all(|(a, b)| self.types_equal(*a, *b)))
    }
}

pub(super) fn property_name(signature: &InterfaceSignature) -> &str {
    let prefix = match signature.accessor {
        Some(PropertyAccessorKind::Getter) => "$get$",
        Some(PropertyAccessorKind::Setter) => "$set$",
        None => unreachable!("a property member has an accessor role"),
    };
    signature
        .name
        .strip_prefix(prefix)
        .expect("an accessor retains its logical property name")
}
