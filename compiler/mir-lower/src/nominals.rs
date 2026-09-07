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
                hir::StructRepresentation::Declared { attributes, .. } => {
                    mir::StructRepresentation::Declared {
                        c_layout: attributes.c_layout.map(|layout| mir::MirCLayoutContract {
                            aligned: lower_c_layout_value(layout.aligned),
                            packed: lower_c_layout_value(layout.packed),
                        }),
                        interior_mutable: attributes.interior_mutable,
                        fields: Vec::new(),
                    }
                }
                hir::StructRepresentation::Intrinsic { application, .. } => {
                    mir::StructRepresentation::Intrinsic(match application {
                        hir::IntrinsicTypeRepresentation::Integer(kind) => {
                            mir::IntrinsicTypeRepresentation::Integer(lower_integer_kind(*kind))
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
                        | hir::IntrinsicTypeRepresentation::Array { .. }
                        | hir::IntrinsicTypeRepresentation::MutableArray { .. } => {
                            unreachable!("the registry fixes intrinsic declaration targets")
                        }
                    })
                }
            };
            let mir_id = self.structs.defs.alloc(mir::StructDef {
                name: decl.name.clone(),
                gc_free: decl.gc_free,
                representation,
            });
            assert_eq!(mir_id, self.struct_map[&hir_id]);
            self.structs.hir_ids.insert(mir_id, hir_id);
        }
    }

    /// Fill the MIR struct field types. This runs after the mangling shell
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
                    .map(|field| mir::Field {
                        name: field.name.clone(),
                        ty: types.lower(
                            field.ty,
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
            let arguments = decl
                .type_arguments
                .iter()
                .map(|argument| {
                    types.lower(
                        *argument,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    )
                })
                .collect();
            self.interfaces
                .get_or_create(module, &mut self.shell, hir_id, arguments);
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
                        hir::IntrinsicTypeRepresentation::String => {
                            mir::IntrinsicTypeRepresentation::String
                        }
                        hir::IntrinsicTypeRepresentation::Array { element } => {
                            mir::IntrinsicTypeRepresentation::Array {
                                element: types.lower(
                                    *element,
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
                                    &mut self.enums,
                                    &mut self.structs,
                                    &mut self.interfaces,
                                    &mut self.shell,
                                ),
                            }
                        }
                        hir::IntrinsicTypeRepresentation::Integer(_)
                        | hir::IntrinsicTypeRepresentation::Boolean
                        | hir::IntrinsicTypeRepresentation::Ptr { .. }
                        | hir::IntrinsicTypeRepresentation::FunPtr { .. } => {
                            unreachable!("the registry fixes intrinsic declaration targets")
                        }
                    })
                }
            };
            let mir_id = self.classes.alloc(mir::ClassDef {
                modifier,
                name: decl.name.clone(),
                representation,
                interfaces: Vec::new(),
                vtable: Vec::new(),
                itables: Vec::new(),
            });
            assert_eq!(mir_id, self.class_map[&hir_id]);
        }
    }

    /// Resolve base classes and concrete interface applications after the
    /// mangling shell contains every class name. Interface type arguments may
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

    /// A declared function's symbol: `scoop.<name>` (the fixed
    /// `scoop_main` for the entry point). When the name is shared by
    /// overloads (M7), the parameter encoding is appended so each
    /// overload gets a distinct LLVM symbol: `scoop.show.I`,
    /// `scoop.println.S`, `scoop.Doc.describe.I` for methods (see
    /// `mir::mangle_overload`). vtable / itable slots and thunk calls
    /// reference functions by id, so they pick the final symbol up
    /// from the arena automatically.
    pub(super) fn declare_symbol(
        &mut self,
        module: &hir::Module,
        hir_id: hir::FunctionId,
    ) -> String {
        // Entry linkage keeps its fixed runtime symbol until the program
        // descriptor owns a typed entry pointer (M23 runtime registry).
        if hir_id == module.entry {
            return mir::ENTRY_SYMBOL.to_string();
        }
        // Persistent symbols are complete upstream: overloads differ by
        // signature key and specializations by ODR body member, so no
        // name-based overload or instance encoding remains here.
        module.functions[hir_id].symbol.clone()
    }

    /// Declare one local-concrete user function (body filled later):
    /// `scoop.<name>`, `scoop.<Type>.<name>` for members, or the fixed
    /// entry symbol `scoop_main` that the C runtime calls (`main` is
    /// never instantiated from a generic template, hir-lower guarantees it);
    /// overloads get the
    /// parameter encoding appended (`declare_symbol`).
    pub(super) fn declare_function(
        &mut self,
        module: &hir::Module,
        hir_id: hir::FunctionId,
    ) -> mir::FunctionId {
        let function = &module.functions[hir_id];
        let name = fn_name(function);
        let symbol = self.declare_symbol(module, hir_id);
        let id = self.functions.alloc(mir::Function {
            gc_effect: lower_gc_effect(function.attributes.gc_effect),
            name,
            symbol,
            // Filled in when the body is lowered below.
            params: Vec::new(),
            return_ty: mir::Type::Unit,
            body: mir::Body::unreachable(Arena::new()),
        });
        self.top_level.push(id);
        self.function_map.insert(hir_id, id);
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
