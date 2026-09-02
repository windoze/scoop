use super::*;

impl Lowerer {
    /// Declare an interface method as a signature-only shell that is never
    /// emitted. Virtual interface calls name it so LIR receives the complete
    /// indirect-call parameter and return types.
    pub(super) fn declare_interface_method(
        &mut self,
        module: &hir::Module,
        hir_id: hir::FunctionId,
    ) -> mir::FunctionId {
        let function = &module.functions[hir_id];
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        let mut locals = Arena::new();
        let params = function
            .params
            .iter()
            .map(|param| {
                let ty = types.lower(
                    param.ty,
                    &mut self.enums,
                    &mut self.structs,
                    &mut self.interfaces,
                    &mut self.shell,
                );
                let local = locals.alloc(mir::Local {
                    name: param.name.clone(),
                    ty: ty.clone(),
                    mutable: false,
                });
                mir::Param {
                    name: param.name.clone(),
                    ty,
                    local,
                }
            })
            .collect();
        let return_ty = types.lower(
            function.return_ty,
            &mut self.enums,
            &mut self.structs,
            &mut self.interfaces,
            &mut self.shell,
        );
        let name = fn_name(function);
        let symbol = self.declare_symbol(module, hir_id);
        let id = self.functions.alloc(mir::Function {
            gc_effect: lower_gc_effect(function.attributes.gc_effect),
            symbol,
            name,
            params,
            return_ty: return_ty.clone(),
            body: mir::Body::unreachable(locals),
        });
        self.function_map.insert(hir_id, id);
        self.record_function_instance(module, hir_id, id);
        id
    }

