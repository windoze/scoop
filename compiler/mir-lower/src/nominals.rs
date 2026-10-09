use super::*;

impl Lowerer {
    /// Reserve raw-id-preserving nominal maps before any intrinsic pointer
    /// shell lowers its complete pointee type. The subsequent arena
    /// allocations assert this one-to-one ordering.
    pub(super) fn reserve_nominal_ids(&mut self, module: &hir::Module) {
        for (hir_id, _) in module.structs.iter() {
            self.struct_map.insert(hir_id, remap_idx(hir_id));
        }
        for (hir_id, _) in module.classes.iter() {
            self.class_map.insert(hir_id, remap_idx(hir_id));
        }
    }

    /// Transpose local-concrete HIR structs into MIR in declaration order
    /// (ids only; field types are filled by `fill_struct_fields`).
    pub(super) fn lower_structs(&mut self, module: &hir::Module) {
        for (hir_id, decl) in module.structs.iter() {
            let representation = match &decl.representation {
                hir::StructRepresentation::Declared {
                    attributes, c_abi, ..
                } => mir::StructRepresentation::Declared {
                    c_abi: match c_abi {
                        hir::StructCAbi::SourceRepresentation => {
                            mir::StructCAbi::SourceRepresentation
                        }
                        hir::StructCAbi::UInt64Field { field } => {
                            mir::StructCAbi::UInt64Field { field: *field }
                        }
                    },
                    c_layout: attributes.c_layout.map(|layout| mir::MirCLayoutContract {
                        aligned: lower_c_layout_value(layout.aligned),
                        packed: lower_c_layout_value(layout.packed),
                    }),
                    interior_mutable: attributes.interior_mutable,
                    fields: Vec::new(),
                },
                hir::StructRepresentation::Intrinsic { application, .. } => {
                    mir::StructRepresentation::Intrinsic(match application {
                        hir::IntrinsicTypeRepresentation::MaybeUninit { value } => {
                            let types = Types {
                                module,
                                struct_map: &self.struct_map,
                                class_map: &self.class_map,
                            };
                            mir::IntrinsicTypeRepresentation::MaybeUninit {
                                value: types.lower(
                                    *value,
                                    &mut self.source_exact_types,
                                    &mut self.enums,
                                    &mut self.structs,
                                    &mut self.interfaces,
                                    &mut self.shell,
                                ),
                            }
                        }
                        hir::IntrinsicTypeRepresentation::Unit => {
                            mir::IntrinsicTypeRepresentation::Unit
                        }
                        hir::IntrinsicTypeRepresentation::Integer(kind) => {
                            mir::IntrinsicTypeRepresentation::Integer(lower_integer_kind(*kind))
                        }
                        hir::IntrinsicTypeRepresentation::Float(kind) => {
                            mir::IntrinsicTypeRepresentation::Float(*kind)
                        }
                        hir::IntrinsicTypeRepresentation::Char => {
                            mir::IntrinsicTypeRepresentation::Char
                        }
                        hir::IntrinsicTypeRepresentation::Boolean => {
                            mir::IntrinsicTypeRepresentation::Boolean
                        }
                        hir::IntrinsicTypeRepresentation::Ptr { pointee } => {
                            let types = Types {
                                module,
                                struct_map: &self.struct_map,
                                class_map: &self.class_map,
                            };
                            mir::IntrinsicTypeRepresentation::Ptr {
                                pointee: types.lower(
                                    *pointee,
                                    &mut self.source_exact_types,
                                    &mut self.enums,
                                    &mut self.structs,
                                    &mut self.interfaces,
                                    &mut self.shell,
                                ),
                            }
                        }
                        hir::IntrinsicTypeRepresentation::FunPtr { signature } => {
                            mir::IntrinsicTypeRepresentation::FunPtr {
                                signature: remap_idx(*signature),
                            }
                        }
                        hir::IntrinsicTypeRepresentation::String
                        | hir::IntrinsicTypeRepresentation::Atomic(_)
                        | hir::IntrinsicTypeRepresentation::Any
                        | hir::IntrinsicTypeRepresentation::Nothing
                        | hir::IntrinsicTypeRepresentation::Array { .. }
                        | hir::IntrinsicTypeRepresentation::MutableArray { .. } => {
                            unreachable!("the registry fixes intrinsic declaration targets")
                        }
                    })
                }
            };
            let mir_id = self.structs.defs.alloc(mir::StructDef {
                name: decl.name.clone(),
                type_arguments: Vec::new(),
                gc_free: decl.gc_free,
                representation,
            });
            assert_eq!(mir_id, self.struct_map[&hir_id]);
            self.structs.hir_ids.insert(mir_id, hir_id);
        }
    }

