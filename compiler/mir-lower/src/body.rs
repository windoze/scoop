use super::*;

/// Per-function-body lowering state.
pub(super) struct BodyLowerer<'a> {
    pub(super) module: &'a hir::Module,
    pub(super) struct_map: &'a HashMap<hir::StructId, mir::StructId>,
    pub(super) class_map: &'a HashMap<hir::ClassId, mir::ClassId>,
    pub(super) interfaces: &'a mut InterfaceRegistry,
    /// MIR struct definitions used for representation and GC
    /// classification of compiler-synthesized aggregates.
    pub(super) structs: &'a mut StructRegistry,
    /// Method signature key -> vtable slot per class
    /// (`compute_dispatch`).
    pub(super) method_slots: &'a HashMap<mir::ClassId, HashMap<String, u32>>,
    pub(super) function_map: &'a HashMap<hir::FunctionId, mir::FunctionId>,
    pub(super) extern_map: &'a HashMap<hir::ExternFunctionId, mir::ExternFunctionId>,
    pub(super) global_map: &'a HashMap<hir::GlobalId, mir::GlobalId>,
    pub(super) callback_bridges: &'a mut Arena<mir::CallbackBridge>,
    pub(super) callback_by_target:
        &'a mut HashMap<(mir::FunctionId, mir::FunctionTypeId), mir::CallbackBridgeId>,
    pub(super) foreign_callback_adapters: &'a mut Arena<mir::ForeignCallbackAdapter>,
    pub(super) foreign_callback_bridges: &'a mut Arena<mir::ForeignCallbackBridge>,
    pub(super) foreign_callback_by_registration:
        &'a mut HashMap<hir::ForeignCallbackRegistrationId, mir::ForeignCallbackBridgeId>,
    /// HIR class -> its constructor function (`ClassInit` calls).
    pub(super) ctors: &'a HashMap<hir::ClassId, mir::FunctionId>,
    pub(super) strings: &'a mut Arena<mir::StringConst>,
    pub(super) functions: &'a mut Arena<mir::Function>,
    pub(super) top_level: &'a mut Vec<mir::FunctionId>,
    pub(super) instances: &'a mut InstanceRegistry,
    /// Instantiated enum definitions, filled on creation; variant
    /// field types feed pattern lowering and representation.
    pub(super) enums: &'a mut EnumRegistry,
    /// Boxed value types discovered in this body (`Box` / `is` / `as`).
    pub(super) boxed: &'a mut BoxedRegistry,
    /// MIR class arena (boxed value types are appended here).
    pub(super) classes: &'a mut Arena<mir::ClassDef>,
    /// Mangling shell (enum / struct names for `encode_type`).
    pub(super) shell: &'a mut mir::Module,
    /// HIR local -> MIR local (same declaration order per body).
    pub(super) local_map: HashMap<hir::LocalId, mir::LocalId>,
    /// Constructor-parameter identities available while lowering one
    /// generated class constructor's delegation expressions.
    pub(super) constructor_param_map: HashMap<hir::ConstructorParamId, mir::LocalId>,
    /// MIR locals, including the hidden ones created during lowering
    /// (`when` subjects, destructuring slots, `!!` temporaries).
    pub(super) locals: Arena<mir::Local>,
    pub(super) hidden_count: usize,
    /// Statement kinds that must precede the statement currently being
    /// lowered (the trap test of `!!`); drained by the caller.
    pub(super) prelude: Vec<smir::StatementKind>,
    /// Declaration indices of `Option::Some` / `Option::None`.
    pub(super) option_variants: (u32, u32),
    pub(super) coroutines: &'a mut CoroutineRegistry,
    pub(super) lambda_closures: &'a HashMap<hir::LambdaId, mir::ClosureClassId>,
    pub(super) anonymous_closures: &'a HashMap<hir::AnonymousFunctionId, mir::ClosureClassId>,
    pub(super) reference_closures: &'a HashMap<hir::CallableReferenceId, mir::ClosureClassId>,
    pub(super) closure_classes: &'a mut Arena<mir::ClosureClass>,
    pub(super) closure_invokes: &'a mut Arena<mir::ClosureInvokeFunction>,
    pub(super) closure_capture_indices: &'a mut HashMap<(mir::ClosureClassId, hir::BindingId), u32>,
    pub(super) closure_adapters: &'a mut Arena<mir::ClosureAdapter>,
    pub(super) closure_adapter_by_types:
        &'a mut HashMap<(mir::FunctionTypeId, mir::FunctionTypeId), mir::ClosureAdapterId>,
    pub(super) dynamic_closure_adapters: &'a mut Arena<mir::DynamicClosureAdapter>,
    pub(super) dynamic_adapter_by_target:
        &'a mut HashMap<mir::FunctionTypeId, mir::DynamicClosureAdapterId>,
    pub(super) function_bridge_targets: &'a mut Vec<mir::FunctionTypeId>,
    pub(super) suspend_sources: &'a mut Vec<SuspendSource>,
    pub(super) current_closure: Option<mir::ClosureClassId>,
    pub(super) current_closure_local: Option<mir::LocalId>,
    /// Hidden by-value parameters of a lifted local function, keyed by the
    /// global lexical binding they carry.
    pub(super) current_local_capture_params: HashMap<hir::BindingId, hir::LocalId>,
}

/// A step from a pattern subject down to a nested field.
#[derive(Clone, Copy)]
enum Access {
    Field(u32),
    EnumField { variant: u32, index: u32 },
}

