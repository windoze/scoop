use super::*;

pub(super) struct ResolvedField<'a> {
    pub identity: scoop_identity::PersistentFieldId,
    pub name: &'a str,
    pub ty: export::TypeId,
}

impl Concretizer<'_> {
    pub(super) fn build_dispatch_slot_identities(&self) -> concrete::DispatchSlotIdentities {
        let mut virtual_slots = vec![None; self.virtual_method_by_source.len()];
        for (source, target) in &self.virtual_method_by_source {
            let position = target.into_raw() as usize;
            assert!(
                virtual_slots[position]
                    .replace(self.source.dispatch_slot_identities[*source].clone())
                    .is_none(),
                "each LocalConcrete virtual family has one Export HIR origin"
            );
        }
        let virtual_slots = virtual_slots
            .into_iter()
            .map(|slot| slot.expect("LocalConcrete virtual family ids are contiguous"))
            .collect();

        let mut interface_slots = self
            .interfaces
            .iter()
            .map(|(_, interface)| vec![None; interface.methods.len()])
            .collect::<Vec<_>>();
        let identities = self
            .source
            .dispatch_slot_identities
            .records()
            .chain(
                self.source
                    .loaded_interface_definitions
                    .values()
                    .flat_map(|interface| interface.methods.iter().map(|method| &method.slot)),
            )
            .map(|record| (record.id(), record))
            .collect::<HashMap<_, _>>();
        for ((interface, source), slot) in &self.interface_slot_by_source {
            let interface_index = interface.into_raw().into_u32() as usize;
            let slot_index = slot.into_raw() as usize;
            assert!(
                interface_slots[interface_index][slot_index]
                    .replace((*identities[source]).clone())
                    .is_none(),
                "each concrete interface slot retains its actual declaration identity"
            );
        }
        let interface_slots = interface_slots
            .into_iter()
            .map(|slots| {
                slots
                    .into_iter()
                    .map(|slot| slot.expect("LocalConcrete interface slot ids are contiguous"))
                    .collect()
            })
            .collect();

        concrete::DispatchSlotIdentities::checked(virtual_slots, interface_slots, &self.interfaces)
            .expect("validated Export HIR dispatch identities survive concretization")
    }

    pub(super) fn lower_struct_constructor(
        &mut self,
        source_id: export::StructConstructorDefinition,
        structure: concrete::StructId,
        substitution: &[concrete::TypeId],
    ) -> PendingStructConstructor {
        let source = self.struct_constructor_definition(source_id);
        let parameters = self.lower_constructor_parameters(source.parameters, substitution);
        let kind = match source.kind {
            export::StructConstructorKind::Primary => concrete::StructConstructorKind::Primary,
            export::StructConstructorKind::Secondary {
                delegation,
                body,
                gc_effect,
            } => {
                let target =
                    self.lower_struct_constructor_application(delegation.target, substitution);
                let (argument_body, locals) =
                    self.lower_constructor_argument_plan(&delegation.arguments, substitution);
                let (body, _) = self.lower_body(body, substitution);
                debug_assert_eq!(argument_body.locals.len(), locals.len());
                concrete::StructConstructorKind::Secondary {
                    gc_effect: *gc_effect,
                    target,
                    arguments: argument_body,
                    body,
                }
            }
        };
        PendingStructConstructor {
            structure,
            source_discriminator: source.discriminator,
            safety: source.safety,
            origin: source.origin,
            parameters,
            kind,
        }
    }

    pub(super) fn lower_struct_constructor_application(
        &mut self,
        source: export::StructConstructorApplicationId,
        substitution: &[concrete::TypeId],
    ) -> concrete::StructConstructorId {
        let application = &self.source.struct_constructor_applications[source];
        let structure = self.lower_struct_application(application.owner, substitution);
        self.request_struct_constructor_source(application.constructor, structure)
    }

    pub(super) fn lower_constructor_argument_plan(
        &mut self,
        source: &export::ConstructorArguments,
        substitution: &[concrete::TypeId],
    ) -> (concrete::ConstructorArguments, Vec<concrete::LocalId>) {
        let (locals, local_map) = self.lower_locals(&source.locals, substitution);
        let statements = self.lower_statement_region(&source.statements, substitution, &local_map);
        let args = source
            .args
            .iter()
            .map(|argument| self.lower_expr(argument, substitution, &local_map))
            .collect();
        (
            concrete::ConstructorArguments {
                locals,
                statements,
                args,
            },
            local_map,
        )
    }

    pub(in crate::concretize) fn lower_virtual_method(
        &mut self,
        source: export::VirtualMethodId,
    ) -> concrete::VirtualMethodId {
        let next = self.virtual_method_by_source.len() as u32;
        *self
            .virtual_method_by_source
            .entry(source)
            .or_insert_with(|| concrete::VirtualMethodId::from_raw(next))
    }
}
