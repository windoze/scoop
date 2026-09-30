use std::collections::HashSet;

use super::*;

pub(super) struct InterfaceMethodInstance<'a> {
    pub method: ResolvedInterfaceMethod<'a>,
    pub arguments: Vec<concrete::TypeId>,
}

impl<'input> Concretizer<'input> {
    pub(super) fn interface_method_instances(
        &mut self,
        ty: export::TypeId,
        substitution: &[concrete::TypeId],
    ) -> Vec<InterfaceMethodInstance<'input>> {
        let mut result = Vec::new();
        self.collect_interface_method_instances(ty, substitution, &mut HashSet::new(), &mut result);
        let suppressed = result
            .iter()
            .flat_map(|instance| instance.method.overrides.iter().copied())
            .collect::<HashSet<_>>();
        let mut slots = HashSet::new();
        result.retain(|instance| {
            !suppressed.contains(&instance.method.slot) && slots.insert(instance.method.slot)
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
        seen: &mut HashSet<(export::SourceNominalId, Vec<concrete::TypeId>)>,
        out: &mut Vec<InterfaceMethodInstance<'input>>,
    ) {
        let source = self.source;
        match &source.types[ty] {
            export::Type::Interface(application) => {
                let application = &source.interface_applications[*application];
                let arguments = application
                    .arguments
                    .iter()
                    .map(|argument| self.lower_type(*argument, substitution))
                    .collect::<Vec<_>>();
                let declaration = &source.interfaces[application.template];
                let origin = source.nominal_identities[application.template].declaration_id();
                if !seen.insert((origin, arguments.clone())) {
                    return;
                }
                for &parent in &declaration.parents {
                    self.collect_interface_method_instances(parent, &arguments, seen, out);
                }
                out.extend(
                    declaration
                        .methods
                        .iter()
                        .map(|&member| InterfaceMethodInstance {
                            method: self.source_interface_method(member),
                            arguments: arguments.clone(),
                        }),
                );
            }
            export::Type::ImportedInterface(interface) => {
                let arguments = interface
                    .arguments
                    .iter()
                    .map(|argument| self.lower_type(*argument, substitution))
                    .collect::<Vec<_>>();
                if !seen.insert((interface.declaration.owner(), arguments)) {
                    return;
                }
                out.extend(
                    interface
                        .methods
                        .iter()
                        .map(|method| InterfaceMethodInstance {
                            method: ResolvedInterfaceMethod::from_dependency(method),
                            arguments: substitution.to_vec(),
                        }),
                );
            }
            _ => unreachable!("interface members are reached through interface types"),
        }
    }
}
