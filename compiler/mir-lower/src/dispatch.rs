//! Dynamic-dispatch construction.
//!
//! This module owns concrete vtable/itable assembly, interface-variance and
//! function-variance bridges, plus boxed value adjust thunks. Every semantic
//! target and slot identity has already been fixed by concrete HIR.

use super::*;

impl Lowerer {
    pub(super) fn finalize_variance_itables(&mut self, module: &hir::Module) -> bool {
        let targets: Vec<mir::InterfaceId> =
            self.interfaces.defs.iter().map(|(id, _)| id).collect();
        let classes: Vec<mir::ClassId> = self.classes.iter().map(|(id, _)| id).collect();
        let mut added = false;
        for class in classes {
            let sources: Vec<(mir::InterfaceId, Vec<mir::TableSlot>)> = self.classes[class]
                .itables
                .iter()
                .map(|record| (record.interface, clone_slots(&record.slots)))
                .collect();
            for &target in &targets {
                if self.classes[class]
                    .itables
                    .iter()
                    .any(|record| record.interface == target)
                {
                    continue;
                }
                let Some((source, slots)) = sources
                    .iter()
                    .find(|(source, _)| self.interface_is_subtype(module, *source, target))
                else {
                    continue;
                };
                let (source_id, _) = self.interfaces.source(*source);
                let method_indices: Vec<_> = module.interfaces[source_id]
                    .methods
                    .iter()
                    .enumerate()
                    .map(|(index, _)| index)
                    .collect();
                let bridge_slots = slots
                    .iter()
                    .enumerate()
                    .map(|(slot_index, slot)| {
                        mir::TableSlot::Function(self.build_variance_bridge(
                            module,
                            class,
                            *source,
                            target,
                            method_indices[slot_index],
                            slot,
                        ))
                    })
                    .collect();
                self.classes[class].interfaces.push(target);
                self.classes[class].itables.push(mir::ItableRecord {
                    interface: target,
                    slots: bridge_slots,
                });
                added = true;
            }
        }
        added
    }