    /// Fill the exact arguments of eagerly reserved struct/class
    /// applications after the initial type context contains every nominal.
    pub(super) fn fill_nominal_type_arguments(&mut self, module: &hir::Module) {
        for (hir_id, declaration) in module.structs.iter() {
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let arguments = declaration
                .type_arguments
                .iter()
                .map(|argument| {
                    types.lower(
                        *argument,
                        &mut self.source_exact_types,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    )
                })
                .collect::<Vec<_>>();
            let mir_id = self.struct_map[&hir_id];
            self.structs.defs[mir_id]
                .type_arguments
                .clone_from(&arguments);
            self.shell.structs[mir_id].type_arguments = arguments;
            types.lower(
                declaration.canonical_type,
                &mut self.source_exact_types,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
        }
        for (hir_id, declaration) in module.classes.iter() {
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let arguments = declaration
                .type_arguments
                .iter()
                .map(|argument| {
                    types.lower(
                        *argument,
                        &mut self.source_exact_types,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    )
                })
                .collect::<Vec<_>>();
            let mir_id = self.class_map[&hir_id];
            self.classes[mir_id].type_arguments.clone_from(&arguments);
            self.shell.classes[mir_id].type_arguments = arguments;
            types.lower(
                declaration.canonical_type,
                &mut self.source_exact_types,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
        }
        // Owned non-generic declarations include private types referenced only
        // by exported default arguments. Foreign and generic applications keep
        // their existing use-driven materialization path.
        for declaration in module
            .enums
            .iter()
            .map(|(_, declaration)| declaration)
            .filter(|declaration| {
                declaration.type_arguments.is_empty()
                    && declaration.origin.source().is_some_and(|source| {
                        source.concrete_id().is_some()
                            && source.declaration().origin() == module.cone
                    })
            })
        {
            Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            }
            .lower(
                declaration.canonical_type,
                &mut self.source_exact_types,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
        }
    }

    /// Fill the MIR struct field types. This runs after the type context
    /// exists because field types can reference concrete enums.
    pub(super) fn fill_struct_fields(&mut self, module: &hir::Module) {
        for (hir_id, decl) in module.structs.iter() {
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let fields = match &decl.representation {
                hir::StructRepresentation::Declared { fields, .. } => fields
                    .iter()
                    .map(|field| mir::DeclaredStructField {
                        identity: field.identity,
                        name: field.name.clone(),
                        ty: types.lower(
                            field.ty,
                            &mut self.source_exact_types,
                            &mut self.enums,
                            &mut self.structs,
                            &mut self.interfaces,
                            &mut self.shell,
                        ),
                    })
                    .collect(),
                hir::StructRepresentation::Intrinsic { .. } => Vec::new(),
            };
            let mir_id = self.struct_map[&hir_id];
            match &mut self.structs.defs[mir_id].representation {
                mir::StructRepresentation::Declared {
                    fields: mir_fields, ..
                } => *mir_fields = fields,
                mir::StructRepresentation::Intrinsic(_) => debug_assert!(fields.is_empty()),
            }
        }
    }

    /// Materialize every local-concrete interface eagerly.
    pub(super) fn lower_interfaces(&mut self, module: &hir::Module) {
        for (hir_id, decl) in module.interfaces.iter() {
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let lowered = types.lower(
                decl.canonical_type,
                &mut self.source_exact_types,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
            assert!(
                matches!(lowered, mir::Type::Interface(id) if self.interfaces.source(id).0 == hir_id),
                "the canonical interface type retains its physical declaration"
            );
        }
    }

    /// Transpose the HIR class arena into MIR in declaration order.
    /// Fields / vtable / itables are filled later (they need the base
    /// class and the method list, respectively).
    pub(super) fn declare_classes(&mut self, module: &hir::Module) {
        for (hir_id, decl) in module.classes.iter() {
            let modifier = match decl.modifier {
                hir::ClassModifier::Final => mir::ClassModifier::Final,
                hir::ClassModifier::Open => mir::ClassModifier::Open,
                hir::ClassModifier::Abstract => mir::ClassModifier::Abstract,
            };
            let representation = match &decl.representation {
                hir::ClassRepresentation::Declared { .. } => mir::ClassRepresentation::Declared {
                    fields: Vec::new(),
                    base_class: None,
                },
                hir::ClassRepresentation::Intrinsic { application, .. } => {
                    let types = Types {
                        module,
                        struct_map: &self.struct_map,
                        class_map: &self.class_map,
                    };
                    mir::ClassRepresentation::Intrinsic(match application {
                        hir::IntrinsicTypeRepresentation::Atomic(storage) => {
                            mir::IntrinsicTypeRepresentation::Atomic(storage.clone().map(|ty| {
                                types.lower(
                                    ty,
                                    &mut self.source_exact_types,
                                    &mut self.enums,
                                    &mut self.structs,
                                    &mut self.interfaces,
                                    &mut self.shell,
                                )
                            }))
                        }
                        hir::IntrinsicTypeRepresentation::Any => {
                            mir::IntrinsicTypeRepresentation::Any
                        }
                        hir::IntrinsicTypeRepresentation::Nothing => {
                            mir::IntrinsicTypeRepresentation::Nothing
                        }
                        hir::IntrinsicTypeRepresentation::String => {
                            mir::IntrinsicTypeRepresentation::String
                        }
                        hir::IntrinsicTypeRepresentation::Array { element } => {
                            mir::IntrinsicTypeRepresentation::Array {
                                element: types.lower(
                                    *element,
                                    &mut self.source_exact_types,
                                    &mut self.enums,
                                    &mut self.structs,
                                    &mut self.interfaces,
                                    &mut self.shell,
                                ),
                            }
                        }
                        hir::IntrinsicTypeRepresentation::MutableArray { element } => {
                            mir::IntrinsicTypeRepresentation::MutableArray {
                                element: types.lower(
                                    *element,
                                    &mut self.source_exact_types,
                                    &mut self.enums,
                                    &mut self.structs,
                                    &mut self.interfaces,
                                    &mut self.shell,
                                ),
                            }
                        }
                        hir::IntrinsicTypeRepresentation::Unit
                        | hir::IntrinsicTypeRepresentation::MaybeUninit { .. }
                        | hir::IntrinsicTypeRepresentation::Integer(_)
                        | hir::IntrinsicTypeRepresentation::Float(_)
                        | hir::IntrinsicTypeRepresentation::Char
                        | hir::IntrinsicTypeRepresentation::Boolean
                        | hir::IntrinsicTypeRepresentation::Ptr { .. }
                        | hir::IntrinsicTypeRepresentation::FunPtr { .. } => {
                            unreachable!("the registry fixes intrinsic declaration targets")
                        }
                    })
                }
            };
            let mir_id = self.classes.alloc(mir::ClassDef {
                release_policy: Default::default(),
                modifier,
                name: decl.name.clone(),
                type_arguments: Vec::new(),
                representation,
                interfaces: Vec::new(),
                vtable: Vec::new(),
                itables: Vec::new(),
            });
            assert_eq!(mir_id, self.class_map[&hir_id]);
        }
    }

    /// Resolve base classes and concrete interface applications after the
    /// type context contains every class name. Interface type arguments may
    /// themselves be class types.
    pub(super) fn fill_class_hierarchy(&mut self, module: &hir::Module) {
        for (hir_id, decl) in module.classes.iter() {
            let mir_id = self.class_map[&hir_id];
            let base_class = decl.base_class().map(|base| self.class_map[&base]);
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let interfaces = decl
                .interfaces
                .iter()
                .map(|&interface_ty| {
                    let lowered = types.lower(
                        interface_ty,
                        &mut self.source_exact_types,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    );
                    let mir::Type::Interface(interface) = lowered else {
                        unreachable!("HIR implementation lists contain only interfaces")
                    };
                    interface
                })
                .collect();
            let class = &mut self.classes[mir_id];
            match &mut class.representation {
                mir::ClassRepresentation::Declared {
                    base_class: mir_base,
                    ..
                } => *mir_base = base_class,
                mir::ClassRepresentation::Intrinsic(_) => debug_assert!(base_class.is_none()),
            }
            class.interfaces = interfaces;
        }
    }

    /// Declare one local-concrete user function; its body is filled later.
    pub(super) fn declare_function(
        &mut self,
        module: &hir::Module,
        hir_id: hir::FunctionId,
    ) -> mir::FunctionId {
        let function = &module.functions[hir_id];
        let name = fn_name(function);
        let id = self.functions.alloc(mir::Function {
            gc_effect: if matches!(function.kind, hir::FunctionKind::Extern(id)
                if !matches!(module.extern_functions[id].abi,
                    hir::ExternAbi::C(scoop_identity::CAbiCallMode::GcLeaf)))
            {
                mir::GcEffect::Managed
            } else {
                lower_gc_effect(function.attributes.gc_effect)
            },
            name,
            // Filled in when the body is lowered below.
            params: Vec::new(),
            return_ty: mir::Type::Unit,
            body: mir::Body::unreachable(Arena::new()),
        });
        self.top_level.push(id);
        self.function_map.insert(hir_id, id);
        self.source_callables.record_function(module, id, hir_id);
        self.record_function_instance(module, hir_id, id);
        id
    }

    pub(super) fn lower_function_type_id(
        &mut self,
        module: &hir::Module,
        id: hir::FunctionTypeId,
    ) -> mir::FunctionTypeId {
        let ty = module.function_types[id].canonical_type;
        let lowered = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        }
        .lower(
            ty,
            &mut self.source_exact_types,
            &mut self.enums,
            &mut self.structs,
            &mut self.interfaces,
            &mut self.shell,
        );
        let mir::Type::Function(id) = lowered else {
            unreachable!("lowering a function type preserves its category")
        };
        id
    }
}

const fn lower_c_layout_value(value: hir::HirCLayoutValue) -> mir::MirCLayoutValue {
    match value {
        hir::HirCLayoutValue::Natural => mir::MirCLayoutValue::Natural,
        hir::HirCLayoutValue::A1 => mir::MirCLayoutValue::A1,
        hir::HirCLayoutValue::A2 => mir::MirCLayoutValue::A2,
        hir::HirCLayoutValue::A4 => mir::MirCLayoutValue::A4,
        hir::HirCLayoutValue::A8 => mir::MirCLayoutValue::A8,
        hir::HirCLayoutValue::A16 => mir::MirCLayoutValue::A16,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_layout_contract_values_transpose_exhaustively() {
        let source = [
            hir::HirCLayoutValue::Natural,
            hir::HirCLayoutValue::A1,
            hir::HirCLayoutValue::A2,
            hir::HirCLayoutValue::A4,
            hir::HirCLayoutValue::A8,
            hir::HirCLayoutValue::A16,
        ];
        let expected = [
            mir::MirCLayoutValue::Natural,
            mir::MirCLayoutValue::A1,
            mir::MirCLayoutValue::A2,
            mir::MirCLayoutValue::A4,
            mir::MirCLayoutValue::A8,
            mir::MirCLayoutValue::A16,
        ];
        assert_eq!(source.map(lower_c_layout_value), expected);
    }
}