impl BodyLowerer<'_> {
    pub(super) fn lower_function(
        mut self,
        function: &hir::Function,
        body: &hir::Body,
    ) -> (Vec<mir::Param>, mir::Type, smir::Body) {
        for (hir_id, local) in body.locals.iter() {
            let ty = self.lower_type(local.ty);
            let mir_id = self.locals.alloc(mir::Local {
                name: local.name.clone(),
                ty,
                mutable: local.mutable,
            });
            self.local_map.insert(hir_id, mir_id);
        }
        let params = function
            .params
            .iter()
            .map(|param| mir::Param {
                name: param.name.clone(),
                ty: self.lower_type(param.ty),
                local: self.local_map[&param.local],
            })
            .collect();
        if self.current_closure.is_some() {
            self.current_closure_local = function
                .params
                .first()
                .map(|param| self.local_map[&param.local]);
        }
        let return_ty = self.lower_type(function.return_ty);
        let statements = if is_abstract_bodiless(function) {
            // An abstract method (hir-lower materializes it bodiless):
            // every override replaces its vtable slot and the class
            // cannot be instantiated, so the slot is never reached;
            // the emitted function traps like a pure-virtual stub.
            let message =
                self.trap_message(format!("call to abstract method `{}`", fn_name(function)));
            vec![smir::Statement {
                kind: smir::StatementKind::Expr(smir::Expr::new(
                    mir::Type::Unit,
                    smir::ExprKind::Call(smir::Call {
                        target: mir::CallTarget {
                            kind: mir::CallKind::Direct,
                            callee: mir::Callee::Runtime(mir::RuntimeFn::Trap),
                        },
                        args: vec![smir::Expr::new(
                            mir::Type::String,
                            smir::ExprKind::StringConst(message),
                        )],
                        return_ty: mir::Type::Unit,
                    }),
                )),
                span: function.span,
            }]
        } else {
            self.lower_statements(&body.statements)
        };
        (
            params,
            return_ty,
            smir::Body {
                locals: self.locals,
                statements,
            },
        )
    }

    pub(super) fn lower_type(&mut self, ty: hir::TypeId) -> mir::Type {
        Types {
            module: self.module,
            struct_map: self.struct_map,
            class_map: self.class_map,
        }
        .lower(ty, self.enums, self.structs, self.interfaces, self.shell)
    }

    pub(super) fn lower_function_type_id(
        &mut self,
        function_type: hir::FunctionTypeId,
    ) -> mir::FunctionTypeId {
        let ty = self.module.function_types[function_type].canonical_type;
        let mir::Type::Function(function_type) = self.lower_type(ty) else {
            unreachable!("lowering a function type preserves its category")
        };
        function_type
    }

    fn ensure_lambda_closure(&mut self, id: hir::LambdaId) -> mir::ClosureClassId {
        self.lambda_closures[&id]
    }

    fn ensure_anonymous_closure(&mut self, id: hir::AnonymousFunctionId) -> mir::ClosureClassId {
        self.anonymous_closures[&id]
    }

    fn ensure_reference_closure(&mut self, id: hir::CallableReferenceId) -> mir::ClosureClassId {
        self.reference_closures[&id]
    }

    fn adapt_function_value(
        &mut self,
        value: smir::Expr,
        source: mir::FunctionTypeId,
        target: mir::FunctionTypeId,
        span: Span,
    ) -> smir::Expr {
        if source == target {
            return value;
        }
        let adapter = self.ensure_function_adapter(source, target, span);
        smir::Expr::new(
            mir::Type::Function(target),
            smir::ExprKind::ClosureAlloc {
                class: self.closure_adapters[adapter].class,
                captures: vec![value],
            },
        )
    }

    fn adapt_mir_subtype(
        &mut self,
        value: smir::Expr,
        source: &mir::Type,
        target: &mir::Type,
        span: Span,
    ) -> smir::Expr {
        if source == target {
            return value;
        }
        if let (mir::Type::Function(source), mir::Type::Function(target)) = (source, target) {
            return self.adapt_function_value(value, *source, *target, span);
        }
        if is_boxable(source) && is_reference_mir(target) {
            self.register_boxed(source, None);
            if let mir::Type::Interface(interface) = target {
                let boxed = self.boxed.get_or_create(self.classes, self.shell, source);
                if !self.classes[boxed].interfaces.contains(interface) {
                    self.classes[boxed].interfaces.push(*interface);
                }
            }
            return smir::Expr::new(target.clone(), smir::ExprKind::Box(Box::new(value)));
        }
        if is_reference_mir(source) && is_reference_mir(target) {
            return smir::Expr::new(
                target.clone(),
                smir::ExprKind::Retype {
                    operand: Box::new(value),
                    ty: Box::new(target.clone()),
                },
            );
        }
        unreachable!("function adapter conversions follow the HIR subtype relation")
    }

    fn ensure_function_adapter(
        &mut self,
        source: mir::FunctionTypeId,
        target: mir::FunctionTypeId,
        span: Span,
    ) -> mir::ClosureAdapterId {
        if let Some(&adapter) = self.closure_adapter_by_types.get(&(source, target)) {
            return adapter;
        }
        let source_signature = self.shell.function_types[source].clone();
        let target_signature = self.shell.function_types[target].clone();
        assert_eq!(
            source_signature.is_suspend, target_signature.is_suspend,
            "ordinary and suspend function types never coerce"
        );
        assert_eq!(
            source_signature.parameter_types.len(),
            target_signature.parameter_types.len(),
            "function variance preserves arity"
        );
        let source_name = mir::encode_type(self.shell, &mir::Type::Function(source));
        let target_name = mir::encode_type(self.shell, &mir::Type::Function(target));
        let name = format!("$Closure$adapter${source_name}${target_name}");

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
            name,
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
            args.push(self.adapt_mir_subtype(
                smir::Expr::local(local, target_ty.clone()),
                target_ty,
                source_ty,
                span,
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
        let mut statements = if target_signature.return_type == mir::Type::Unit {
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
                    value: Some(self.adapt_mir_subtype(
                        call,
                        &source_signature.return_type,
                        &target_signature.return_type,
                        span,
                    )),
                },
                span,
            }]
        };
        self.functions[function].params = params;
        self.functions[function].body = cfg::lower(
            smir::Body {
                locals,
                statements: std::mem::take(&mut statements),
            },
            target_signature.return_type,
        );
        if target_signature.is_suspend {
            self.suspend_sources.push(SuspendSource {
                function,
                source_return: self.shell.function_types[target].return_type.clone(),
                instance: None,
            });
        }
        adapter
    }

    fn adapt_checked_function_value(
        &mut self,
        value: smir::Expr,
        target: mir::FunctionTypeId,
        span: Span,
    ) -> smir::Expr {
        let adapter = self.ensure_dynamic_function_adapter(target, span);
        smir::Expr::new(
            mir::Type::Function(target),
            smir::ExprKind::ClosureAlloc {
                class: self.dynamic_closure_adapters[adapter].class,
                captures: vec![value],
            },
        )
    }

    fn ensure_dynamic_function_adapter(
        &mut self,
        target: mir::FunctionTypeId,
        span: Span,
    ) -> mir::DynamicClosureAdapterId {
        if let Some(&adapter) = self.dynamic_adapter_by_target.get(&target) {
            return adapter;
        }
        if !self.function_bridge_targets.contains(&target) {
            self.function_bridge_targets.push(target);
        }
        let signature = self.shell.function_types[target].clone();
        let encoded = mir::encode_type(self.shell, &mir::Type::Function(target));
        let function = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: format!("$dynamic_adapter.{encoded}"),
            symbol: format!("scoop.$dynamic_adapter.{encoded}"),
            params: Vec::new(),
            return_ty: signature.return_type.clone(),
            body: mir::Body::unreachable(Arena::new()),
        });
        self.top_level.push(function);
        let invoke = self
            .closure_invokes
            .alloc(mir::ClosureInvokeFunction { function });
        let class = self.closure_classes.alloc(mir::ClosureClass {
            name: format!("$Closure$dynamic_adapter${encoded}"),
            function_type: target,
            invoke,
            captures: vec![mir::Field {
                name: "$source".to_string(),
                ty: mir::Type::Any,
            }],
            bridges: Vec::new(),
        });
        let adapter = self
            .dynamic_closure_adapters
            .alloc(mir::DynamicClosureAdapter { class, target });
        self.dynamic_adapter_by_target.insert(target, adapter);

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
            mir::Type::Any,
            smir::ExprKind::ClosureCapture {
                closure: Box::new(smir::Expr::local(closure, mir::Type::Function(target))),
                class,
                index: 0,
            },
        )];
        for (index, ty) in signature.parameter_types.iter().cloned().enumerate() {
            let local = locals.alloc(mir::Local {
                name: format!("arg{index}"),
                ty: ty.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name: format!("arg{index}"),
                ty: ty.clone(),
                local,
            });
            args.push(smir::Expr::local(local, ty));
        }
        let call = smir::Expr::new(
            signature.return_type.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::FunctionBridge {
                        function_type: target,
                    },
                    callee: mir::Callee::FunctionBridge(target),
                },
                args,
                return_ty: signature.return_type.clone(),
            }),
        );
        let statements = if signature.return_type == mir::Type::Unit {
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
                kind: smir::StatementKind::Return { value: Some(call) },
                span,
            }]
        };
        self.functions[function].params = params;
        self.functions[function].body = cfg::lower(
            smir::Body { locals, statements },
            signature.return_type.clone(),
        );
        if signature.is_suspend {
            self.suspend_sources.push(SuspendSource {
                function,
                source_return: signature.return_type,
                instance: None,
            });
        }
        adapter
    }

    /// A fresh hidden local (`$<prefix>.<n>`), compiler-generated.
    fn new_hidden(&mut self, prefix: &str, ty: mir::Type, mutable: bool) -> mir::LocalId {
        self.hidden_count += 1;
        self.locals.alloc(mir::Local {
            name: format!("${prefix}.{}", self.hidden_count),
            ty,
            mutable,
        })
    }

    /// Emit the queued prelude statements (the trap tests of `!!`)
    /// before the statement they belong to.
    fn drain_prelude(&mut self, span: Span, out: &mut Vec<smir::Statement>) {
        out.extend(
            self.prelude
                .drain(..)
                .map(|kind| smir::Statement { kind, span }),
        );
    }

    fn lower_statements(&mut self, statements: &[hir::Statement]) -> Vec<smir::Statement> {
        let mut out = Vec::new();
        for statement in statements {
            self.lower_statement(statement, &mut out);
        }
        out
    }

    fn lower_statement(&mut self, statement: &hir::Statement, out: &mut Vec<smir::Statement>) {
        let span = statement.span;
        let kind = match &statement.kind {
            hir::StatementKind::LocalFunction(_) => return,
            hir::StatementKind::Expr(expr) => {
                let expr = self.lower_expr(expr);
                self.drain_prelude(span, out);
                smir::StatementKind::Expr(expr)
            }
            hir::StatementKind::Return { value } => {
                let value = value.as_ref().map(|value| self.lower_expr(value));
                self.drain_prelude(span, out);
                smir::StatementKind::Return { value }
            }
            hir::StatementKind::ValDecl { pattern, init } => {
                self.lower_val_decl(pattern, init, span, out);
                return;
            }
            hir::StatementKind::Assign { target, value } => {
                let kind = match target {
                    hir::AssignTarget::Local(local) => {
                        let local = self.local_map[local];
                        let value = self.lower_expr(value);
                        smir::StatementKind::Assign { local, value }
                    }
                    hir::AssignTarget::Global(global) => smir::StatementKind::GlobalAssign {
                        global: self.global_map[global],
                        value: self.lower_expr(value),
                    },
                    // `m[i] = v` (only `MutableArray`, checked at HIR).
                    // M8: the bounds check moved here from codegen —
                    // the array and the index are evaluated once into
                    // hidden locals and checked before the store; the
                    // value expression stays inside the `ArraySet`
                    // node and is evaluated after the check.
                    hir::AssignTarget::Index { array, index } => {
                        let array_ty = self.lower_type(array.ty);
                        let mir::Type::Class(array_type) = array_ty else {
                            unreachable!("an array assignment has an intrinsic class type")
                        };
                        let array_slot =
                            self.new_hidden("arr", mir::Type::Class(array_type), false);
                        let index_slot = self.new_hidden("idx", mir::Type::Int, false);
                        let array_value = self.lower_expr(array);
                        self.prelude.push(smir::StatementKind::ValDecl {
                            local: array_slot,
                            init: array_value,
                        });
                        let index_value = self.lower_expr(index);
                        self.prelude.push(smir::StatementKind::ValDecl {
                            local: index_slot,
                            init: index_value,
                        });
                        self.bounds_check(array_type, array_slot, index_slot, span);
                        let value = self.lower_expr(value);
                        smir::StatementKind::ArraySet {
                            array_type,
                            array: smir::Expr::local(array_slot, mir::Type::Class(array_type)),
                            index: smir::Expr::local(index_slot, mir::Type::Int),
                            value,
                        }
                    }
                    // `obj.field = v` (only `var` properties of
                    // classes, checked at HIR); the index is the
                    // flattened field index.
                    hir::AssignTarget::Field { receiver, field } => {
                        let hir::FieldRef::ClassField { index, .. } = field else {
                            unreachable!("hir-lower only allows assignment to class properties")
                        };
                        let object = self.lower_expr(receiver);
                        let value = self.lower_expr(value);
                        smir::StatementKind::FieldSet {
                            object,
                            index: *index,
                            value,
                        }
                    }
                };
                self.drain_prelude(span, out);
                kind
            }
            hir::StatementKind::If {
                cond,
                then_body,
                else_body,
            } => {
                let cond = self.lower_expr(cond);
                self.drain_prelude(span, out);
                let then_body = self.lower_statements(then_body);
                let else_body = else_body.as_ref().map(|body| self.lower_statements(body));
                smir::StatementKind::If {
                    cond,
                    then_body,
                    else_body,
                }
            }
            hir::StatementKind::While { cond, body } => {
                self.lower_while(cond, body, span, out);
                return;
            }
            hir::StatementKind::When(when) => {
                self.lower_when(when, span, out);
                return;
            }
            // `try` / `catch` / `finally` stays structured in MIR
            // (M8, DESIGN 3.3); the control-flow expansion (invoke /
            // landingpad) is LIR's job.
            hir::StatementKind::Try(try_) => smir::StatementKind::Try(self.lower_try(try_)),
            hir::StatementKind::Throw(expr) => {
                let value = self.lower_expr(expr);
                self.drain_prelude(span, out);
                smir::StatementKind::Throw(value)
            }
        };
        out.push(smir::Statement { kind, span });
    }

    /// `try` translates one-to-one: body, ordered catches (the catch
    /// type is resolved to the concrete MIR type), and the optional
    /// finally body.
    fn lower_try(&mut self, try_: &hir::Try) -> smir::Try {
        let body = self.lower_statements(&try_.body);
        let catches = try_
            .catches
            .iter()
            .map(|catch| smir::CatchClause {
                local: self.local_map[&catch.local],
                ty: Box::new(self.lower_type(catch.ty)),
                body: self.lower_statements(&catch.body),
                span: catch.span,
            })
            .collect();
        let finally_body = try_
            .finally_body
            .as_ref()
            .map(|body| self.lower_statements(body));
        smir::Try {
            body,
            catches,
            finally_body,
        }
    }

    /// Construct and throw one compiler-known exception. The zero-argument
    /// constructor target is complete in LocalConcrete HIR, so this operation
    /// only transposes typed identities.
    fn throw_builtin(&mut self, exception: hir::CompilerException, span: Span) -> smir::Statement {
        let class = exception.class();
        let ctor = self.ctors[&class];
        let exception_ty = mir::Type::Class(self.class_map[&class]);
        smir::Statement {
            kind: smir::StatementKind::Throw(smir::Expr::new(
                exception_ty.clone(),
                smir::ExprKind::Call(smir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::User(ctor),
                    },
                    args: Vec::new(),
                    return_ty: exception_ty,
                }),
            )),
            span,
        }
    }

    /// The M8 array bounds check (DESIGN section 1), shared by
    /// `ArrayGet` and `ArraySet`:
    /// `if (index < 0 || index >= array.size) throw IndexOutOfBoundsException()`.
    /// CFG normalization expands the `||` into branch edges.
    fn bounds_check(
        &mut self,
        array_type: mir::ClassId,
        array: mir::LocalId,
        index: mir::LocalId,
        span: Span,
    ) {
        let out_of_bounds = logic(
            smir::LogicOp::Or,
            smir::Expr::new(
                mir::Type::Boolean,
                smir::ExprKind::Binary {
                    op: mir::BinOp::IntLt,
                    lhs: Box::new(smir::Expr::local(index, mir::Type::Int)),
                    rhs: Box::new(smir::Expr::int(0)),
                },
            ),
            smir::Expr::new(
                mir::Type::Boolean,
                smir::ExprKind::Binary {
                    op: mir::BinOp::IntGe,
                    lhs: Box::new(smir::Expr::local(index, mir::Type::Int)),
                    rhs: Box::new(smir::Expr::new(
                        mir::Type::Int,
                        smir::ExprKind::ArrayLen {
                            array_type,
                            operand: Box::new(smir::Expr::local(
                                array,
                                mir::Type::Class(array_type),
                            )),
                        },
                    )),
                },
            ),
        );
        let throw = self.throw_builtin(
            self.module.exception_core.index_out_of_bounds_exception,
            span,
        );
        self.prelude.push(smir::StatementKind::If {
            cond: out_of_bounds,
            then_body: vec![throw],
            else_body: None,
        });
    }

    /// A `val` declaration: either the plain M1–M3 binding form, or a
    /// destructuring declaration (spec 4.6) whose init value is
    /// evaluated once into a hidden local that the pattern's bindings
    /// extract from.
    fn lower_val_decl(
        &mut self,
        pattern: &hir::Pattern,
        init: &hir::Expr,
        span: Span,
        out: &mut Vec<smir::Statement>,
    ) {
        if let hir::Pattern::Binding { local } = pattern {
            let local = self.local_map[local];
            let init = self.lower_expr(init);
            self.drain_prelude(span, out);
            out.push(smir::Statement {
                kind: smir::StatementKind::ValDecl { local, init },
                span,
            });
            return;
        }
        let ty = self.lower_type(init.ty);
        let init = self.lower_expr(init);
        self.drain_prelude(span, out);
        let slot = self.new_hidden("bind", ty.clone(), false);
        out.push(smir::Statement {
            kind: smir::StatementKind::ValDecl { local: slot, init },
            span,
        });
        let mut path = Vec::new();
        let mut bindings = Vec::new();
        let cond = self.lower_pattern(pattern, slot, &mut path, &ty, &mut bindings);
        // hir-lower only emits irrefutable patterns (tuple / struct /
        // binding / wildcard) in destructuring declarations.
        debug_assert!(cond.is_none(), "destructuring patterns are irrefutable");
        for (local, init) in bindings {
            out.push(smir::Statement {
                kind: smir::StatementKind::ValDecl { local, init },
                span,
            });
        }
    }

    fn lower_while(
        &mut self,
        cond: &hir::Expr,
        body: &[hir::Statement],
        span: Span,
        out: &mut Vec<smir::Statement>,
    ) {
        let cond_mir = self.lower_expr(cond);
        if self.prelude.is_empty() {
            let body = self.lower_statements(body);
            out.push(smir::Statement {
                kind: smir::StatementKind::While {
                    cond: cond_mir,
                    body,
                },
                span,
            });
            return;
        }
        // The condition contains a trap test (`!!`), which is a
        // statement sequence and must run on every iteration:
        // `P; while (C) B` becomes `P; var $c = C; while ($c) { B; P;
        // $c = C }`. The condition and its prelude are lowered twice;
        // each execution path still evaluates them exactly once per
        // iteration.
        self.drain_prelude(span, out);
        let cond_local = self.new_hidden("cond", mir::Type::Boolean, true);
        out.push(smir::Statement {
            kind: smir::StatementKind::ValDecl {
                local: cond_local,
                init: cond_mir,
            },
            span,
        });
        let mut body = self.lower_statements(body);
        let cond_again = self.lower_expr(cond);
        let prelude_again = std::mem::take(&mut self.prelude);
        body.extend(
            prelude_again
                .into_iter()
                .map(|kind| smir::Statement { kind, span }),
        );
        body.push(smir::Statement {
            kind: smir::StatementKind::Assign {
                local: cond_local,
                value: cond_again,
            },
            span,
        });
        out.push(smir::Statement {
            kind: smir::StatementKind::While {
                cond: smir::Expr::local(cond_local, mir::Type::Boolean),
                body,
            },
            span,
        });
    }

    /// `when` becomes a decision sequence (DESIGN 3.3): the subject is
    /// evaluated once into a hidden local, then the arms chain if/else
    /// tests; the `else` arm is the fallback.
    fn lower_when(&mut self, when: &hir::When, span: Span, out: &mut Vec<smir::Statement>) {
        let subject_ty = self.lower_type(when.subject.ty);
        let subject_init = self.lower_expr(&when.subject);
        self.drain_prelude(span, out);
        let subject = self.new_hidden("when", subject_ty.clone(), false);
        out.push(smir::Statement {
            kind: smir::StatementKind::ValDecl {
                local: subject,
                init: subject_init,
            },
            span,
        });
        let mut chain =
            self.lower_arms(&when.arms, subject, &subject_ty, when.else_body.as_deref());
        out.append(&mut chain);
    }

    /// Lower `arms` into the decision sequence: each arm is
    /// `if (<pattern condition>) { <bindings>; [if (<guard>) <body>
    /// else <next>] } else <next>` — a failed guard falls through to
    /// the next arm. With no guard the arm body is the then branch
    /// directly; an unconditionally matching arm (binding / wildcard,
    /// no guard) is inlined and makes the remaining arms unreachable
    /// (hir-lower rejects those). Exhaustiveness was checked at HIR,
    /// so the innermost else can only be reached via `else_body`.
    fn lower_arms(
        &mut self,
        arms: &[hir::WhenArm],
        subject: mir::LocalId,
        subject_ty: &mir::Type,
        else_body: Option<&[hir::Statement]>,
    ) -> Vec<smir::Statement> {
        let Some((arm, rest)) = arms.split_first() else {
            return else_body
                .map(|body| self.lower_statements(body))
                .unwrap_or_default();
        };
        let mut path = Vec::new();
        let mut bindings = Vec::new();
        let cond = self.lower_pattern(&arm.pattern, subject, &mut path, subject_ty, &mut bindings);
        let mut then: Vec<smir::Statement> = bindings
            .into_iter()
            .map(|(local, init)| smir::Statement {
                kind: smir::StatementKind::ValDecl { local, init },
                span: arm.span,
            })
            .collect();
        if let Some(guard) = &arm.guard {
            let guard_cond = self.lower_expr(guard);
            let guard_prelude = std::mem::take(&mut self.prelude);
            then.extend(guard_prelude.into_iter().map(|kind| smir::Statement {
                kind,
                span: arm.span,
            }));
            let body = self.lower_statements(&arm.body);
            let next = self.lower_arms(rest, subject, subject_ty, else_body);
            then.push(smir::Statement {
                kind: smir::StatementKind::If {
                    cond: guard_cond,
                    then_body: body,
                    else_body: non_empty(next),
                },
                span: arm.span,
            });
        } else {
            then.extend(self.lower_statements(&arm.body));
        }
        let Some(cond) = cond else {
            // Matches unconditionally; `rest` is unreachable.
            return then;
        };
        let next = self.lower_arms(rest, subject, subject_ty, else_body);
        vec![smir::Statement {
            kind: smir::StatementKind::If {
                cond,
                then_body: then,
                else_body: non_empty(next),
            },
            span: arm.span,
        }]
    }

    /// Lower a pattern matching the value at `root` + `path` (of MIR
    /// type `ty`): returns the match condition (`None` when the
    /// pattern matches unconditionally) and appends the binding
    /// initializers — `local = <value at path>` — in declaration
    /// order. CFG normalization expands the condition's `&&` chain, so a
    /// variant field is only extracted once its tag test has passed.
    fn lower_pattern(
        &mut self,
        pattern: &hir::Pattern,
        root: mir::LocalId,
        path: &mut Vec<Access>,
        ty: &mir::Type,
        bindings: &mut Vec<(mir::LocalId, smir::Expr)>,
    ) -> Option<smir::Expr> {
        match pattern {
            hir::Pattern::Binding { local } => {
                let init = self.accessed(root, path);
                bindings.push((self.local_map[local], init));
                None
            }
            hir::Pattern::Wildcard => None,
            hir::Pattern::Literal {
                value,
                equals,
                subject_ty,
            } => {
                debug_assert_eq!(self.lower_type(*subject_ty), *ty);
                let function = self.module.callable_function(*equals);
                let callee = self.instances.get(function).map_or_else(
                    || mir::Callee::User(self.function_map[&function]),
                    mir::Callee::Monomorphized,
                );
                Some(smir::Expr::new(
                    mir::Type::Boolean,
                    smir::ExprKind::Call(smir::Call {
                        target: mir::CallTarget {
                            kind: mir::CallKind::Direct,
                            callee,
                        },
                        args: vec![self.accessed(root, path), self.lower_expr(value)],
                        return_ty: mir::Type::Boolean,
                    }),
                ))
            }
            hir::Pattern::Variant {
                variant, fields, ..
            } => {
                let mir::Type::Enum(enum_id, _) = ty else {
                    unreachable!("a variant pattern matches an enum value")
                };
                let enum_id = *enum_id;
                let variant = variant.into_raw();
                let tag = smir::Expr::new(
                    mir::Type::Int,
                    smir::ExprKind::EnumTag(Box::new(self.accessed(root, path))),
                );
                let mut cond = smir::Expr::new(
                    mir::Type::Boolean,
                    smir::ExprKind::Binary {
                        op: mir::BinOp::IntEq,
                        lhs: Box::new(tag),
                        rhs: Box::new(smir::Expr::int(i64::from(variant))),
                    },
                );
                for (index, sub) in fields {
                    let field_ty = self.enums.defs[enum_id].variants[variant as usize].fields
                        [*index as usize]
                        .ty
                        .clone();
                    path.push(Access::EnumField {
                        variant,
                        index: *index,
                    });
                    if let Some(sub_cond) = self.lower_pattern(sub, root, path, &field_ty, bindings)
                    {
                        cond = and(cond, sub_cond);
                    }
                    path.pop();
                }
                Some(cond)
            }
            hir::Pattern::Tuple(elements) => {
                let mir::Type::Tuple(element_types) = ty else {
                    unreachable!("a tuple pattern matches a tuple value")
                };
                let element_types = element_types.clone();
                let mut cond: Option<smir::Expr> = None;
                for (index, sub) in elements.iter().enumerate() {
                    path.push(Access::Field(index as u32));
                    if let Some(sub_cond) =
                        self.lower_pattern(sub, root, path, &element_types[index], bindings)
                    {
                        cond = Some(match cond {
                            None => sub_cond,
                            Some(acc) => and(acc, sub_cond),
                        });
                    }
                    path.pop();
                }
                cond
            }
            hir::Pattern::Struct { fields, .. } => {
                let mir::Type::Struct(struct_id) = ty else {
                    unreachable!("a struct pattern matches a struct value")
                };
                let field_types: Vec<mir::Type> = self.structs.defs[*struct_id]
                    .declared_fields()
                    .iter()
                    .map(|field| field.ty.clone())
                    .collect();
                let mut cond: Option<smir::Expr> = None;
                for (index, sub) in fields {
                    path.push(Access::Field(*index));
                    if let Some(sub_cond) =
                        self.lower_pattern(sub, root, path, &field_types[*index as usize], bindings)
                    {
                        cond = Some(match cond {
                            None => sub_cond,
                            Some(acc) => and(acc, sub_cond),
                        });
                    }
                    path.pop();
                }
                cond
            }
        }
    }

    pub(super) fn lower_expr(&mut self, expr: &hir::Expr) -> smir::Expr {
        let ty = self.lower_type(expr.ty);
        let kind = match &expr.kind {
            hir::ExprKind::StringLiteral(value) => {
                // One global constant per literal occurrence, numbered
                // in order of appearance (deterministic).
                let symbol = format!("scoop.str.{}", self.strings.len());
                let id = self.strings.alloc(mir::StringConst {
                    value: value.clone(),
                    symbol,
                });
                smir::ExprKind::StringConst(id)
            }
            hir::ExprKind::IntLiteral(value) => smir::ExprKind::IntLiteral(*value),
            hir::ExprKind::BoolLiteral(value) => smir::ExprKind::BoolLiteral(*value),
            hir::ExprKind::UnitLiteral => smir::ExprKind::UnitLiteral,
            hir::ExprKind::TupleLiteral(elements) => {
                smir::ExprKind::TupleLiteral(elements.iter().map(|e| self.lower_expr(e)).collect())
            }
            hir::ExprKind::StructInit { args, .. } => {
                // The (possibly instantiated) struct def comes from
                // the expression's type: generic applications (M9)
                // resolve to their instance, plain structs to the
                // base definition.
                let mir::Type::Struct(struct_id) = self.lower_type(expr.ty) else {
                    unreachable!("a struct construction has a struct type")
                };
                smir::ExprKind::StructInit {
                    struct_id,
                    args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                }
            }
            // Class construction calls the class's constructor
            // function (`scoop.ctor.<Class>`); the raw allocation and
            // field initialization live inside it (see `lower_ctor`).
            hir::ExprKind::ClassInit { class_id, args } => {
                let ctor = self.ctors[class_id];
                smir::ExprKind::Call(smir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::User(ctor),
                    },
                    args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                    return_ty: self.lower_type(expr.ty),
                })
            }
            hir::ExprKind::VariantConstruct { variant, args, .. } => {
                smir::ExprKind::VariantConstruct {
                    variant: variant.into_raw(),
                    fields: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                }
            }
            hir::ExprKind::Local(local) => {
                let local = self.local_map[local];
                let narrowed = self.lower_type(expr.ty);
                if self.locals[local].ty == narrowed {
                    smir::ExprKind::Local(local)
                } else if let mir::Type::Function(function_type) = narrowed {
                    return self.adapt_checked_function_value(
                        smir::Expr::local(local, self.locals[local].ty.clone()),
                        function_type,
                        expr.span,
                    );
                } else {
                    smir::ExprKind::Retype {
                        operand: Box::new(smir::Expr::local(local, self.locals[local].ty.clone())),
                        ty: Box::new(narrowed),
                    }
                }
            }
            hir::ExprKind::ConstructorParam(parameter) => {
                smir::ExprKind::Local(self.constructor_param_map[parameter])
            }
            hir::ExprKind::GlobalRead(global) => {
                smir::ExprKind::GlobalRead(self.global_map[global])
            }
            hir::ExprKind::Capture(binding) => {
                if let Some(local) = self.current_local_capture_params.get(binding) {
                    return smir::Expr::new(ty, smir::ExprKind::Local(self.local_map[local]));
                }
                let class = self
                    .current_closure
                    .expect("capture reads only appear in closure invoke bodies");
                let index = self.closure_capture_indices[&(class, *binding)];
                let closure = self
                    .current_closure_local
                    .expect("a closure invoke body has its hidden receiver local");
                smir::ExprKind::ClosureCapture {
                    closure: Box::new(smir::Expr::local(closure, self.locals[closure].ty.clone())),
                    class,
                    index,
                }
            }
            hir::ExprKind::Lambda(id) => {
                let class = self.ensure_lambda_closure(*id);
                let sources: Vec<_> = self.module.lambdas[*id]
                    .captures
                    .iter()
                    .map(|capture| capture.source.clone())
                    .collect();
                smir::ExprKind::ClosureAlloc {
                    class,
                    captures: sources
                        .iter()
                        .map(|source| self.lower_expr(source))
                        .collect(),
                }
            }
            hir::ExprKind::AnonymousFunction(id) => {
                let class = self.ensure_anonymous_closure(*id);
                let sources: Vec<_> = self.module.anonymous_functions[*id]
                    .captures
                    .iter()
                    .map(|capture| capture.source.clone())
                    .collect();
                smir::ExprKind::ClosureAlloc {
                    class,
                    captures: sources
                        .iter()
                        .map(|source| self.lower_expr(source))
                        .collect(),
                }
            }
            hir::ExprKind::CallableReference(id) => {
                let class = self.ensure_reference_closure(*id);
                let reference = &self.module.callable_references[*id];
                let mut captures = Vec::with_capacity(
                    reference.captures.len()
                        + usize::from(matches!(
                            &reference.target,
                            hir::CallableReferenceTarget::BoundMember { .. }
                                | hir::CallableReferenceTarget::BoundExtension { .. }
                        )),
                );
                match &reference.target {
                    hir::CallableReferenceTarget::BoundMember { receiver, .. }
                    | hir::CallableReferenceTarget::BoundExtension { receiver, .. } => {
                        captures.push(self.lower_expr(receiver));
                    }
                    _ => {}
                }
                captures.extend(
                    reference
                        .captures
                        .iter()
                        .map(|capture| self.lower_expr(&capture.source)),
                );
                smir::ExprKind::ClosureAlloc { class, captures }
            }
            hir::ExprKind::FunctionCoercion {
                source,
                coercion,
                target_type,
            } => {
                let conversion = &self.module.function_coercions[*coercion];
                debug_assert_eq!(conversion.target, *target_type);
                let source_type = self.lower_function_type_id(conversion.source);
                let target_type = self.lower_function_type_id(*target_type);
                let value = self.lower_expr(source);
                return self.adapt_function_value(value, source_type, target_type, expr.span);
            }
            // Array nodes carry the exact concrete intrinsic class identity;
            // LIR never reconstructs it from an element layout or context.
            hir::ExprKind::ArrayLiteral(elements) => {
                let mir::Type::Class(array_type) = self.lower_type(expr.ty) else {
                    unreachable!("an array literal has an intrinsic class type")
                };
                smir::ExprKind::ArrayLiteral {
                    array_type,
                    elements: elements.iter().map(|e| self.lower_expr(e)).collect(),
                }
            }
            // Subscript read. M8: the bounds check moved here from
            // codegen — the array and the index are evaluated once
            // into hidden locals, then `IndexOutOfBoundsException`
            // throws when the index is out of range (the prelude
            // mechanism `!!` uses).
            hir::ExprKind::Index { receiver, index } => {
                let array_ty = self.lower_type(receiver.ty);
                let mir::Type::Class(array_type) = array_ty else {
                    unreachable!("an array subscript has an intrinsic class receiver")
                };
                let array_slot = self.new_hidden("arr", mir::Type::Class(array_type), false);
                let index_slot = self.new_hidden("idx", mir::Type::Int, false);
                let array = self.lower_expr(receiver);
                self.prelude.push(smir::StatementKind::ValDecl {
                    local: array_slot,
                    init: array,
                });
                let index = self.lower_expr(index);
                self.prelude.push(smir::StatementKind::ValDecl {
                    local: index_slot,
                    init: index,
                });
                self.bounds_check(array_type, array_slot, index_slot, expr.span);
                smir::ExprKind::ArrayGet {
                    array_type,
                    array: Box::new(smir::Expr::local(array_slot, mir::Type::Class(array_type))),
                    index: Box::new(smir::Expr::local(index_slot, mir::Type::Int)),
                }
            }
            hir::ExprKind::ArrayLen(operand) => {
                let mir::Type::Class(array_type) = self.lower_type(operand.ty) else {
                    unreachable!("array.size has an intrinsic class receiver")
                };
                smir::ExprKind::ArrayLen {
                    array_type,
                    operand: Box::new(self.lower_expr(operand)),
                }
            }
            hir::ExprKind::ArrayClone(operand) => {
                let mir::Type::Class(source_type) = self.lower_type(operand.ty) else {
                    unreachable!("an array conversion has an intrinsic class source")
                };
                let mir::Type::Class(target_type) = self.lower_type(expr.ty) else {
                    unreachable!("an array conversion has an intrinsic class target")
                };
                smir::ExprKind::ArrayClone {
                    source_type,
                    target_type,
                    operand: Box::new(self.lower_expr(operand)),
                }
            }
            hir::ExprKind::PtrFromUInt(operand) => {
                let mir::Type::Ptr(pointee) = self.lower_type(expr.ty) else {
                    unreachable!("PtrFromUInt has a pointer type")
                };
                smir::ExprKind::PtrFromUInt {
                    operand: Box::new(self.lower_expr(operand)),
                    pointee,
                }
            }
            hir::ExprKind::PtrToUInt(operand) => {
                smir::ExprKind::PtrToUInt(Box::new(self.lower_expr(operand)))
            }
            hir::ExprKind::PtrCast(operand) => {
                let mir::Type::Ptr(pointee) = self.lower_type(expr.ty) else {
                    unreachable!("PtrCast has a pointer type")
                };
                smir::ExprKind::PtrCast {
                    operand: Box::new(self.lower_expr(operand)),
                    pointee,
                }
            }
            hir::ExprKind::PtrLoad { pointer, offset } => {
                let hir::TypeKind::Ptr(pointee) = self.module.types[pointer.ty].kind else {
                    unreachable!("PtrLoad has a pointer operand")
                };
                smir::ExprKind::PtrLoad {
                    pointer: Box::new(self.lower_expr(pointer)),
                    pointee: Box::new(self.lower_type(pointee)),
                    offset: offset
                        .as_ref()
                        .map(|offset| Box::new(self.lower_expr(offset))),
                }
            }
            hir::ExprKind::PtrStore {
                pointer,
                offset,
                value,
            } => {
                let hir::TypeKind::Ptr(pointee) = self.module.types[pointer.ty].kind else {
                    unreachable!("PtrStore has a pointer operand")
                };
                smir::ExprKind::PtrStore {
                    pointer: Box::new(self.lower_expr(pointer)),
                    pointee: Box::new(self.lower_type(pointee)),
                    offset: offset
                        .as_ref()
                        .map(|offset| Box::new(self.lower_expr(offset))),
                    value: Box::new(self.lower_expr(value)),
                }
            }
            hir::ExprKind::PtrOffset {
                pointer,
                offset,
                subtract,
            } => {
                let hir::TypeKind::Ptr(pointee) = self.module.types[pointer.ty].kind else {
                    unreachable!("PtrOffset has a pointer operand")
                };
                smir::ExprKind::PtrOffset {
                    pointer: Box::new(self.lower_expr(pointer)),
                    pointee: Box::new(self.lower_type(pointee)),
                    offset: Box::new(self.lower_expr(offset)),
                    subtract: *subtract,
                }
            }
            hir::ExprKind::AddressOf(hir::Place::Local(local)) => {
                let local = self.local_map[local];
                smir::ExprKind::AddressOf {
                    local,
                    pointee: Box::new(self.locals[local].ty.clone()),
                }
            }
            hir::ExprKind::AddressOf(hir::Place::Global(global)) => smir::ExprKind::GlobalAddress {
                global: self.global_map[global],
                pointee: Box::new(self.lower_type(self.module.globals[*global].ty)),
            },
            hir::ExprKind::SizeOf(ty) => smir::ExprKind::SizeOf(Box::new(self.lower_type(*ty))),
            hir::ExprKind::AlignOf(ty) => smir::ExprKind::AlignOf(Box::new(self.lower_type(*ty))),
            hir::ExprKind::FunPtrNull => {
                let mir::Type::FunPtr(signature) = self.lower_type(expr.ty) else {
                    unreachable!("FunPtrNull has a FunPtr type")
                };
                smir::ExprKind::FunPtrNull(signature)
            }
            hir::ExprKind::FunctionAddress(function) => {
                let mir::Type::FunPtr(signature) = self.lower_type(expr.ty) else {
                    unreachable!("FunctionAddress has a FunPtr type")
                };
                let callback =
                    self.ensure_callback_bridge(self.function_map[function], signature, expr.span);
                smir::ExprKind::FunctionAddress { callback }
            }
            hir::ExprKind::ForeignCallbackRegister {
                registration,
                closure,
            } => {
                let bridge = self.ensure_foreign_callback_bridge(*registration, expr.span);
                smir::ExprKind::ForeignCallbackRegister {
                    bridge,
                    closure: Box::new(self.lower_expr(closure)),
                }
            }
            hir::ExprKind::ForeignCallbackOperation {
                operation,
                callback,
            } => smir::ExprKind::ForeignCallbackOperation {
                operation: match operation {
                    hir::ForeignCallbackOperation::Retain => mir::ForeignCallbackOperation::Retain,
                    hir::ForeignCallbackOperation::Release => {
                        mir::ForeignCallbackOperation::Release
                    }
                    hir::ForeignCallbackOperation::State => mir::ForeignCallbackOperation::State,
                    hir::ForeignCallbackOperation::Failure => {
                        mir::ForeignCallbackOperation::Failure
                    }
                },
                callback: Box::new(self.lower_expr(callback)),
                result_ty: Box::new(self.lower_type(expr.ty)),
            },
            hir::ExprKind::FieldAccess { receiver, field } => {
                // Struct fields, tuple elements and class constructor
                // properties are all 0-based here (the class index
                // follows the flattened base-prefix layout; LIR turns
                // it into a heap object load).
                let index = match field {
                    hir::FieldRef::StructField { index, .. }
                    | hir::FieldRef::ClassField { index, .. }
                    | hir::FieldRef::TupleIndex(index) => *index,
                };
                smir::ExprKind::FieldAccess {
                    receiver: Box::new(self.lower_expr(receiver)),
                    index,
                }
            }
            hir::ExprKind::MethodCall {
                receiver,
                callee,
                args,
            } => return self.lower_method_call(receiver, *callee, args, expr.ty),
            // `Box` / `Unbox` / `is` stay dedicated MIR nodes; LIR
            // lowers them (the runtime box call, the payload load,
            // the `scoop_rt_is_instance` call). Boxing registers the
            // boxed value type (and the target interface) on the way.
            hir::ExprKind::Box(operand) => {
                let payload = self.lower_type(operand.ty);
                self.register_boxed(&payload, Some(expr.ty));
                smir::ExprKind::Box(Box::new(self.lower_expr(operand)))
            }
            // Smart casts unbox inline wherever the narrowed local is read
            // (e.g. as a field-access receiver). Every unbox is bound to a
            // typed hidden local so the synthetic expression and its result
            // local carry the same complete type.
            hir::ExprKind::Unbox(operand) => {
                let ty = self.lower_type(expr.ty);
                let operand = self.lower_expr(operand);
                let slot = self.new_hidden("ub", ty.clone(), false);
                self.prelude.push(smir::StatementKind::ValDecl {
                    local: slot,
                    init: smir::Expr::new(ty, smir::ExprKind::Unbox(Box::new(operand))),
                });
                smir::ExprKind::Local(slot)
            }
            hir::ExprKind::IsInstance { operand, check_ty } => {
                let check_ty = self.lower_type(*check_ty);
                self.register_check(&check_ty);
                smir::ExprKind::IsInstance {
                    operand: Box::new(self.lower_expr(operand)),
                    check_ty: Box::new(check_ty),
                }
            }
            hir::ExprKind::Cast { operand, optional } => {
                return self.lower_cast(operand, *optional, expr.ty, expr.span);
            }
            hir::ExprKind::Call { callee, args } => {
                return self.lower_call(*callee, args, expr.ty);
            }
            hir::ExprKind::LocalFunctionCall {
                callee,
                captures,
                args,
                ..
            } => {
                let callee = self.lower_user_callee(*callee);
                let call_args: Vec<_> = captures.iter().chain(args).collect();
                let return_ty = self.lower_type(expr.ty);
                return self.call(callee, &call_args, return_ty);
            }
            hir::ExprKind::CallableCall {
                callee,
                function_type,
                args,
            } => {
                let function_type = self.lower_function_type_id(*function_type);
                let mut call_args = Vec::with_capacity(args.len() + 1);
                call_args.push(self.lower_expr(callee));
                call_args.extend(args.iter().map(|arg| self.lower_expr(arg)));
                smir::ExprKind::Call(smir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Closure { function_type },
                        callee: mir::Callee::Closure(function_type),
                    },
                    args: call_args,
                    return_ty: self.lower_type(expr.ty),
                })
            }
            hir::ExprKind::Binary { op, lhs, rhs } => {
                return self.lower_binary(*op, lhs, rhs, expr.span);
            }
            hir::ExprKind::Unary { op, operand } => {
                let operand = Box::new(self.lower_expr(operand));
                let op = match op {
                    hir::UnOp::Neg => mir::UnOp::IntNeg,
                    hir::UnOp::Not => mir::UnOp::BoolNot,
                };
                smir::ExprKind::Unary { op, operand }
            }
            // The Option nodes (hir-lower's `?.` / `?:` / `!!`
            // desugars) become generic enum operations on core's
            // `Option` enum (DESIGN 3.3).
            hir::ExprKind::SomeWrap(operand) => {
                let (some, _) = self.option_variants;
                smir::ExprKind::VariantConstruct {
                    variant: some,
                    fields: vec![self.lower_expr(operand)],
                }
            }
            hir::ExprKind::NoneLiteral => {
                let (_, none) = self.option_variants;
                smir::ExprKind::VariantConstruct {
                    variant: none,
                    fields: Vec::new(),
                }
            }
            hir::ExprKind::IsSome(operand) => {
                let (some, _) = self.option_variants;
                let operand = self.lower_expr(operand);
                smir::ExprKind::Binary {
                    op: mir::BinOp::IntEq,
                    lhs: Box::new(smir::Expr::new(
                        mir::Type::Int,
                        smir::ExprKind::EnumTag(Box::new(operand)),
                    )),
                    rhs: Box::new(smir::Expr::int(i64::from(some))),
                }
            }
            hir::ExprKind::Unwrap {
                operand,
                trap_on_none,
            } => {
                let (some, _) = self.option_variants;
                if *trap_on_none {
                    return self.trapping_unwrap(operand, expr.ty, expr.span, some);
                } else {
                    // The surrounding control flow already guarantees
                    // `Some` (`?.` / `?:` desugars, the equality
                    // expansion).
                    smir::ExprKind::EnumField {
                        operand: Box::new(self.lower_expr(operand)),
                        variant: some,
                        index: 0,
                    }
                }
            }
        };
        smir::Expr::new(ty, kind)
    }

    fn ensure_callback_bridge(
        &mut self,
        source: mir::FunctionId,
        signature: mir::FunctionTypeId,
        span: Span,
    ) -> mir::CallbackBridgeId {
        if let Some(callback) = self.callback_by_target.get(&(source, signature)) {
            return *callback;
        }

        let callback_index = self.callback_bridges.len();
        let signature_def = self.shell.function_types[signature].clone();
        let source_name = self.functions[source].name.clone();
        let mut locals = Arena::new();
        let mut params = Vec::new();

        let result_storage = if signature_def.return_type == mir::Type::Unit {
            None
        } else {
            let ty = mir::Type::Ptr(Box::new(signature_def.return_type.clone()));
            let local = locals.alloc(mir::Local {
                name: "$result".to_string(),
                ty: ty.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name: "$result".to_string(),
                ty,
                local,
            });
            Some(local)
        };

        let mut args = Vec::with_capacity(signature_def.parameter_types.len());
        for (index, parameter_type) in signature_def.parameter_types.iter().enumerate() {
            let name = format!("$arg{index}");
            let pointer_type = mir::Type::Ptr(Box::new(parameter_type.clone()));
            let local = locals.alloc(mir::Local {
                name: name.clone(),
                ty: pointer_type.clone(),
                mutable: false,
            });
            params.push(mir::Param {
                name,
                ty: pointer_type.clone(),
                local,
            });
            args.push(mir::Expr::new(
                parameter_type.clone(),
                mir::ExprKind::PtrLoad {
                    pointer: Box::new(mir::Expr::local(local, pointer_type)),
                    pointee: Box::new(parameter_type.clone()),
                    offset: None,
                },
            ));
        }

        let call = mir::Call {
            target: mir::CallTarget {
                kind: mir::CallKind::Direct,
                callee: mir::Callee::User(source),
            },
            args,
        };
        let mut statements = Vec::new();
        if let Some(result_storage) = result_storage {
            let result = locals.alloc(mir::Local {
                name: "$value".to_string(),
                ty: signature_def.return_type.clone(),
                mutable: false,
            });
            statements.push(mir::Statement {
                kind: mir::StatementKind::Call(mir::CallEffect::Value {
                    destination: result,
                    call,
                }),
                span,
            });
            statements.push(mir::Statement {
                kind: mir::StatementKind::Expr(mir::Expr::new(
                    mir::Type::Unit,
                    mir::ExprKind::PtrStore {
                        pointer: Box::new(mir::Expr::local(
                            result_storage,
                            mir::Type::Ptr(Box::new(signature_def.return_type.clone())),
                        )),
                        pointee: Box::new(signature_def.return_type.clone()),
                        offset: None,
                        value: Box::new(mir::Expr::local(
                            result,
                            signature_def.return_type.clone(),
                        )),
                    },
                )),
                span,
            });
        } else {
            statements.push(mir::Statement {
                kind: mir::StatementKind::Call(mir::CallEffect::Unit(call)),
                span,
            });
        }

        let mut blocks = Arena::new();
        let entry = blocks.alloc(mir::BasicBlock {
            name: "entry".to_string(),
            statements,
            terminator: mir::Terminator::Return { value: None },
            unwind: None,
        });
        let bridge_function = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::NoGc,
            name: format!("callback bridge for {source_name}"),
            symbol: format!("scoop_callback_bridge_{callback_index}"),
            params,
            return_ty: mir::Type::Unit,
            body: mir::Body {
                locals,
                blocks,
                entry,
            },
        });
        self.top_level.push(bridge_function);
        let callback = self.callback_bridges.alloc(mir::CallbackBridge {
            source,
            signature,
            bridge_function,
        });
        self.callback_by_target
            .insert((source, signature), callback);
        callback
    }

    fn ensure_foreign_callback_bridge(
        &mut self,
        registration_id: hir::ForeignCallbackRegistrationId,
        span: Span,
    ) -> mir::ForeignCallbackBridgeId {
        if let Some(&bridge) = self.foreign_callback_by_registration.get(&registration_id) {
            return bridge;
        }

        let registration = self.module.foreign_callback_registrations[registration_id].clone();
        let native_signature = self.lower_function_type_id(registration.native_function_type);
        let managed_signature = self.lower_function_type_id(registration.managed_function_type);
        let callback = self.struct_map[&registration.callback];
        let signature = self.shell.function_types[managed_signature].clone();
        debug_assert!(!signature.is_suspend);

        let mut locals = Arena::new();
        let closure_ty = mir::Type::Function(managed_signature);
        let closure = locals.alloc(mir::Local {
            name: "$closure".to_string(),
            ty: closure_ty.clone(),
            mutable: false,
        });
        let result_pointer_ty = mir::Type::Ptr(Box::new(signature.return_type.clone()));
        let result_storage = locals.alloc(mir::Local {
            name: "$result".to_string(),
            ty: result_pointer_ty.clone(),
            mutable: false,
        });
        let opaque_pointer = mir::Type::Ptr(Box::new(mir::Type::Unit));
        let arguments_pointer_ty = mir::Type::Ptr(Box::new(opaque_pointer.clone()));
        let argument_storage = locals.alloc(mir::Local {
            name: "$arguments".to_string(),
            ty: arguments_pointer_ty.clone(),
            mutable: false,
        });
        let throwable =
            mir::Type::Class(self.class_map[&self.module.exception_core.throwable.class()]);
        let exception_pointer_ty = mir::Type::Ptr(Box::new(throwable.clone()));
        let exception_out = locals.alloc(mir::Local {
            name: "$exception".to_string(),
            ty: exception_pointer_ty.clone(),
            mutable: false,
        });

        let params = vec![
            mir::Param {
                name: "$closure".to_string(),
                ty: closure_ty.clone(),
                local: closure,
            },
            mir::Param {
                name: "$result".to_string(),
                ty: result_pointer_ty.clone(),
                local: result_storage,
            },
            mir::Param {
                name: "$arguments".to_string(),
                ty: arguments_pointer_ty.clone(),
                local: argument_storage,
            },
            mir::Param {
                name: "$exception".to_string(),
                ty: exception_pointer_ty.clone(),
                local: exception_out,
            },
        ];

        let mut call_args = vec![mir::Expr::local(closure, closure_ty.clone())];
        for (index, parameter_ty) in signature.parameter_types.iter().enumerate() {
            let raw = mir::Expr::new(
                opaque_pointer.clone(),
                mir::ExprKind::PtrLoad {
                    pointer: Box::new(mir::Expr::local(
                        argument_storage,
                        arguments_pointer_ty.clone(),
                    )),
                    pointee: Box::new(opaque_pointer.clone()),
                    offset: Some(Box::new(mir::Expr::int(index as i64))),
                },
            );
            let parameter_pointer = mir::Type::Ptr(Box::new(parameter_ty.clone()));
            call_args.push(mir::Expr::new(
                parameter_ty.clone(),
                mir::ExprKind::PtrLoad {
                    pointer: Box::new(mir::Expr::new(
                        parameter_pointer,
                        mir::ExprKind::PtrCast {
                            operand: Box::new(raw),
                            pointee: Box::new(parameter_ty.clone()),
                        },
                    )),
                    pointee: Box::new(parameter_ty.clone()),
                    offset: None,
                },
            ));
        }
        let call = mir::Call {
            target: mir::CallTarget {
                kind: mir::CallKind::Closure {
                    function_type: managed_signature,
                },
                callee: mir::Callee::Closure(managed_signature),
            },
            args: call_args,
        };

        let exception = locals.alloc(mir::Local {
            name: "$caught".to_string(),
            ty: throwable.clone(),
            mutable: false,
        });
        let statement = |kind| mir::Statement { kind, span };
        let mut blocks = Arena::new();
        let catch = blocks.alloc(mir::BasicBlock {
            name: "callback.failure".to_string(),
            statements: vec![
                statement(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                    cleanup: false,
                })),
                statement(mir::StatementKind::Eh(mir::EhStatement::BeginCatch)),
                statement(mir::StatementKind::Call(mir::CallEffect::Value {
                    destination: exception,
                    call: mir::Call {
                        target: mir::CallTarget {
                            kind: mir::CallKind::Direct,
                            callee: mir::Callee::Runtime(mir::RuntimeFn::MaterializeException),
                        },
                        args: vec![mir::Expr::caught_exception()],
                    },
                })),
                statement(mir::StatementKind::Eh(mir::EhStatement::EndCatch)),
                statement(mir::StatementKind::Expr(mir::Expr::new(
                    mir::Type::Unit,
                    mir::ExprKind::PtrStore {
                        pointer: Box::new(mir::Expr::local(
                            exception_out,
                            exception_pointer_ty.clone(),
                        )),
                        pointee: Box::new(throwable.clone()),
                        offset: None,
                        value: Box::new(mir::Expr::local(exception, throwable.clone())),
                    },
                ))),
            ],
            terminator: mir::Terminator::Return {
                value: Some(mir::Expr::new(
                    mir::Type::UInt,
                    mir::ExprKind::IntLiteral(1),
                )),
            },
            unwind: None,
        });

        let mut success_statements = Vec::new();
        if signature.return_type == mir::Type::Unit {
            success_statements.push(statement(mir::StatementKind::Call(mir::CallEffect::Unit(
                call,
            ))));
        } else {
            let value = locals.alloc(mir::Local {
                name: "$value".to_string(),
                ty: signature.return_type.clone(),
                mutable: false,
            });
            success_statements.push(statement(mir::StatementKind::Call(
                mir::CallEffect::Value {
                    destination: value,
                    call,
                },
            )));
            success_statements.push(statement(mir::StatementKind::Expr(mir::Expr::new(
                mir::Type::Unit,
                mir::ExprKind::PtrStore {
                    pointer: Box::new(mir::Expr::local(result_storage, result_pointer_ty.clone())),
                    pointee: Box::new(signature.return_type.clone()),
                    offset: None,
                    value: Box::new(mir::Expr::local(value, signature.return_type.clone())),
                },
            ))));
        }
        let entry = blocks.alloc(mir::BasicBlock {
            name: "entry".to_string(),
            statements: success_statements,
            terminator: mir::Terminator::Return {
                value: Some(mir::Expr::new(
                    mir::Type::UInt,
                    mir::ExprKind::IntLiteral(0),
                )),
            },
            unwind: Some(catch),
        });
        let adapter_index = self.foreign_callback_adapters.len();
        let function = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: format!("foreign callback adapter {adapter_index}"),
            symbol: format!("scoop_foreign_callback_adapter_{adapter_index}"),
            params,
            return_ty: mir::Type::UInt,
            body: mir::Body {
                locals,
                blocks,
                entry,
            },
        });
        self.top_level.push(function);
        let adapter = self
            .foreign_callback_adapters
            .alloc(mir::ForeignCallbackAdapter {
                function,
                managed_signature,
            });
        let bridge = self
            .foreign_callback_bridges
            .alloc(mir::ForeignCallbackBridge {
                adapter,
                callback,
                native_signature,
                context_index: registration.context_index,
                mode: match registration.mode {
                    hir::ForeignCallbackMode::Reusable => mir::ForeignCallbackMode::Reusable,
                    hir::ForeignCallbackMode::OneShot => mir::ForeignCallbackMode::OneShot,
                },
            });
        self.foreign_callback_by_registration
            .insert(registration_id, bridge);
        bridge
    }

    /// `x!!`: the operand is evaluated once into a hidden local, then
    /// `if (tag == Some) { val $uw = <field 0> } else { throw UnwrapException() }`.
    /// The if/else is queued in `prelude` — it must precede the
    /// statement this expression belongs to — and the expression
    /// itself becomes the result local. The exception is an ordinary
    /// constructor call (`throw_builtin`, M8).
    fn trapping_unwrap(
        &mut self,
        operand: &hir::Expr,
        result_ty: hir::TypeId,
        span: Span,
        some: u32,
    ) -> smir::Expr {
        let option_ty = self.lower_type(operand.ty);
        let payload_ty = self.lower_type(result_ty);
        let value = self.lower_expr(operand);
        let slot = self.new_hidden("opt", option_ty.clone(), false);
        let result = self.new_hidden("uw", payload_ty.clone(), false);
        let throw = self.throw_builtin(self.module.exception_core.unwrap_exception, span);
        self.prelude.push(smir::StatementKind::ValDecl {
            local: slot,
            init: value,
        });
        self.prelude.push(smir::StatementKind::If {
            cond: smir::Expr::new(
                mir::Type::Boolean,
                smir::ExprKind::Binary {
                    op: mir::BinOp::IntEq,
                    lhs: Box::new(smir::Expr::new(
                        mir::Type::Int,
                        smir::ExprKind::EnumTag(Box::new(smir::Expr::local(
                            slot,
                            option_ty.clone(),
                        ))),
                    )),
                    rhs: Box::new(smir::Expr::int(i64::from(some))),
                },
            ),
            then_body: vec![smir::Statement {
                kind: smir::StatementKind::ValDecl {
                    local: result,
                    init: smir::Expr::new(
                        payload_ty.clone(),
                        smir::ExprKind::EnumField {
                            operand: Box::new(smir::Expr::local(slot, option_ty)),
                            variant: some,
                            index: 0,
                        },
                    ),
                },
                span,
            }],
            else_body: Some(vec![throw]),
        });
        smir::Expr::local(result, payload_ty)
    }

    fn lower_call(
        &mut self,
        callable: hir::Callable,
        args: &[hir::Expr],
        result_ty: hir::TypeId,
    ) -> smir::Expr {
        let function = self.module.callable_function(callable);
        if let Some(protocol) = self
            .module
            .coroutine_protocol_for_function(function)
            .copied()
        {
            if function == protocol.start_coroutine {
                return self.lower_coroutine_start(protocol, args);
            }
            return self.lower_coroutine_suspend(protocol, args);
        }
        // `@Intrinsic` primitive functions (scoop.core, M7 DESIGN
        // section 2): handled up front — generic intrinsics (the M9
        // GC facilities) take this path too, before the generic-callee
        // arm below would reject their missing function-map entry.
        if let hir::FunctionKind::Intrinsic(intrinsic) = &self.module.functions[function].kind {
            return self.lower_intrinsic_call(intrinsic.kind, args, result_ty);
        }
        let callee = self.lower_user_callee(callable);
        let return_ty = self.lower_type(result_ty);
        self.call(callee, &args.iter().collect::<Vec<_>>(), return_ty)
    }

    fn lower_user_callee(&mut self, callable: hir::Callable) -> mir::Callee {
        let function = self.module.callable_function(callable);
        match &self.module.functions[function].kind {
            hir::FunctionKind::User(_) => self.instances.get(function).map_or_else(
                || mir::Callee::User(self.function_map[&function]),
                mir::Callee::Monomorphized,
            ),
            hir::FunctionKind::Extern(extern_id) => mir::Callee::Extern(self.extern_map[extern_id]),
            hir::FunctionKind::Intrinsic(_) => unreachable!("handled above"),
        }
    }

    fn lower_coroutine_start(
        &mut self,
        protocol: hir::CoroutineProtocol,
        args: &[hir::Expr],
    ) -> smir::Expr {
        let [task, completion] = args else {
            unreachable!("hir-lower validates startCoroutine's two parameters")
        };
        let result = self.lower_type(protocol.result_type);
        let task = self.lower_expr(task);
        let completion = self.lower_expr(completion);
        let task_interface = self.interfaces.mir_id(protocol.suspend_task);
        let continuation_interface = self.interfaces.mir_id(protocol.continuation);
        let (_, step_ty) = self
            .coroutines
            .step_for(&result, self.structs, self.enums, self.shell);
        let run = self.instances.get(protocol.suspend_task_run).unwrap();
        let resume = self.instances.get(protocol.continuation_resume).unwrap();
        let failure = self
            .instances
            .get(protocol.continuation_resume_with_exception)
            .unwrap();
        let throwable =
            mir::Type::Class(self.class_map[&self.module.exception_core.throwable.class()]);
        let helper = self.coroutines.start_helper(
            &result,
            task_interface,
            continuation_interface,
            run,
            resume,
            failure,
            &step_ty,
            throwable,
            self.functions,
            self.top_level,
            self.shell,
        );
        self.prelude.push(smir::StatementKind::Expr(smir::Expr::new(
            mir::Type::Unit,
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee: mir::Callee::User(helper),
                },
                args: vec![task, completion],
                return_ty: mir::Type::Unit,
            }),
        )));
        smir::Expr::unit()
    }

    fn lower_coroutine_suspend(
        &mut self,
        protocol: hir::CoroutineProtocol,
        args: &[hir::Expr],
    ) -> smir::Expr {
        let [registration] = args else {
            unreachable!("hir-lower validates suspendCoroutine's one parameter")
        };
        let result = self.lower_type(protocol.result_type);
        let registration_interface = self.interfaces.mir_id(protocol.suspend_registration);
        let register = self
            .instances
            .get(protocol.suspend_registration_register)
            .unwrap();
        smir::Expr::new(
            result.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Interface {
                        interface: registration_interface,
                        slot: 0,
                    },
                    callee: mir::Callee::CoroutineSuspend { register },
                },
                args: vec![self.lower_expr(registration)],
                return_ty: result,
            }),
        )
    }

    /// An `@Intrinsic` call: the typed intrinsic kind maps directly onto the
    /// runtime function (`print` / `println` themselves are ordinary
    /// overloaded core functions and take the `User` path in
    /// `lower_call`). Raw intrinsic names do not reach this stage.
    ///
    /// The M9 GC facilities (milestone9 DESIGN section 1, runtime spec
    /// 3.4) marshal between the raw machine word the runtime functions
    /// speak (`u64` addresses / handle values) and the Scoop-level
    /// handle aggregates: `pin` / `getGcHandle` wrap the word into the
    /// handle struct, `unpin` / `releaseGcHandle` unwrap it.
    fn lower_intrinsic_call(
        &mut self,
        kind: hir::IntrinsicFunctionKind,
        args: &[hir::Expr],
        result_ty: hir::TypeId,
    ) -> smir::Expr {
        let function = match kind {
            hir::IntrinsicFunctionKind::IntToString => mir::RuntimeFn::IntToString,
            hir::IntrinsicFunctionKind::BoolToString => mir::RuntimeFn::BoolToString,
            hir::IntrinsicFunctionKind::GcPinRaw => mir::RuntimeFn::Pin,
            hir::IntrinsicFunctionKind::GcUnpinRaw => mir::RuntimeFn::Unpin,
            hir::IntrinsicFunctionKind::GcGetHandleRaw => mir::RuntimeFn::GetHandle,
            hir::IntrinsicFunctionKind::GcReleaseHandleRaw => mir::RuntimeFn::ReleaseHandle,
            hir::IntrinsicFunctionKind::GcCollect => mir::RuntimeFn::GcCollect,
            hir::IntrinsicFunctionKind::GcStats => mir::RuntimeFn::GcStats,
            hir::IntrinsicFunctionKind::CoroutineStart
            | hir::IntrinsicFunctionKind::CoroutineSuspend => {
                unreachable!("coroutine intrinsics are lowered through the typed protocol")
            }
            hir::IntrinsicFunctionKind::Pointer(_)
            | hir::IntrinsicFunctionKind::ForeignCallbackRegister
            | hir::IntrinsicFunctionKind::ForeignCallbackRetain
            | hir::IntrinsicFunctionKind::ForeignCallbackRelease
            | hir::IntrinsicFunctionKind::ForeignCallbackState
            | hir::IntrinsicFunctionKind::ForeignCallbackFailure => {
                unreachable!("HIR expands this intrinsic before MIR")
            }
        };
        let callee = mir::Callee::Runtime(function);
        let return_ty = self.lower_type(result_ty);
        self.call(callee, &args.iter().collect::<Vec<_>>(), return_ty)
    }

    fn call(
        &mut self,
        callee: mir::Callee,
        args: &[&hir::Expr],
        return_ty: mir::Type,
    ) -> smir::Expr {
        smir::Expr::new(
            return_ty.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee,
                },
                args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                return_ty,
            }),
        )
    }

    /// A resolved method call (impl spec 2.9): the receiver becomes
    /// argument 0 (`this`), and the call kind is annotated from the
    /// receiver's static type — class receiver → `Virtual` (the M6
    /// simplification: class methods always dispatch through the
    /// vtable), interface receiver → `Interface` (the slot is the
    /// method signature's index in the interface declaration), value
    /// type → `Direct`. A method without a vtable slot (generic
    /// methods never enter the vtable) stays `Direct`. The slot is
    /// located by the callee's signature (`signature_key`), so
    /// overloads dispatch to their own slot and overrides hit the
    /// replaced base slot.
    fn lower_method_call(
        &mut self,
        receiver: &hir::Expr,
        callable: hir::Callable,
        args: &[hir::Expr],
        result_ty: hir::TypeId,
    ) -> smir::Expr {
        let module = self.module;
        let function = module.callable_function(callable);
        let f = &module.functions[function];
        let callee = self.instances.get(function).map_or_else(
            || mir::Callee::User(self.function_map[&function]),
            mir::Callee::Monomorphized,
        );
        // The receiver's static type decides the dispatch kind.
        enum Receiver {
            Class(hir::ClassId),
            Interface(hir::TypeId),
            Value,
        }
        let receiver_kind = match &module.types[receiver.ty].kind {
            hir::TypeKind::Class(class) => Receiver::Class(*class),
            hir::TypeKind::Interface(..) => Receiver::Interface(receiver.ty),
            hir::TypeKind::Any => unreachable!("Any has no methods"),
            _ => Receiver::Value,
        };
        let generic_static_method = is_generic_method(f);
        let kind = if generic_static_method {
            // Generic member functions never participate in virtual
            // dispatch. Methods parameterized only by a generic interface
            // host are different: they still dispatch through that concrete
            // interface application's itable.
            mir::CallKind::Direct
        } else {
            match receiver_kind {
                Receiver::Class(_)
                    if f.method
                        .is_some_and(|method| method.modifier == hir::MethodModifier::Final) =>
                {
                    mir::CallKind::Direct
                }
                Receiver::Class(class) => {
                    let key = self.signature_key(f);
                    match self.method_slots[&self.class_map[&class]].get(&key) {
                        Some(&slot) => mir::CallKind::Virtual { slot },
                        None => mir::CallKind::Direct,
                    }
                }
                Receiver::Interface(interface_ty) => {
                    let mir::Type::Interface(interface) = self.lower_type(interface_ty) else {
                        unreachable!()
                    };
                    let (iface, _) = self.interfaces.source(interface);
                    let key = self.signature_key(f);
                    let mut slot = None;
                    for (index, sig) in module.interfaces[iface].methods.iter().enumerate() {
                        if self.sig_key(sig) == key {
                            slot = Some(index as u32);
                            break;
                        }
                    }
                    mir::CallKind::Interface {
                        interface,
                        slot: slot
                            .expect("hir-lower resolves interface calls to interface methods"),
                    }
                }
                Receiver::Value => mir::CallKind::Direct,
            }
        };
        let mut call_args = Vec::with_capacity(args.len() + 1);
        call_args.push(self.lower_expr(receiver));
        call_args.extend(args.iter().map(|arg| self.lower_expr(arg)));
        let return_ty = self.lower_type(result_ty);
        smir::Expr::new(
            return_ty.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget { kind, callee },
                args: call_args,
                return_ty,
            }),
        )
    }

    /// The callee's dispatch signature key (`name(<param encoding>)`,
    /// receiver excluded) — must agree with
    /// `Lowerer::fn_signature_key`, which keys the vtable slots.
    fn signature_key(&mut self, function: &hir::Function) -> String {
        let skip = usize::from(function.method.is_some());
        self.key_parts(short_name(&function.name), &function.params[skip..])
    }

    fn sig_key(&mut self, sig: &hir::MethodSig) -> String {
        self.key_parts(&sig.name, &sig.params)
    }

    fn key_parts(&mut self, name: &str, params: &[hir::Param]) -> String {
        let params: Vec<mir::Type> = params
            .iter()
            .map(|param| self.lower_type(param.ty))
            .collect();
        format!("{name}({})", mir::encode_params(self.shell, &params))
    }

    /// Register the boxed value type a `Box` produces. The boxed
    /// itables cover the value type's *declared* interfaces (spec
    /// 4.4.3) no matter what the value is boxed to; the box target,
    /// when an interface, is covered too (hir-lower guarantees the
    /// value type implements it, so it is normally already in the
    /// declared set). The itable slots — the adjust thunks — are
    /// generated by `finalize_boxed`.
    fn register_boxed(&mut self, payload: &mir::Type, target: Option<hir::TypeId>) {
        if !is_boxable(payload) {
            return;
        }
        let class_id = self.boxed.get_or_create(self.classes, self.shell, payload);
        let declared: Vec<hir::TypeId> = match payload {
            mir::Type::Struct(mir_id) => {
                let hir_id = self.hir_struct(*mir_id);
                self.module.structs[hir_id].interfaces.clone()
            }
            mir::Type::Enum(mir_id, _) => {
                let hir_id = self.enums.hir_ids[mir_id];
                self.module.enums[hir_id].interfaces.clone()
            }
            // Tuples and primitives implement no interfaces.
            _ => Vec::new(),
        };
        let types = Types {
            module: self.module,
            struct_map: self.struct_map,
            class_map: self.class_map,
        };
        let mut covered = Vec::new();
        for interface_ty in declared {
            let lowered = types.lower(
                interface_ty,
                self.enums,
                self.structs,
                self.interfaces,
                self.shell,
            );
            let mir::Type::Interface(interface) = lowered else {
                unreachable!()
            };
            covered.push(interface);
        }
        if let Some(target) = target {
            if matches!(self.module.types[target].kind, hir::TypeKind::Interface(..)) {
                let mir::Type::Interface(interface) = self.lower_type(target) else {
                    unreachable!()
                };
                covered.push(interface);
            }
        }
        for iface in covered {
            let interfaces = &mut self.classes[class_id].interfaces;
            if !interfaces.contains(&iface) {
                interfaces.push(iface);
            }
        }
    }

    /// The HIR declaration behind a plain or instantiated MIR struct.
    fn hir_struct(&self, mir_id: mir::StructId) -> hir::StructId {
        if let Some((hir_id, _)) = self.structs.instances.get(&mir_id) {
            return *hir_id;
        }
        self.struct_map
            .iter()
            .find(|(_, mir)| **mir == mir_id)
            .map(|(&hir, _)| hir)
            .expect("every MIR struct comes from a HIR struct")
    }

    /// Register the boxed value type an `is` / `as` check needs (the
    /// runtime compares against the boxed type's TypeDescriptor).
    fn register_check(&mut self, check_ty: &mir::Type) {
        if is_boxable(check_ty) {
            let check_ty = check_ty.clone();
            self.register_boxed(&check_ty, None);
        } else if let mir::Type::Function(function_type) = check_ty
            && !self.function_bridge_targets.contains(function_type)
        {
            self.function_bridge_targets.push(*function_type);
        }
    }

    /// `as` / `as?` (DESIGN 2.3): the operand is evaluated once into
    /// a hidden local. `as` throws `ClassCastException` when the
    /// runtime check fails (M8); `as?` wraps the result
    /// in `Some` / `None` through core's `Option` — the same prelude
    /// mechanism `!!` uses. A target of `Any` is statically true and
    /// needs no check. Class / interface targets stay the same
    /// reference; value targets come out of the box (`Unbox`).
    fn lower_cast(
        &mut self,
        operand: &hir::Expr,
        optional: bool,
        expr_ty: hir::TypeId,
        span: Span,
    ) -> smir::Expr {
        let target_hir = if optional {
            let hir::TypeKind::Enum(option) = self.module.types[expr_ty].kind else {
                unreachable!("an `as?` result is an Option<T>")
            };
            let (some, _) = self.module.enums[option]
                .option_variants
                .expect("an `as?` result is core's Option<T>");
            self.module.enums[option].variants[some.into_raw() as usize].fields[0].ty
        } else {
            expr_ty
        };
        let target = self.lower_type(target_hir);
        self.register_check(&target);
        let operand_ty = self.lower_type(operand.ty);
        let value = self.lower_expr(operand);
        let slot = self.new_hidden("cast", operand_ty.clone(), false);
        self.prelude.push(smir::StatementKind::ValDecl {
            local: slot,
            init: value,
        });
        let cond = match &target {
            mir::Type::Any => smir::Expr::bool(true),
            _ => smir::Expr::new(
                mir::Type::Boolean,
                smir::ExprKind::IsInstance {
                    operand: Box::new(smir::Expr::local(slot, operand_ty.clone())),
                    check_ty: Box::new(target.clone()),
                },
            ),
        };
        if !optional {
            let throw = self.throw_builtin(self.module.exception_core.class_cast_exception, span);
            self.prelude.push(smir::StatementKind::If {
                cond: smir::Expr::new(
                    mir::Type::Boolean,
                    smir::ExprKind::Unary {
                        op: mir::UnOp::BoolNot,
                        operand: Box::new(cond),
                    },
                ),
                then_body: vec![throw],
                else_body: None,
            });
            // A checked function view needs an exact target-ABI closure;
            // its invoke dispatches through the source closure's bridge
            // table. Value targets are unboxed by the outer HIR node.
            return match target {
                mir::Type::Function(function_type) => self.adapt_checked_function_value(
                    smir::Expr::local(slot, operand_ty),
                    function_type,
                    span,
                ),
                target @ (mir::Type::String
                | mir::Type::Class(_)
                | mir::Type::Interface(_)
                | mir::Type::Any) => smir::Expr::new(
                    target.clone(),
                    smir::ExprKind::Retype {
                        operand: Box::new(smir::Expr::local(slot, operand_ty)),
                        ty: Box::new(target),
                    },
                ),
                // hir-lower wraps a checked value-type cast in an outer
                // `Unbox`. Preserve the checked boxed/reference operand here;
                // the outer node is the sole operation that produces the
                // target value type.
                _ => smir::Expr::local(slot, operand_ty),
            };
        }
        let unboxed = match &target {
            mir::Type::Function(function_type) => self.adapt_checked_function_value(
                smir::Expr::local(slot, operand_ty.clone()),
                *function_type,
                span,
            ),
            mir::Type::Class(_) | mir::Type::Interface(_) | mir::Type::Any => smir::Expr::new(
                target.clone(),
                smir::ExprKind::Retype {
                    operand: Box::new(smir::Expr::local(slot, operand_ty.clone())),
                    ty: Box::new(target.clone()),
                },
            ),
            // Only `as?` unwraps here: hir-lower wraps a value-typed
            // `as` in a hir-level `Unbox(Cast)` node, so the payload
            // extraction for `as` happens when that outer `Unbox` is
            // lowered — adding another one here would double-unwrap.
            _ => smir::Expr::new(
                target.clone(),
                smir::ExprKind::Unbox(Box::new(smir::Expr::local(slot, operand_ty))),
            ),
        };
        let option_ty = self.lower_type(expr_ty);
        let (some, none) = self.option_variants;
        let result = self.new_hidden("cast", option_ty.clone(), true);
        let some_value = smir::Expr::new(
            option_ty.clone(),
            smir::ExprKind::VariantConstruct {
                variant: some,
                fields: vec![unboxed],
            },
        );
        let none_value = smir::Expr::new(
            option_ty.clone(),
            smir::ExprKind::VariantConstruct {
                variant: none,
                fields: Vec::new(),
            },
        );
        self.prelude.push(smir::StatementKind::If {
            cond,
            then_body: vec![smir::Statement {
                kind: smir::StatementKind::Assign {
                    local: result,
                    value: some_value,
                },
                span,
            }],
            else_body: Some(vec![smir::Statement {
                kind: smir::StatementKind::Assign {
                    local: result,
                    value: none_value,
                },
                span,
            }]),
        });
        smir::Expr::local(result, option_ty)
    }

    /// A trap message string constant (`scoop.str.N`, numbered in
    /// order of appearance like the literal constants).
    fn trap_message(&mut self, message: String) -> mir::StringConstId {
        let symbol = format!("scoop.str.{}", self.strings.len());
        self.strings.alloc(mir::StringConst {
            value: message,
            symbol,
        })
    }

    fn lower_binary(
        &mut self,
        op: hir::BinOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
        span: Span,
    ) -> smir::Expr {
        use mir::BinOp::*;
        match op {
            // String `+` is runtime concatenation (DESIGN 2.3); hir-lower
            // type checking makes both operands String here.
            hir::BinOp::Add if matches!(self.module.types[lhs.ty].kind, hir::TypeKind::String) => {
                self.call(
                    mir::Callee::Runtime(mir::RuntimeFn::StringConcat),
                    &[lhs, rhs],
                    mir::Type::String,
                )
            }
            hir::BinOp::Add => self.primitive(IntAdd, lhs, rhs),
            hir::BinOp::Sub => self.primitive(IntSub, lhs, rhs),
            hir::BinOp::Mul => self.primitive(IntMul, lhs, rhs),
            // M8 (DESIGN section 1): integer division checks the
            // divisor — a zero divisor throws `ArithmeticException`
            // instead of hitting LLVM `sdiv` UB. Both operands are
            // evaluated once into hidden locals (left to right), so
            // the check and the division share one evaluation.
            hir::BinOp::Div => {
                let lhs_slot = self.new_hidden("div", mir::Type::Int, false);
                let rhs_slot = self.new_hidden("div", mir::Type::Int, false);
                let lhs = self.lower_expr(lhs);
                self.prelude.push(smir::StatementKind::ValDecl {
                    local: lhs_slot,
                    init: lhs,
                });
                let rhs = self.lower_expr(rhs);
                self.prelude.push(smir::StatementKind::ValDecl {
                    local: rhs_slot,
                    init: rhs,
                });
                let throw =
                    self.throw_builtin(self.module.exception_core.arithmetic_exception, span);
                self.prelude.push(smir::StatementKind::If {
                    cond: smir::Expr::new(
                        mir::Type::Boolean,
                        smir::ExprKind::Binary {
                            op: mir::BinOp::IntEq,
                            lhs: Box::new(smir::Expr::local(rhs_slot, mir::Type::Int)),
                            rhs: Box::new(smir::Expr::int(0)),
                        },
                    ),
                    then_body: vec![throw],
                    else_body: None,
                });
                smir::Expr::new(
                    mir::Type::Int,
                    smir::ExprKind::Binary {
                        op: IntDiv,
                        lhs: Box::new(smir::Expr::local(lhs_slot, mir::Type::Int)),
                        rhs: Box::new(smir::Expr::local(rhs_slot, mir::Type::Int)),
                    },
                )
            }
            hir::BinOp::Lt => self.primitive(IntLt, lhs, rhs),
            hir::BinOp::Le => self.primitive(IntLe, lhs, rhs),
            hir::BinOp::Gt => self.primitive(IntGt, lhs, rhs),
            hir::BinOp::Ge => self.primitive(IntGe, lhs, rhs),
            hir::BinOp::Eq | hir::BinOp::Ne => {
                unreachable!("HIR resolves == and != to exact method calls")
            }
            // `===` / `!==`: reference identity — the primitive
            // comparison on the two pointers.
            hir::BinOp::RefEq => self.primitive(IntEq, lhs, rhs),
            hir::BinOp::RefNe => self.primitive(IntNe, lhs, rhs),
            // Short-circuit operators stay in the private construction
            // tree until CFG normalization emits their branch edges.
            hir::BinOp::And => self.short_circuit(smir::LogicOp::And, lhs, rhs),
            hir::BinOp::Or => self.short_circuit(smir::LogicOp::Or, lhs, rhs),
        }
    }

    fn primitive(&mut self, op: mir::BinOp, lhs: &hir::Expr, rhs: &hir::Expr) -> smir::Expr {
        let ty = match op {
            mir::BinOp::IntLt
            | mir::BinOp::IntLe
            | mir::BinOp::IntGt
            | mir::BinOp::IntGe
            | mir::BinOp::IntEq
            | mir::BinOp::IntNe
            | mir::BinOp::BoolEq
            | mir::BinOp::BoolNe => mir::Type::Boolean,
            mir::BinOp::IntAdd | mir::BinOp::IntSub | mir::BinOp::IntMul | mir::BinOp::IntDiv => {
                self.lower_type(lhs.ty)
            }
        };
        let lhs = Box::new(self.lower_expr(lhs));
        let rhs = Box::new(self.lower_expr(rhs));
        smir::Expr::new(ty, smir::ExprKind::Binary { op, lhs, rhs })
    }

    fn short_circuit(&mut self, op: smir::LogicOp, lhs: &hir::Expr, rhs: &hir::Expr) -> smir::Expr {
        let lhs = self.lower_expr(lhs);
        let rhs = self.lower_expr(rhs);
        logic(op, lhs, rhs)
    }

    /// Produce a pattern subject's nested value. Variant field extractions are
    /// guarded by the decision sequence that proved the active tag.
    fn accessed(&mut self, root: mir::LocalId, path: &[Access]) -> smir::Expr {
        let mut current_ty = self.locals[root].ty.clone();
        let mut lowered = smir::Expr::local(root, current_ty.clone());
        for access in path {
            let (next_ty, kind) = match access {
                Access::Field(index) => {
                    let next_ty = match &current_ty {
                        mir::Type::Tuple(elements) => elements[*index as usize].clone(),
                        mir::Type::Struct(struct_id) => self.structs.defs[*struct_id]
                            .declared_fields()[*index as usize]
                            .ty
                            .clone(),
                        _ => unreachable!("tuple/struct patterns only access aggregate fields"),
                    };
                    let kind = smir::ExprKind::FieldAccess {
                        receiver: Box::new(lowered),
                        index: *index,
                    };
                    (next_ty, kind)
                }
                Access::EnumField { variant, index } => {
                    let mir::Type::Enum(enum_id, _) = current_ty else {
                        unreachable!("variant pattern field access has an enum receiver")
                    };
                    let next_ty = self.enums.defs[enum_id].variants[*variant as usize].fields
                        [*index as usize]
                        .ty
                        .clone();
                    let kind = smir::ExprKind::EnumField {
                        operand: Box::new(lowered),
                        variant: *variant,
                        index: *index,
                    };
                    (next_ty, kind)
                }
            };
            current_ty = next_ty.clone();
            lowered = smir::Expr::new(next_ty, kind);
        }
        lowered
    }
}

/// Combine two conditions with `&&`; CFG normalization expands the short circuit.
fn and(lhs: smir::Expr, rhs: smir::Expr) -> smir::Expr {
    logic(smir::LogicOp::And, lhs, rhs)
}

fn logic(op: smir::LogicOp, lhs: smir::Expr, rhs: smir::Expr) -> smir::Expr {
    smir::Expr::new(
        mir::Type::Boolean,
        smir::ExprKind::ShortCircuit {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
    )
}

/// `Some(statements)` unless empty (an absent else branch).
fn non_empty(statements: Vec<smir::Statement>) -> Option<Vec<smir::Statement>> {
    if statements.is_empty() {
        None
    } else {
        Some(statements)
    }
}