    fn build_variance_bridge(
        &mut self,
        module: &hir::Module,
        class: mir::ClassId,
        source: mir::InterfaceId,
        target: mir::InterfaceId,
        method_index: usize,
        source_slot: &mir::TableSlot,
    ) -> mir::FunctionId {
        let (source_id, _) = self.interfaces.source(source);
        let (target_id, _) = self.interfaces.source(target);
        let source_signature = &module.interfaces[source_id].methods[method_index];
        let target_signature = &module.interfaces[target_id].methods[method_index];
        let source_types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        let target_types = Types { ..source_types };
        let mut source_params = Vec::new();
        let mut target_params = Vec::new();
        for (source_param, target_param) in
            source_signature.params.iter().zip(&target_signature.params)
        {
            source_params.push(source_types.lower(
                source_param.ty,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            ));
            target_params.push(target_types.lower(
                target_param.ty,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            ));
        }
        let source_return = source_types.lower(
            source_signature.return_ty,
            &mut self.enums,
            &mut self.structs,
            &mut self.interfaces,
            &mut self.shell,
        );
        let target_return = target_types.lower(
            target_signature.return_ty,
            &mut self.enums,
            &mut self.structs,
            &mut self.interfaces,
            &mut self.shell,
        );

        let mut locals = Arena::new();
        let this = locals.alloc(mir::Local {
            name: "this".to_string(),
            ty: mir::Type::Any,
            mutable: false,
        });
        let mut params = vec![mir::Param {
            name: "this".to_string(),
            ty: mir::Type::Any,
            local: this,
        }];
        let mut args = vec![smir::Expr::local(this, mir::Type::Any)];
        for ((param, target_ty), source_ty) in target_signature
            .params
            .iter()
            .zip(target_params)
            .zip(source_params)
        {
            let local = locals.alloc(mir::Local {
                name: param.name.clone(),
                ty: target_ty.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name: param.name.clone(),
                ty: target_ty.clone(),
                local,
            });
            args.push(self.adapt_variance_bridge(
                smir::Expr::local(local, target_ty.clone()),
                &target_ty,
                &source_ty,
            ));
        }
        let callee = match source_slot {
            mir::TableSlot::Function(function) => mir::Callee::User(*function),
            mir::TableSlot::Runtime(function) => mir::Callee::Runtime(*function),
        };
        let call = smir::Expr::new(
            source_return.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee,
                },
                args,
                return_ty: source_return.clone(),
            }),
        );
        let kind = if target_return == mir::Type::Unit {
            smir::StatementKind::Expr(call)
        } else {
            let value = self.adapt_variance_bridge(call, &source_return, &target_return);
            smir::StatementKind::Return { value: Some(value) }
        };
        let class_name = &self.classes[class].name;
        let target_name = &self.interfaces.defs[target].name;
        let name = format!(
            "variance.{class_name}.{target_name}.{}.{}",
            target_signature.name, method_index
        );
        let body = cfg::lower(
            smir::Body {
                locals,
                statements: vec![smir::Statement {
                    kind,
                    span: target_signature.span,
                }],
            },
            target_return.clone(),
        );
        let id = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            symbol: format!("scoop.{name}"),
            name,
            params,
            return_ty: target_return.clone(),
            body,
        });
        self.top_level.push(id);
        if target_signature.is_suspend {
            self.suspend_sources.push(SuspendSource {
                function: id,
                source_return: target_return,
                instance: None,
            });
        }
        id
    }

    fn adapt_variance_bridge(
        &mut self,
        value: smir::Expr,
        source: &mir::Type,
        target: &mir::Type,
    ) -> smir::Expr {
        if source == target {
            return value;
        }
        if let (mir::Type::Function(source), mir::Type::Function(target_type)) = (source, target) {
            let adapter = self.ensure_function_adapter(*source, *target_type);
            return smir::Expr::new(
                mir::Type::Function(*target_type),
                smir::ExprKind::ClosureAlloc {
                    class: self.closure_adapters[adapter].class,
                    captures: vec![value],
                },
            );
        }
        if is_reference_mir(source) && is_reference_mir(target) {
            return value;
        }
        if is_boxable(source) && is_reference_mir(target) {
            let boxed = self
                .boxed
                .get_or_create(&mut self.classes, &mut self.shell, source);
            if let mir::Type::Interface(interface) = target {
                if !self.classes[boxed].interfaces.contains(interface) {
                    self.classes[boxed].interfaces.push(*interface);
                }
            }
            return smir::Expr::new(target.clone(), smir::ExprKind::Box(Box::new(value)));
        }
        unreachable!("variance bridge adaptations always follow a subtype conversion")
    }

    fn ensure_function_adapter(
        &mut self,
        source: mir::FunctionTypeId,
        target: mir::FunctionTypeId,
    ) -> mir::ClosureAdapterId {
        if let Some(&adapter) = self.closure_adapter_by_types.get(&(source, target)) {
            return adapter;
        }
        let source_signature = self.shell.function_types[source].clone();
        let target_signature = self.shell.function_types[target].clone();
        let source_name = mir::encode_type(&self.shell, &mir::Type::Function(source));
        let target_name = mir::encode_type(&self.shell, &mir::Type::Function(target));
        let function = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: format!("$adapter.{source_name}.{target_name}"),
            symbol: format!("scoop.$adapter.{source_name}.{target_name}"),
            params: Vec::new(),
            return_ty: target_signature.return_type.clone(),
            body: mir::Body::unreachable(Arena::new()),
        });
        self.top_level.push(function);
        let invoke = self
            .closure_invokes
            .alloc(mir::ClosureInvokeFunction { function });
        let class = self.closure_classes.alloc(mir::ClosureClass {
            name: format!("$Closure$adapter${source_name}${target_name}"),
            function_type: target,
            invoke,
            captures: vec![mir::Field {
                name: "$source".to_string(),
                ty: mir::Type::Function(source),
            }],
            bridges: Vec::new(),
        });
        let adapter = self.closure_adapters.alloc(mir::ClosureAdapter {
            class,
            source,
            target,
        });
        self.closure_adapter_by_types
            .insert((source, target), adapter);

        let span = Span::new(0, 0);
        let mut locals = Arena::new();
        let closure = locals.alloc(mir::Local {
            name: "$closure".to_string(),
            ty: mir::Type::Function(target),
            mutable: false,
        });
        let mut params = vec![mir::Param {
            name: "$closure".to_string(),
            ty: mir::Type::Function(target),
            local: closure,
        }];
        let mut args = vec![smir::Expr::new(
            mir::Type::Function(source),
            smir::ExprKind::ClosureCapture {
                closure: Box::new(smir::Expr::local(closure, mir::Type::Function(target))),
                class,
                index: 0,
            },
        )];
        for (index, (target_ty, source_ty)) in target_signature
            .parameter_types
            .iter()
            .zip(&source_signature.parameter_types)
            .enumerate()
        {
            let local = locals.alloc(mir::Local {
                name: format!("arg{index}"),
                ty: target_ty.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name: format!("arg{index}"),
                ty: target_ty.clone(),
                local,
            });
            args.push(self.adapt_variance_bridge(
                smir::Expr::local(local, target_ty.clone()),
                target_ty,
                source_ty,
            ));
        }
        let call = smir::Expr::new(
            source_signature.return_type.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Closure {
                        function_type: source,
                    },
                    callee: mir::Callee::Closure(source),
                },
                args,
                return_ty: source_signature.return_type.clone(),
            }),
        );
        let statements = if target_signature.return_type == mir::Type::Unit {
            vec![
                smir::Statement {
                    kind: smir::StatementKind::Expr(call),
                    span,
                },
                smir::Statement {
                    kind: smir::StatementKind::Return { value: None },
                    span,
                },
            ]
        } else {
            vec![smir::Statement {
                kind: smir::StatementKind::Return {
                    value: Some(self.adapt_variance_bridge(
                        call,
                        &source_signature.return_type,
                        &target_signature.return_type,
                    )),
                },
                span,
            }]
        };
        self.functions[function].params = params;
        self.functions[function].body = cfg::lower(
            smir::Body { locals, statements },
            target_signature.return_type.clone(),
        );
        if target_signature.is_suspend {
            self.suspend_sources.push(SuspendSource {
                function,
                source_return: target_signature.return_type,
                instance: None,
            });
        }
        adapter
    }

    fn interface_is_subtype(
        &mut self,
        module: &hir::Module,
        source: mir::InterfaceId,
        target: mir::InterfaceId,
    ) -> bool {
        if source == target {
            return true;
        }
        let (source_id, source_args) = self.interfaces.source(source);
        let source_args = source_args.to_vec();
        let (target_id, target_args) = self.interfaces.source(target);
        let target_args = target_args.to_vec();
        if module.interfaces[source_id].family != module.interfaces[target_id].family
            || source_args.len() != target_args.len()
        {
            return false;
        }
        module.interfaces[source_id]
            .variances
            .iter()
            .copied()
            .zip(&source_args)
            .zip(&target_args)
            .all(|((variance, source), target)| match variance {
                hir::Variance::Invariant => source == target,
                hir::Variance::Out => self.mir_type_is_subtype(module, source, target),
                hir::Variance::In => self.mir_type_is_subtype(module, target, source),
            })
    }

    fn mir_type_is_subtype(
        &mut self,
        module: &hir::Module,
        source: &mir::Type,
        target: &mir::Type,
    ) -> bool {
        if source == target || matches!(target, mir::Type::Any) {
            return true;
        }
        match (source, target) {
            (mir::Type::Class(source), mir::Type::Class(target)) => {
                let mut current = Some(*source);
                while let Some(class) = current {
                    if class == *target {
                        return true;
                    }
                    current = self.classes[class].base_class();
                }
                false
            }
            (mir::Type::Interface(source), mir::Type::Interface(target)) => {
                self.interface_is_subtype(module, *source, *target)
            }
            (mir::Type::Class(source), mir::Type::Interface(target)) => {
                let interfaces = self.classes[*source].interfaces.clone();
                interfaces
                    .into_iter()
                    .any(|interface| self.interface_is_subtype(module, interface, *target))
            }
            (mir::Type::Struct(_) | mir::Type::Enum(_, _), mir::Type::Interface(target)) => self
                .value_interfaces(module, source)
                .into_iter()
                .any(|interface| self.interface_is_subtype(module, interface, *target)),
            (mir::Type::Function(source), mir::Type::Function(target)) => {
                let source = self.shell.function_types[*source].clone();
                let target = self.shell.function_types[*target].clone();
                source.is_suspend == target.is_suspend
                    && source.parameter_types.len() == target.parameter_types.len()
                    && target
                        .parameter_types
                        .iter()
                        .zip(&source.parameter_types)
                        .all(|(target, source)| self.mir_type_is_subtype(module, target, source))
                    && self.mir_type_is_subtype(module, &source.return_type, &target.return_type)
            }
            _ => false,
        }
    }

    pub(super) fn finalize_function_bridges(&mut self, module: &hir::Module) -> bool {
        let classes: Vec<_> = self.closure_classes.iter().map(|(id, _)| id).collect();
        let targets = self.function_bridge_targets.clone();
        let mut added = false;
        for class in classes {
            let source = self.closure_classes[class].function_type;
            for &target in &targets {
                if self.finalized_function_bridges.contains(&(class, target))
                    || !self.mir_type_is_subtype(
                        module,
                        &mir::Type::Function(source),
                        &mir::Type::Function(target),
                    )
                {
                    continue;
                }
                let function = if source == target {
                    let invoke = self.closure_classes[class].invoke;
                    self.closure_invokes[invoke].function
                } else {
                    self.build_function_bridge(class, source, target)
                };
                self.closure_classes[class]
                    .bridges
                    .push(mir::FunctionBridge { target, function });
                self.finalized_function_bridges.insert((class, target));
                added = true;
            }
        }
        added
    }

    fn build_function_bridge(
        &mut self,
        class: mir::ClosureClassId,
        source: mir::FunctionTypeId,
        target: mir::FunctionTypeId,
    ) -> mir::FunctionId {
        let source_signature = self.shell.function_types[source].clone();
        let target_signature = self.shell.function_types[target].clone();
        let mut locals = Arena::new();
        let closure = locals.alloc(mir::Local {
            name: "$source".to_string(),
            ty: mir::Type::Function(source),
            mutable: false,
        });
        let mut params = vec![mir::Param {
            name: "$source".to_string(),
            ty: mir::Type::Function(source),
            local: closure,
        }];
        let mut args = vec![smir::Expr::local(closure, mir::Type::Function(source))];
        for (index, (target_ty, source_ty)) in target_signature
            .parameter_types
            .iter()
            .zip(&source_signature.parameter_types)
            .enumerate()
        {
            let local = locals.alloc(mir::Local {
                name: format!("arg{index}"),
                ty: target_ty.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name: format!("arg{index}"),
                ty: target_ty.clone(),
                local,
            });
            args.push(self.adapt_variance_bridge(
                smir::Expr::local(local, target_ty.clone()),
                target_ty,
                source_ty,
            ));
        }
        let call = smir::Expr::new(
            source_signature.return_type.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Closure {
                        function_type: source,
                    },
                    callee: mir::Callee::Closure(source),
                },
                args,
                return_ty: source_signature.return_type.clone(),
            }),
        );
        let statements = if target_signature.return_type == mir::Type::Unit {
            vec![
                smir::Statement {
                    kind: smir::StatementKind::Expr(call),
                    span: Span { start: 0, end: 0 },
                },
                smir::Statement {
                    kind: smir::StatementKind::Return { value: None },
                    span: Span { start: 0, end: 0 },
                },
            ]
        } else {
            vec![smir::Statement {
                kind: smir::StatementKind::Return {
                    value: Some(self.adapt_variance_bridge(
                        call,
                        &source_signature.return_type,
                        &target_signature.return_type,
                    )),
                },
                span: Span { start: 0, end: 0 },
            }]
        };
        let source_name = &self.closure_classes[class].name;
        let target_name = mir::encode_type(&self.shell, &mir::Type::Function(target));
        let name = format!("function_bridge.{source_name}.{target_name}");
        let body = cfg::lower(
            smir::Body { locals, statements },
            target_signature.return_type.clone(),
        );
        let function = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            symbol: format!("scoop.{name}"),
            name,
            params,
            return_ty: target_signature.return_type.clone(),
            body,
        });
        self.top_level.push(function);
        if target_signature.is_suspend {
            self.suspend_sources.push(SuspendSource {
                function,
                source_return: target_signature.return_type,
                instance: None,
            });
        }
        function
    }

    /// Fix every class's vtable and itables (impl spec 2.9): a derived
    /// Build each class's vtable / itables from dispatch identities emitted by
    /// concrete HIR. Vtable slots are inherited base-prefix first; overrides
    /// carry the base virtual family and replace its slot, while overloads
    /// carry distinct families. Interface records and every slot target are
    /// likewise complete upstream data, never reconstructed from signatures.
    pub(super) fn compute_dispatch(&mut self, module: &hir::Module, order: &[hir::ClassId]) {
        for &hir_id in order {
            let mir_id = self.class_map[&hir_id];
            let decl = &module.classes[hir_id];
            let (mut vtable, mut slots) = match decl.base_class() {
                Some((base, _)) => {
                    let base = self.class_map[base];
                    (
                        clone_slots(&self.classes[base].vtable),
                        self.method_slots[&base].clone(),
                    )
                }
                None => (Vec::new(), HashMap::new()),
            };
            for &fn_id in &decl.methods {
                let function = &module.functions[fn_id];
                let Some(method) = function.method else {
                    continue;
                };
                let family = match method.dispatch {
                    hir::MethodDispatch::Virtual(family)
                    | hir::MethodDispatch::FinalOverride(family) => family,
                    hir::MethodDispatch::Direct | hir::MethodDispatch::Interface { .. } => {
                        continue;
                    }
                };
                let mir_fn = self.function_map[&fn_id];
                match slots.get(&family) {
                    Some(&slot) => vtable[slot as usize] = mir::TableSlot::Function(mir_fn),
                    None => {
                        slots.insert(family, vtable.len() as u32);
                        vtable.push(mir::TableSlot::Function(mir_fn));
                    }
                }
            }
            let itables = decl
                .interface_implementations
                .iter()
                .map(|implementation| {
                    let interface = self.interfaces.mir_id(implementation.interface);
                    let method_count = module.interfaces[implementation.interface].methods.len();
                    let mut slots = std::iter::repeat_with(|| None)
                        .take(method_count)
                        .collect::<Vec<_>>();
                    for method in &implementation.methods {
                        let target = match method.target {
                            hir::InterfaceImplementationTarget::Method(function) => function,
                            hir::InterfaceImplementationTarget::Abstract { declaration } => {
                                declaration
                            }
                        };
                        let slot = method.slot.into_raw() as usize;
                        let previous = slots[slot]
                            .replace(mir::TableSlot::Function(self.function_map[&target]));
                        assert!(
                            previous.is_none(),
                            "concrete HIR emits each itable slot once"
                        );
                    }
                    let slots = slots
                        .into_iter()
                        .map(|slot| slot.expect("concrete HIR emits every itable slot"))
                        .collect();
                    mir::ItableRecord { interface, slots }
                })
                .collect();
            let class = &mut self.classes[mir_id];
            class.vtable = vtable;
            class.itables = itables;
            self.method_slots.insert(mir_id, slots);
        }
    }

    /// Generate boxed value types' ordinary interface dispatch. A box has no
    /// universal vtable entries; every interface the value type implements
    /// gets an itable whose slots point at adjust thunks. The thunk's
    /// `this` is the boxed object; it unboxes and tail-calls the real
    /// value method. Concrete HIR supplies the exact implementation function
    /// for each typed interface slot.
    pub(super) fn finalize_boxed(&mut self, module: &hir::Module, index: usize) {
        let class_id = self.boxed.order[index];
        let payload = self.classes[class_id].declared_fields()[0].ty.clone();
        let encoded = mir::encode_type(&self.shell, &payload);
        debug_assert!(self.classes[class_id].vtable.is_empty());
        let interfaces = self.classes[class_id].interfaces.clone();
        for iface in interfaces {
            let (hir_iface, _) = self.interfaces.source(iface);
            let method_indices: Vec<_> = module.interfaces[hir_iface]
                .methods
                .iter()
                .enumerate()
                .map(|(index, _)| index)
                .collect();
            let mut slots = Vec::new();
            for index in method_indices {
                let thunk = self.build_thunk(module, &payload, &encoded, iface, index);
                slots.push(mir::TableSlot::Function(thunk));
            }
            self.classes[class_id].itables.push(mir::ItableRecord {
                interface: iface,
                slots,
            });
        }
    }

    /// The adjust thunk for one (boxed value type, interface method)
    /// pair (impl spec 2.9): `this` is the boxed object; the thunk
    /// unboxes it and tail-calls the real value method (value-type
    /// methods take `this` by value at MIR; the pointer convention
    /// of the receiver is a codegen ABI matter). The implementation
    /// is selected by its typed concrete-HIR conformance entry, so overloads
    /// never require a name/signature search. The thunk symbol carries the
    /// parameter encoding when the interface overloads the name.
    fn build_thunk(
        &mut self,
        module: &hir::Module,
        payload: &mir::Type,
        encoded: &str,
        iface: mir::InterfaceId,
        method_index: usize,
    ) -> mir::FunctionId {
        let (hir_iface, _) = self.interfaces.source(iface);
        let signature = &module.interfaces[hir_iface].methods[method_index];
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        let mut locals = Arena::new();
        let this = locals.alloc(mir::Local {
            name: "this".to_string(),
            ty: mir::Type::Any,
            mutable: false,
        });
        let mut params = vec![mir::Param {
            name: "this".to_string(),
            ty: mir::Type::Any,
            local: this,
        }];
        let mut args = vec![smir::Expr::new(
            payload.clone(),
            smir::ExprKind::Unbox(Box::new(smir::Expr::local(this, mir::Type::Any))),
        )];
        let mut target_params = Vec::new();
        let mut argument_locals = Vec::new();
        for param in &signature.params {
            let ty = types.lower(
                param.ty,
                &mut self.enums,
                &mut self.structs,
                &mut self.interfaces,
                &mut self.shell,
            );
            target_params.push(ty.clone());
            let local = locals.alloc(mir::Local {
                name: param.name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name: param.name.clone(),
                ty,
                local,
            });
            argument_locals.push(local);
        }
        let return_ty = types.lower(
            signature.return_ty,
            &mut self.enums,
            &mut self.structs,
            &mut self.interfaces,
            &mut self.shell,
        );
        let implementations = self
            .value_interface_implementations(module, payload)
            .to_vec();
        let source_implementation = implementations
            .into_iter()
            .find(|implementation| {
                let source = self.interfaces.mir_id(implementation.interface);
                self.interface_is_subtype(module, source, iface)
            })
            .expect("concrete HIR supplies the boxed value's target conformance");
        let implementation = source_implementation
            .methods
            .into_iter()
            .find(|implementation| implementation.slot.into_raw() as usize == method_index)
            .expect("concrete HIR supplies every boxed itable slot");
        let implementation = match implementation.target {
            hir::InterfaceImplementationTarget::Method(function) => function,
            hir::InterfaceImplementationTarget::Abstract { .. } => {
                unreachable!("value-type interface implementations are always concrete")
            }
        };
        let implementation_function = &module.functions[implementation];
        let source_types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        let source_params = implementation_function
            .params
            .iter()
            .skip(1)
            .map(|param| {
                source_types.lower(
                    param.ty,
                    &mut self.enums,
                    &mut self.structs,
                    &mut self.interfaces,
                    &mut self.shell,
                )
            })
            .collect::<Vec<_>>();
        let implementation_return = source_types.lower(
            implementation_function.return_ty,
            &mut self.enums,
            &mut self.structs,
            &mut self.interfaces,
            &mut self.shell,
        );
        for ((local, target_ty), source_ty) in argument_locals
            .into_iter()
            .zip(&target_params)
            .zip(&source_params)
        {
            args.push(self.adapt_variance_bridge(
                smir::Expr::local(local, target_ty.clone()),
                target_ty,
                source_ty,
            ));
        }
        let call = smir::Expr::new(
            implementation_return.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee: mir::Callee::User(self.function_map[&implementation]),
                },
                args,
                return_ty: implementation_return.clone(),
            }),
        );
        let kind = if return_ty == mir::Type::Unit {
            smir::StatementKind::Expr(call)
        } else {
            smir::StatementKind::Return {
                value: Some(self.adapt_variance_bridge(call, &implementation_return, &return_ty)),
            }
        };
        let encoding = mir::encode_params(&self.shell, &target_params);
        let iface_name = self.interfaces.defs[iface].name.clone();
        // An interface overloading the method name needs the parameter
        // encoding to keep the thunk symbols distinct.
        let overloaded = module.interfaces[hir_iface]
            .methods
            .iter()
            .filter(|sig| sig.name == signature.name)
            .count()
            > 1;
        let name = if overloaded {
            format!("thunk.{encoded}.{iface_name}.{}.{encoding}", signature.name)
        } else {
            format!("thunk.{encoded}.{iface_name}.{}", signature.name)
        };
        let body = cfg::lower(
            smir::Body {
                locals,
                statements: vec![smir::Statement {
                    kind,
                    span: signature.span,
                }],
            },
            return_ty.clone(),
        );
        let id = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            symbol: format!("scoop.{name}"),
            name,
            params,
            return_ty: return_ty.clone(),
            body,
        });
        self.top_level.push(id);
        if signature.is_suspend {
            self.suspend_sources.push(SuspendSource {
                function: id,
                source_return: return_ty,
                instance: None,
            });
        }
        id
    }

    fn value_interfaces(
        &mut self,
        module: &hir::Module,
        payload: &mir::Type,
    ) -> Vec<mir::InterfaceId> {
        let declared = match self.value_struct_source(module, payload) {
            Some(hir_id) => module.structs[hir_id].interfaces.clone(),
            None => match payload {
                mir::Type::Enum(mir_id, _) => {
                    let hir_id = self.enums.hir_ids[mir_id];
                    module.enums[hir_id].interfaces.clone()
                }
                _ => return Vec::new(),
            },
        };
        let types = Types {
            module,
            struct_map: &self.struct_map,
            class_map: &self.class_map,
        };
        declared
            .into_iter()
            .map(|ty| {
                let lowered = types.lower(
                    ty,
                    &mut self.enums,
                    &mut self.structs,
                    &mut self.interfaces,
                    &mut self.shell,
                );
                let mir::Type::Interface(interface) = lowered else {
                    unreachable!()
                };
                interface
            })
            .collect()
    }

    fn value_interface_implementations<'a>(
        &self,
        module: &'a hir::Module,
        payload: &mir::Type,
    ) -> &'a [hir::InterfaceImplementation] {
        match self.value_struct_source(module, payload) {
            Some(source) => &module.structs[source].interface_implementations,
            None => match payload {
                mir::Type::Enum(id, _) => {
                    &module.enums[self.enums.hir_ids[id]].interface_implementations
                }
                _ => unreachable!("only value types receive boxed interface adapters"),
            },
        }
    }

    /// Exact source declaration for a MIR struct-like payload. Primitive
    /// representations use the typed relation emitted by HIR; ordinary
    /// struct instances use the mandatory MIR→HIR provenance map.
    fn value_struct_source(
        &self,
        module: &hir::Module,
        payload: &mir::Type,
    ) -> Option<hir::StructId> {
        match payload {
            mir::Type::Int => Some(module.intrinsic_type_core.int),
            mir::Type::UInt => Some(module.intrinsic_type_core.uint),
            mir::Type::Boolean => Some(module.intrinsic_type_core.boolean),
            mir::Type::Struct(mir_id) => Some(self.structs.hir_ids[mir_id]),
            _ => None,
        }
    }
}