    /// Materialize an interface slot that has no callable use in this cone.
    /// Local-concrete HIR carries its complete signature on the interface
    /// definition, so MIR can still give every slot a typed function entity
    /// without waiting for a call site or reconstructing it from a name.
    pub(super) fn declare_interface_signature(
        &mut self,
        module: &hir::Module,
        interface: hir::InterfaceId,
        slot: usize,
    ) -> mir::FunctionId {
        let declaration = &module.interfaces[interface];
        let method = &declaration.methods[slot];
        let owner = mir::Type::Interface(self.interfaces.mir_id(interface));
        let mut locals = Arena::new();
        let this = locals.alloc(mir::Local {
            name: "this".to_string(),
            ty: owner.clone(),
            mutable: false,
        });
        let mut params = vec![mir::Param {
            name: "this".to_string(),
            ty: owner,
            local: this,
        }];
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        params.extend(method.params.iter().map(|param| {
            let ty = types.lower(
                param.ty,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
            let local = locals.alloc(mir::Local {
                name: param.name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            mir::Param {
                name: param.name.clone(),
                ty,
                local,
            }
        }));
        let return_ty = types.lower(
            method.return_ty,
            &mut self.enums,
            &mut self.structs,
            &mut self.interfaces,
            &mut self.shell,
        );
        let name = format!("{}.{}", declaration.name, method.name);
        let symbol = format!(
            "scoop.$interface_signature.{}.{}",
            interface.into_raw().into_u32(),
            slot
        );
        self.functions.alloc(mir::Function {
            gc_effect: lower_gc_effect(method.attributes.gc_effect),
            name,
            symbol,
            params,
            return_ty,
            body: mir::Body::unreachable(locals),
        })
    }

    /// Fill the MIR class fields: the base class's (already
    /// flattened) fields come first — the object layout and HIR's
    /// `ClassField` indices follow the same order — then the
    /// constructor properties in declaration order.
    pub(super) fn fill_class_fields(&mut self, module: &hir::Module, order: &[hir::ClassId]) {
        for &hir_id in order {
            let decl = &module.classes[hir_id];
            let mir_id = self.class_map[&hir_id];
            if matches!(
                decl.representation,
                hir::ClassRepresentation::Intrinsic { .. }
            ) {
                debug_assert!(decl.base_class().is_none());
                continue;
            }
            let mut fields = match decl.base_class() {
                Some((base, _)) => {
                    clone_fields(self.classes[self.class_map[base]].declared_fields())
                }
                None => Vec::new(),
            };
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            for field in decl.declared_constructor() {
                fields.push(mir::Field {
                    name: field.name.clone(),
                    ty: types.lower(
                        field.ty,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    ),
                });
            }
            match &mut self.classes[mir_id].representation {
                mir::ClassRepresentation::Declared {
                    fields: mir_fields, ..
                } => *mir_fields = fields,
                mir::ClassRepresentation::Intrinsic(_) => debug_assert!(fields.is_empty()),
            }
        }
    }

    /// Declare the constructor function of one class (`scoop.ctor.
    /// <Class>`): parameters are the constructor properties in
    /// declaration order; the body is filled by `lower_ctor`.
    pub(super) fn declare_ctor(
        &mut self,
        module: &hir::Module,
        constructor_id: hir::ClassConstructorId,
    ) -> mir::FunctionId {
        let constructor = &module.class_constructors[constructor_id];
        let decl = &module.classes[constructor.class];
        let name = format!("ctor.{}", decl.name);
        let id = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            symbol: format!("scoop.{name}"),
            name,
            params: Vec::new(),
            return_ty: mir::Type::Unit,
            body: mir::Body::unreachable(Arena::new()),
        });
        self.top_level.push(id);
        self.ctors.insert(constructor_id, id);
        id
    }

    /// Lower the constructor function's body: a single raw
    /// `smir::ExprKind::ClassInit` over the flattened field values — the
    /// base delegation arguments (evaluated here in the ctor context;
    /// hir-lower M6 lowers them in an empty scope, so they are closed
    /// expressions) expanded recursively down the base chain, then
    /// the class's own constructor properties. Base ctors are never
    /// called: the flattened fields are written in one shot, so the
    /// object identity is a single allocation.
    pub(super) fn lower_ctor(
        &mut self,
        module: &hir::Module,
        constructor_id: hir::ClassConstructorId,
    ) -> (Vec<mir::Param>, mir::Type, smir::Body) {
        let constructor = module.class_constructors[constructor_id].clone();
        let hir_id = constructor.class;
        let decl = &module.classes[hir_id];
        let mir_id = self.class_map[&hir_id];
        let field_count = self.classes[mir_id].declared_fields().len();
        let mut lowerer = BodyLowerer {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
            interfaces: &mut self.interfaces,
            structs: &mut self.structs,
            method_slots: &self.method_slots,
            function_map: &self.function_map,
            extern_map: &self.extern_map,
            global_map: &self.global_map,
            callback_bridges: &mut self.callback_bridges,
            callback_by_target: &mut self.callback_by_target,
            foreign_callback_adapters: &mut self.foreign_callback_adapters,
            foreign_callback_bridges: &mut self.foreign_callback_bridges,
            foreign_callback_by_registration: &mut self.foreign_callback_by_registration,
            ctors: &self.ctors,
            strings: &mut self.strings,
            functions: &mut self.functions,
            top_level: &mut self.top_level,
            instances: &mut self.instances,
            enums: &mut self.enums,
            boxed: &mut self.boxed,
            classes: &mut self.classes,
            shell: &mut self.shell,
            // Delegation arguments are closed (hir-lower M6 lowers
            // them in an empty scope), so no locals are visible.
            local_map: HashMap::new(),
            constructor_param_map: HashMap::new(),
            locals: Arena::new(),
            hidden_count: 0,
            prelude: Vec::new(),
            option_variants: self.option_variants,
            coroutines: &mut self.coroutines,
            lambda_closures: &self.lambda_closures,
            anonymous_closures: &self.anonymous_closures,
            reference_closures: &self.reference_closures,
            closure_classes: &mut self.closure_classes,
            closure_invokes: &mut self.closure_invokes,
            closure_capture_indices: &mut self.closure_capture_indices,
            closure_adapters: &mut self.closure_adapters,
            closure_adapter_by_types: &mut self.closure_adapter_by_types,
            dynamic_closure_adapters: &mut self.dynamic_closure_adapters,
            dynamic_adapter_by_target: &mut self.dynamic_adapter_by_target,
            function_bridge_targets: &mut self.function_bridge_targets,
            suspend_sources: &mut self.suspend_sources,
            current_closure: None,
            current_closure_local: None,
            current_local_capture_params: HashMap::new(),
        };
        assert_eq!(
            decl.declared_constructor().len(),
            constructor.params.len(),
            "the typed constructor signature covers every source parameter"
        );
        let mut params = Vec::new();
        let mut own = Vec::new();
        for (field, parameter_type) in decl
            .declared_constructor()
            .iter()
            .zip(constructor.params.iter().copied())
        {
            let ty = lowerer.lower_type(parameter_type);
            let local = lowerer.locals.alloc(mir::Local {
                name: field.name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name: field.name.clone(),
                ty: ty.clone(),
                local,
            });
            lowerer.constructor_param_map.insert(field.parameter, local);
            own.push(smir::Expr::local(local, ty));
        }
        let args = flattened_ctor_args(&mut lowerer, module, hir_id, own);
        assert_eq!(
            args.len(),
            field_count,
            "the flattened initializer covers every field"
        );
        let return_ty = lowerer.lower_type(constructor.return_type);
        assert_eq!(
            return_ty,
            mir::Type::Class(mir_id),
            "the hidden constructor returns its owning concrete class"
        );
        let body = smir::Body {
            locals: lowerer.locals,
            statements: vec![smir::Statement {
                kind: smir::StatementKind::Return {
                    value: Some(smir::Expr::new(
                        return_ty.clone(),
                        smir::ExprKind::ClassInit {
                            class_id: mir_id,
                            args,
                        },
                    )),
                },
                span: decl.span,
            }],
        };
        (params, return_ty, body)
    }
}
