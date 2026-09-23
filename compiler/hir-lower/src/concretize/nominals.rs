use super::*;

mod c_abi;

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
        for ((interface, source), slot) in &self.interface_slot_by_source {
            let interface_index = interface.into_raw().into_u32() as usize;
            let slot_index = slot.into_raw() as usize;
            assert!(
                interface_slots[interface_index][slot_index]
                    .replace(self.source.dispatch_slot_identities[*source].clone())
                    .is_none(),
                "each LocalConcrete interface slot has one Export HIR origin"
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

    pub(super) fn ensure_struct(
        &mut self,
        source_id: export::StructId,
        arguments: Vec<concrete::TypeId>,
        application: ConcreteApplicationRepresentation,
    ) -> concrete::StructId {
        let key = (source_id, arguments.clone());
        if let Some(&id) = self.struct_by_key.get(&key) {
            return id;
        }
        let source = self.source.structs[source_id].clone();
        assert_eq!(source.type_params.len(), arguments.len());
        let name = self.source_nominal_name(&source.name, source.owner);
        let representation = match (&source.representation, application) {
            (
                export::StructRepresentation::Declared(_),
                ConcreteApplicationRepresentation::Declared,
            ) => concrete::StructRepresentation::Declared {
                attributes: source.attributes,
                c_abi: self.struct_c_abi(source_id),
                fields: Vec::new(),
            },
            (
                export::StructRepresentation::Intrinsic(declaration),
                ConcreteApplicationRepresentation::Intrinsic(application),
            ) => concrete::StructRepresentation::Intrinsic {
                declaration: *declaration,
                application,
            },
            _ => unreachable!("ExportHir declaration and application representations agree"),
        };
        let id = concrete::StructId::from_raw(
            u32::try_from(self.structs.len())
                .expect("concrete struct ids fit in u32")
                .into(),
        );
        let (type_kind, initially_gc_free) = match &representation {
            concrete::StructRepresentation::Intrinsic {
                application: concrete::IntrinsicTypeRepresentation::Integer(kind),
                ..
            } => (concrete::TypeKind::Integer(*kind), true),
            concrete::StructRepresentation::Intrinsic {
                application: concrete::IntrinsicTypeRepresentation::Boolean,
                ..
            } => (concrete::TypeKind::Boolean, true),
            concrete::StructRepresentation::Intrinsic {
                application: concrete::IntrinsicTypeRepresentation::Ptr { pointee },
                ..
            } => (concrete::TypeKind::Ptr(*pointee), true),
            concrete::StructRepresentation::Intrinsic {
                application: concrete::IntrinsicTypeRepresentation::FunPtr { signature },
                ..
            } => (concrete::TypeKind::FunPtr(*signature), true),
            concrete::StructRepresentation::Declared { .. } => {
                (concrete::TypeKind::Struct(id), false)
            }
            concrete::StructRepresentation::Intrinsic { .. } => {
                unreachable!("the registry fixes intrinsic declaration targets")
            }
        };
        let ty = self.intern_type(type_kind, initially_gc_free);
        let allocated = self.structs.alloc(concrete::StructDef {
            origin: self.source.nominal_identities[source_id].clone(),
            canonical_type: ty,
            name,
            owner: self.lower_nominal_owner(source.owner),
            type_arguments: arguments.clone(),
            gc_free: false,
            representation,
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            span: source.span,
        });
        assert_eq!(allocated, id);
        self.struct_by_key.insert(key, id);
        self.struct_source.insert(id, source_id);
        self.struct_type.insert(id, ty);
        let fields: Vec<_> = source
            .semantic_fields()
            .iter()
            .enumerate()
            .map(|(index, field)| concrete::DeclaredStructField {
                identity: self.source.field_identities[export::StructFieldRef::checked(
                    &self.source.structs,
                    source_id,
                    u32::try_from(index).expect("struct field count fits u32"),
                )
                .expect("semantic field indices are valid")]
                .id(),
                name: field.name.clone(),
                ty: self.lower_type(field.ty, &arguments),
            })
            .collect();
        let methods =
            self.request_concrete_methods(&source.methods, concrete::MethodOwner::Struct(id));
        let interface_implementations =
            self.lower_interface_implementations(&source.interface_implementations, &arguments);
        let interfaces = interface_implementations
            .iter()
            .map(|implementation| self.interface_type[&implementation.interface])
            .collect();
        let gc_free = fields.iter().all(|field| self.types[field.ty].gc_free);
        assert!(
            !source.attributes.no_gc || gc_free,
            "HIR diagnoses an invalid @NoGC struct specialization"
        );
        self.structs[id].interfaces = interfaces;
        self.structs[id].interface_implementations = interface_implementations;
        self.structs[id].gc_free = gc_free;
        self.types[ty].gc_free = gc_free;
        match &mut self.structs[id].representation {
            concrete::StructRepresentation::Declared {
                fields: concrete_fields,
                ..
            } => *concrete_fields = fields,
            concrete::StructRepresentation::Intrinsic { .. } => {
                debug_assert!(fields.is_empty() && gc_free);
            }
        }
        self.structs[id].methods = methods;
        if source.type_params.is_empty() {
            for &constructor in &source.constructors {
                if self.automatic_struct_constructor(constructor) {
                    self.request_struct_constructor(constructor, id);
                }
            }
        }
        id
    }

    pub(super) fn lower_struct_constructor(
        &mut self,
        source_id: export::StructConstructorId,
        structure: concrete::StructId,
        substitution: &[concrete::TypeId],
    ) -> PendingStructConstructor {
        let source = self.source.struct_constructors[source_id].clone();
        let parameters = source
            .parameters
            .iter()
            .map(|parameter| concrete::ConstructorParameter {
                id: concrete::ConstructorParamId::from_raw(parameter.id.into_raw()),
                binding: concrete::BindingId::from_raw(parameter.binding.into_raw()),
                definition: parameter.definition,
                name: parameter.name.clone(),
                ty: self.lower_type(parameter.ty, substitution),
            })
            .collect();
        let kind = match &source.kind {
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
            source_discriminator: source_id.into_raw().into_u32(),
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
        self.request_struct_constructor(application.constructor, structure)
    }

    fn lower_constructor_argument_plan(
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

    pub(super) fn ensure_enum(
        &mut self,
        source_id: export::EnumId,
        arguments: Vec<concrete::TypeId>,
    ) -> concrete::EnumId {
        let key = (source_id, arguments.clone());
        if let Some(&id) = self.enum_by_key.get(&key) {
            return id;
        }
        let source = self.source.enums[source_id].clone();
        assert_eq!(source.type_params.len(), arguments.len());
        let name = self.source_nominal_name(&source.name, source.owner);
        let id = concrete::EnumId::from_raw(
            u32::try_from(self.enums.len())
                .expect("concrete enum ids fit in u32")
                .into(),
        );
        let ty = self.intern_type(concrete::TypeKind::Enum(id), false);
        let allocated = self.enums.alloc(concrete::EnumDef {
            origin: self.source.nominal_identities[source_id].clone(),
            canonical_type: ty,
            name,
            owner: self.lower_nominal_owner(source.owner),
            type_arguments: arguments.clone(),
            gc_free: false,
            variants: Vec::new(),
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            span: source.span,
        });
        assert_eq!(allocated, id);
        self.enum_by_key.insert(key, id);
        self.enum_source.insert(id, source_id);
        self.enum_type.insert(id, ty);
        let variants: Vec<_> = source
            .variants
            .iter()
            .enumerate()
            .map(|(variant_index, variant)| {
                let variant_ref = export::EnumVariantRef::checked(
                    &self.source.enums,
                    source_id,
                    u32::try_from(variant_index).expect("source variant indices fit u32"),
                )
                .expect("the source variant exists");
                let fields: Vec<_> = variant
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(field_index, field)| {
                        let field_ref = export::EnumVariantFieldRef::checked(
                            &self.source.enums,
                            variant_ref,
                            u32::try_from(field_index).expect("source field indices fit u32"),
                        )
                        .expect("the source payload field exists");
                        concrete::VariantField {
                            identity: self.source.enum_member_identities[field_ref].id(),
                            name: field.name.clone(),
                            ty: self.lower_type(field.ty, &arguments),
                        }
                    })
                    .collect();
                let gc_free = fields.iter().all(|field| self.types[field.ty].gc_free);
                concrete::Variant {
                    identity: self.source.enum_member_identities[variant_ref].id(),
                    name: variant.name.clone(),
                    gc_free,
                    fields,
                }
            })
            .collect();
        let methods =
            self.request_concrete_methods(&source.methods, concrete::MethodOwner::Enum(id));
        let interface_implementations =
            self.lower_interface_implementations(&source.interface_implementations, &arguments);
        let interfaces = interface_implementations
            .iter()
            .map(|implementation| self.interface_type[&implementation.interface])
            .collect();
        let gc_free = variants.iter().all(|variant| variant.gc_free);
        assert!(
            !source.no_gc || gc_free,
            "HIR diagnoses an invalid @NoGC enum specialization"
        );
        self.enums[id].variants = variants;
        self.enums[id].interfaces = interfaces;
        self.enums[id].interface_implementations = interface_implementations;
        self.enums[id].gc_free = gc_free;
        self.types[ty].gc_free = gc_free;
        self.enums[id].methods = methods;
        id
    }

    pub(super) fn lower_method_dispatch(
        &mut self,
        dispatch: export::MethodDispatch,
        key: &FunctionKey,
    ) -> concrete::MethodDispatch {
        match dispatch {
            export::MethodDispatch::Direct => concrete::MethodDispatch::Direct,
            export::MethodDispatch::Virtual(source) => {
                let next = self.virtual_method_by_source.len() as u32;
                let method = *self
                    .virtual_method_by_source
                    .entry(source)
                    .or_insert_with(|| concrete::VirtualMethodId::from_raw(next));
                concrete::MethodDispatch::Virtual(method)
            }
            export::MethodDispatch::FinalOverride(source) => {
                let next = self.virtual_method_by_source.len() as u32;
                let method = *self
                    .virtual_method_by_source
                    .entry(source)
                    .or_insert_with(|| concrete::VirtualMethodId::from_raw(next));
                concrete::MethodDispatch::FinalOverride(method)
            }
            export::MethodDispatch::Interface(member) => {
                let FunctionKey::Method {
                    owner: concrete::MethodOwner::Interface(interface),
                    ..
                } = *key
                else {
                    unreachable!("interface dispatch belongs to a concrete interface method")
                };
                let slot = self.interface_slot_by_source[&(interface, member)];
                concrete::MethodDispatch::Interface { interface, slot }
            }
        }
    }
}
