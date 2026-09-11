use super::*;

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
        let declaration_name = self.source_nominal_name(&source.name, source.owner);
        let name = self.instance_name(&declaration_name, &arguments);
        let representation = match (&source.representation, application) {
            (
                export::StructRepresentation::Declared(_),
                ConcreteApplicationRepresentation::Declared,
            ) => concrete::StructRepresentation::Declared {
                attributes: source.attributes,
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
            link_stem: source.link_stem.clone(),
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
        if matches!(
            source.representation,
            export::StructRepresentation::Declared(_)
        ) {
            for &constructor in &source.constructors {
                let raw = self.struct_constructor_slots.len() as u32;
                self.struct_constructor_slots.push(None);
                self.struct_constructor_keys.push((constructor, id));
                let concrete = concrete::StructConstructorId::from_raw(raw.into());
                assert!(
                    self.struct_constructor_by_key
                        .insert((constructor, id), concrete)
                        .is_none()
                );
            }
        }
        let fields: Vec<_> = source
            .semantic_fields()
            .iter()
            .map(|field| concrete::Field {
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
        for &constructor in &source.constructors {
            let concrete = self.lower_struct_constructor(constructor, id, &arguments);
            let target = self.struct_constructor_by_key[&(constructor, id)];
            let slot = target.into_raw().into_u32() as usize;
            assert!(
                self.struct_constructor_slots[slot]
                    .replace(concrete)
                    .is_none()
            );
        }
        id
    }

    fn lower_struct_constructor(
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
                name: parameter.name.clone(),
                ty: self.lower_type(parameter.ty, substitution),
            })
            .collect();
        let kind = match &source.kind {
            export::StructConstructorKind::Primary => concrete::StructConstructorKind::Primary,
            export::StructConstructorKind::Secondary { delegation, body } => {
                let target =
                    self.lower_struct_constructor_application(delegation.target, substitution);
                let (argument_body, locals) =
                    self.lower_constructor_argument_plan(&delegation.arguments, substitution);
                let (body, _) = self.lower_body(body, substitution);
                debug_assert_eq!(argument_body.locals.len(), locals.len());
                concrete::StructConstructorKind::Secondary {
                    target,
                    arguments: argument_body,
                    body,
                }
            }
        };
        PendingStructConstructor {
            structure,
            source_discriminator: source_id.into_raw().into_u32(),
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
        self.struct_constructor_by_key[&(application.constructor, structure)]
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
        let declaration_name = self.source_nominal_name(&source.name, source.owner);
        let name = self.instance_name(&declaration_name, &arguments);
        let id = concrete::EnumId::from_raw(
            u32::try_from(self.enums.len())
                .expect("concrete enum ids fit in u32")
                .into(),
        );
        let ty = self.intern_type(concrete::TypeKind::Enum(id), false);
        let allocated = self.enums.alloc(concrete::EnumDef {
            origin: self.source.nominal_identities[source_id].clone(),
            canonical_type: ty,
            link_stem: source.link_stem.clone(),
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
            .map(|variant| {
                let fields: Vec<_> = variant
                    .fields
                    .iter()
                    .map(|field| concrete::Field {
                        name: field.name.clone(),
                        ty: self.lower_type(field.ty, &arguments),
                    })
                    .collect();
                let gc_free = fields.iter().all(|field| self.types[field.ty].gc_free);
                concrete::Variant {
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

    pub(super) fn ensure_interface(
        &mut self,
        source_id: export::InterfaceId,
        arguments: Vec<concrete::TypeId>,
    ) -> concrete::InterfaceId {
        let key = (source_id, arguments.clone());
        if let Some(&id) = self.interface_by_key.get(&key) {
            return id;
        }
        let source = self.source.interfaces[source_id].clone();
        assert_eq!(source.type_params.len(), arguments.len());
        let declaration_name = self.source_nominal_name(&source.name, source.owner);
        let name = self.instance_name(&declaration_name, &arguments);
        let id = concrete::InterfaceId::from_raw(
            u32::try_from(self.interfaces.len())
                .expect("concrete interface ids fit in u32")
                .into(),
        );
        let ty = self.intern_type(concrete::TypeKind::Interface(id), false);
        let allocated = self.interfaces.alloc(concrete::InterfaceDef {
            origin: self.source.nominal_identities[source_id].clone(),
            canonical_type: ty,
            link_stem: source.link_stem.clone(),
            name,
            owner: self.lower_nominal_owner(source.owner),
            family: concrete::InterfaceFamilyId::from_raw(source_id.into_raw().into_u32()),
            type_arguments: arguments.clone(),
            methods: Vec::new(),
            span: source.span,
        });
        assert_eq!(allocated, id);
        self.interface_by_key.insert(key, id);
        self.interface_type.insert(id, ty);
        let method_instances = self.interface_method_instances(source.self_application, &arguments);
        let methods = method_instances
            .iter()
            .enumerate()
            .map(|(index, (method, method_arguments))| {
                self.interface_slot_by_source.insert(
                    (id, *method),
                    concrete::InterfaceMethodSlot::from_raw(index as u32),
                );
                let function =
                    &self.source.functions[self.source.interface_methods[*method].function];
                let implementation = match self.source.interface_methods[*method].implementation {
                    export::InterfaceMemberImplementation::Body => {
                        concrete::InterfaceMemberImplementation::Body
                    }
                    export::InterfaceMemberImplementation::AbstractSlot => {
                        concrete::InterfaceMemberImplementation::AbstractSlot
                    }
                };
                concrete::MethodSig {
                    name: function
                        .name
                        .rsplit('.')
                        .next()
                        .expect("interface methods are qualified")
                        .to_string(),
                    is_suspend: function.is_suspend,
                    attributes: function.attributes,
                    implementation,
                    params: function
                        .params
                        .iter()
                        .skip(1)
                        .map(|param| concrete::Param {
                            name: param.name.clone(),
                            ty: self.lower_type(param.ty, method_arguments),
                            local: remap_idx(param.local),
                        })
                        .collect(),
                    return_ty: self.lower_type(function.return_ty, method_arguments),
                    span: function.span,
                }
            })
            .collect();
        self.interfaces[id].methods = methods;
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

    pub(super) fn interface_method_instances(
        &mut self,
        application: export::InterfaceApplicationId,
        substitution: &[concrete::TypeId],
    ) -> Vec<(export::InterfaceMethodId, Vec<concrete::TypeId>)> {
        let mut result = Vec::new();
        let mut seen = Vec::new();
        self.collect_interface_method_instances(application, substitution, &mut seen, &mut result);
        let suppressed = result
            .iter()
            .flat_map(|(member, _)| {
                self.source.interface_methods[*member]
                    .overrides
                    .iter()
                    .copied()
            })
            .collect::<Vec<_>>();
        result.retain(|(member, _)| !suppressed.contains(member));
        result
    }

    pub(super) fn collect_interface_method_instances(
        &mut self,
        application: export::InterfaceApplicationId,
        substitution: &[concrete::TypeId],
        seen: &mut Vec<(export::InterfaceId, Vec<concrete::TypeId>)>,
        out: &mut Vec<(export::InterfaceMethodId, Vec<concrete::TypeId>)>,
    ) {
        let application = self.source.interface_applications[application].clone();
        let arguments = application
            .arguments
            .iter()
            .map(|argument| self.lower_type(*argument, substitution))
            .collect::<Vec<_>>();
        let key = (application.template, arguments.clone());
        if seen.contains(&key) {
            return;
        }
        seen.push(key);
        let declaration = self.source.interfaces[application.template].clone();
        out.extend(
            declaration
                .methods
                .iter()
                .map(|&member| (member, arguments.clone())),
        );
        for parent in declaration.parents {
            self.collect_interface_method_instances(parent, &arguments, seen, out);
        }
    }

    pub(super) fn instance_name(&self, base: &str, arguments: &[concrete::TypeId]) -> String {
        if arguments.is_empty() {
            base.to_string()
        } else {
            let arguments = arguments
                .iter()
                .map(|argument| self.encode_type(*argument))
                .collect::<Vec<_>>()
                .join("_");
            format!("{base}${arguments}")
        }
    }

    pub(super) fn encode_type(&self, ty: concrete::TypeId) -> String {
        match &self.types[ty].kind {
            concrete::TypeKind::Unit => "U".to_string(),
            concrete::TypeKind::Integer(kind) => match *kind {
                export::IntegerKind::SIGNED_8 => "I8".to_string(),
                export::IntegerKind::SIGNED_16 => "I16".to_string(),
                export::IntegerKind::SIGNED_32 => "I32".to_string(),
                export::IntegerKind::SIGNED_64 => "I64".to_string(),
                export::IntegerKind::UNSIGNED_8 => "V8".to_string(),
                export::IntegerKind::UNSIGNED_16 => "V16".to_string(),
                export::IntegerKind::UNSIGNED_32 => "V32".to_string(),
                export::IntegerKind::UNSIGNED_64 => "V64".to_string(),
            },
            concrete::TypeKind::Boolean => "B".to_string(),
            concrete::TypeKind::String => "S".to_string(),
            concrete::TypeKind::Struct(id) => {
                let name = &self.structs[*id].name;
                format!("D{}_{}X", name.len(), name)
            }
            concrete::TypeKind::Class(id) => match &self.classes[*id].representation {
                concrete::ClassRepresentation::Intrinsic {
                    application: concrete::IntrinsicTypeRepresentation::Array { element },
                    ..
                } => format!("A{}X", self.encode_type(*element)),
                concrete::ClassRepresentation::Intrinsic {
                    application: concrete::IntrinsicTypeRepresentation::MutableArray { element },
                    ..
                } => format!("M{}X", self.encode_type(*element)),
                concrete::ClassRepresentation::Declared { .. }
                | concrete::ClassRepresentation::Intrinsic {
                    application: concrete::IntrinsicTypeRepresentation::String,
                    ..
                } => {
                    let name = &self.classes[*id].name;
                    format!("C{}_{}X", name.len(), name)
                }
                concrete::ClassRepresentation::Intrinsic { .. } => {
                    unreachable!("the intrinsic registry fixes declaration targets")
                }
            },
            concrete::TypeKind::Interface(id) => {
                let name = &self.interfaces[*id].name;
                format!("J{}_{}X", name.len(), name)
            }
            concrete::TypeKind::Any => "Any".to_string(),
            concrete::TypeKind::Tuple(elements) => format!(
                "T{}X",
                elements
                    .iter()
                    .map(|element| self.encode_type(*element))
                    .collect::<Vec<_>>()
                    .join("_")
            ),
            concrete::TypeKind::Function(id) => {
                let function = &self.function_types[*id];
                let kind = if function.is_suspend { "S" } else { "F" };
                let parameters = function
                    .parameter_types
                    .iter()
                    .map(|ty| self.encode_type(*ty))
                    .collect::<Vec<_>>()
                    .join("_");
                format!(
                    "{kind}{parameters}R{}X",
                    self.encode_type(function.return_type)
                )
            }
            concrete::TypeKind::Ptr(pointee) => format!("P{}X", self.encode_type(*pointee)),
            concrete::TypeKind::FunPtr(id) => {
                let function = &self.function_types[*id];
                let parameters = function
                    .parameter_types
                    .iter()
                    .map(|ty| self.encode_type(*ty))
                    .collect::<Vec<_>>()
                    .join("_");
                format!("N{parameters}R{}X", self.encode_type(function.return_type))
            }
            concrete::TypeKind::Enum(id) => {
                let name = &self.enums[*id].name;
                format!("E{}_{}X", name.len(), name)
            }
        }
    }
}
