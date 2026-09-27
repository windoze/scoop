use super::*;

pub(super) enum InterfaceMethodInstance {
    Local {
        member: export::InterfaceMethodId,
        arguments: Vec<concrete::TypeId>,
    },
    Imported(Box<export::ImportedInterfaceMethod>),
}

impl InterfaceMethodInstance {
    pub(super) fn slot(&self, source: &export::Module) -> scoop_identity::PersistentDispatchSlotId {
        match self {
            Self::Local { member, .. } => source.dispatch_slot_identities[*member].id(),
            Self::Imported(method) => method.slot.id(),
        }
    }
}

impl Concretizer<'_> {
    pub(super) fn interface_method_instances(
        &mut self,
        ty: export::TypeId,
        substitution: &[concrete::TypeId],
    ) -> Vec<InterfaceMethodInstance> {
        let mut result = Vec::new();
        let mut seen = Vec::new();
        self.collect_interface_method_instances(ty, substitution, &mut seen, &mut result);
        let mut suppressed = std::collections::HashSet::new();
        for member in &result {
            match member {
                InterfaceMethodInstance::Local { member, .. } => {
                    for reference in &self.source.interface_methods[*member].overrides {
                        suppressed.insert(self.interface_reference_slot(*reference));
                    }
                }
                InterfaceMethodInstance::Imported(method) => {
                    suppressed.extend(method.overrides.iter().copied());
                }
            }
        }
        let mut slots = std::collections::HashSet::new();
        result.retain(|member| {
            let slot = member.slot(self.source);
            !suppressed.contains(&slot) && slots.insert(slot)
        });
        result
    }

    pub(in crate::concretize) fn interface_reference_slot(
        &self,
        reference: export::InterfaceMethodReference,
    ) -> scoop_identity::PersistentDispatchSlotId {
        match reference {
            export::InterfaceMethodReference::Local(member) => {
                self.source.dispatch_slot_identities[member].id()
            }
            export::InterfaceMethodReference::Imported { slot, .. } => slot,
        }
    }

    fn collect_interface_method_instances(
        &mut self,
        ty: export::TypeId,
        substitution: &[concrete::TypeId],
        seen: &mut Vec<(export::TypeId, Vec<concrete::TypeId>)>,
        out: &mut Vec<InterfaceMethodInstance>,
    ) {
        match self.source.types[ty].clone() {
            export::Type::Interface(application) => {
                let application = self.source.interface_applications[application].clone();
                let arguments = application
                    .arguments
                    .iter()
                    .map(|argument| self.lower_type(*argument, substitution))
                    .collect::<Vec<_>>();
                let declaration = self.source.interfaces[application.template].clone();
                let key = (
                    self.source.interface_applications[declaration.self_application].canonical_type,
                    arguments.clone(),
                );
                if seen.contains(&key) {
                    return;
                }
                seen.push(key);
                for parent in declaration.parents {
                    self.collect_interface_method_instances(parent, &arguments, seen, out);
                }
                out.extend(declaration.methods.into_iter().map(|member| {
                    InterfaceMethodInstance::Local {
                        member,
                        arguments: arguments.clone(),
                    }
                }));
            }
            export::Type::ImportedInterface(interface) => {
                let key = (ty, Vec::new());
                if seen.contains(&key) {
                    return;
                }
                seen.push(key);
                out.extend(
                    interface
                        .methods
                        .iter()
                        .cloned()
                        .map(|method| InterfaceMethodInstance::Imported(Box::new(method))),
                );
            }
            _ => unreachable!("interface members are reached through interface types"),
        }
    }
}
