use super::*;

impl BodyLowerer<'_> {
    pub(super) fn ensure_foreign_callback_family(
        &mut self,
        callback: mir::StructId,
    ) -> mir::ForeignCallbackFamilyId {
        if let Some(&family) = self.foreign_callback_family_by_callback.get(&callback) {
            return family;
        }

        let types = Types {
            module: self.module,
            struct_map: self.struct_map,
            class_map: self.class_map,
        };
        let core = self.module.foreign_callback_core;
        for enumeration in [
            core.modes.enumeration(),
            core.states.enumeration(),
            core.failure_result.enumeration(),
        ] {
            self.enums.get_or_create(
                &types,
                self.source_exact_types,
                self.structs,
                self.interfaces,
                self.shell,
                enumeration,
            );
        }
        let reusable = self.enums.lower_variant_ref(core.modes.reusable());
        let one_shot = self.enums.lower_variant_ref(core.modes.one_shot());
        let modes = mir::ForeignCallbackModes::checked(&self.enums.defs, reusable, one_shot)
            .expect("the concrete callback mode protocol maps to checked MIR refs");
        let registered = self.enums.lower_variant_ref(core.states.registered());
        let active = self.enums.lower_variant_ref(core.states.active());
        let completed = self.enums.lower_variant_ref(core.states.completed());
        let failed = self.enums.lower_variant_ref(core.states.failed());
        let states = mir::ForeignCallbackStates::checked(
            &self.enums.defs,
            registered,
            active,
            completed,
            failed,
        )
        .expect("the concrete callback state protocol maps to checked MIR refs");
        let failure_some = self
            .enums
            .lower_variant_field_ref(core.failure_result.some_payload());
        let failure_none = self.enums.lower_variant_ref(core.failure_result.none());
        let failure_option = mir::OptionCore::checked(&self.enums.defs, failure_some, failure_none)
            .expect("the concrete callback failure protocol maps to checked MIR refs");
        let failure_result = mir::ForeignCallbackFailureResult::checked(
            &self.enums.defs,
            failure_option,
            self.class_map[&core.failure_result.throwable()],
        )
        .expect("callback failure remains the exact MIR Option<Throwable> specialization");
        let family = self
            .foreign_callback_families
            .alloc(mir::ForeignCallbackFamily {
                callback,
                modes,
                states,
                failure_result,
            });
        self.foreign_callback_family_by_callback
            .insert(callback, family);
        family
    }

    pub(super) fn ensure_callback_bridge(
        &mut self,
        source: hir::FunctionId,
        source_signature: hir::FunctionTypeId,
        span: Span,
    ) -> mir::CallbackBridgeId {
        let source_materialization = self.module.functions[source].materialization;
        let source = self.function_map[&source];
        let signature = self.lower_function_type_id(source_signature);
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
            pending: mir::CoroutinePendingContext::Root,
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
                loop_header_polls: Vec::new(),
            },
        });
        self.top_level.push(bridge_function);
        let exact_signature = exact_callback_signature(self.module, source_signature);
        let odr_group =
            materialization_context_odr_group(self.module, source_materialization.context());
        let callback = self.callback_bridges.alloc(
            mir::CallbackBridge::new(
                source,
                signature,
                bridge_function,
                source_materialization,
                exact_signature,
                odr_group,
            )
            .expect("a static no-GC callback has one persistent storage-bridge identity"),
        );
        self.callback_by_target
            .insert((source, signature), callback);
        callback
    }

    pub(super) fn ensure_foreign_callback_bridge(
        &mut self,
        registration_id: hir::ForeignCallbackRegistrationId,
        span: Span,
    ) -> mir::ForeignCallbackBridgeId {
        let registration = self.module.foreign_callback_registrations[registration_id].clone();
        if let Some(&bridge) = self
            .foreign_callback_by_application
            .get(&registration.application)
        {
            return bridge;
        }
        let native_signature = self.lower_function_type_id(registration.native_function_type);
        let managed_signature = self.lower_function_type_id(registration.managed_function_type);
        let exact_managed_signature =
            exact_callback_signature(self.module, registration.managed_function_type);
        let callback = self.struct_map[&registration.callback];
        let family = self.ensure_foreign_callback_family(callback);
        let mode = self.enums.lower_variant_ref(registration.mode);
        assert!(
            self.foreign_callback_families[family].modes.contains(mode),
            "a callback registration mode belongs to the validated core protocol"
        );
        let callback_mode =
            if registration.mode == self.module.foreign_callback_core.modes.reusable() {
                hir::CallbackMode::Reusable
            } else {
                assert_eq!(
                    registration.mode,
                    self.module.foreign_callback_core.modes.one_shot(),
                    "a callback registration mode belongs to the validated core protocol"
                );
                hir::CallbackMode::OneShot
            };
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
                    offset: Some(Box::new(mir::Expr::machine_scalar(
                        mir::MachineScalarValue::PointerElementOffset(
                            u64::try_from(index).expect("callback argument index fits u64"),
                        ),
                    ))),
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
            pending: mir::CoroutinePendingContext::Root,
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
                        pending: mir::CoroutinePendingContext::Root,
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
                value: Some(mir::Expr::machine_scalar(
                    mir::MachineScalarValue::ForeignCallbackStatus(
                        mir::ForeignCallbackStatus::Threw,
                    ),
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
                value: Some(mir::Expr::machine_scalar(
                    mir::MachineScalarValue::ForeignCallbackStatus(
                        mir::ForeignCallbackStatus::Returned,
                    ),
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
            return_ty: mir::Type::MachineScalar(mir::MachineScalarKind::ForeignCallbackStatus),
            body: mir::Body {
                locals,
                blocks,
                entry,
                loop_header_polls: Vec::new(),
            },
        });
        self.top_level.push(function);
        let generated = hir::PersistentGeneratedCallableId::from_key(
            &hir::GeneratedCallableKey::ForeignCallbackManagedAdapter {
                application: registration.application,
            },
        )
        .expect("a callback adapter generated-callable identity is hashable");
        let odr_member =
            callback_adapter_odr_member(self.module, registration.application, generated);
        let adapter = self.foreign_callback_adapters.alloc(
            mir::ForeignCallbackAdapter::checked(
                function,
                managed_signature,
                &signature,
                registration.application,
                &exact_managed_signature,
                odr_member,
            )
            .expect("a callback adapter generated-callable identity is hashable"),
        );
        let application_identity = self
            .module
            .callback_applications
            .get(registration.application)
            .expect("a concrete callback registration has a persistent application record")
            .clone();
        let application_record = mir::CallbackApplicationRecord::new(
            registration.application,
            self.foreign_callback_adapters[adapter].signature_subject(),
            exact_managed_signature,
            mir::ForeignCallbackStorageAbi::ClosureResultRootsThrowableToU32,
            callback_mode,
        );
        let bridge = self
            .foreign_callback_bridges
            .alloc(mir::ForeignCallbackBridge {
                application_identity,
                application_record,
                adapter,
                family,
                native_signature,
                context_index: registration.context_index,
                mode,
            });
        self.foreign_callback_by_application
            .insert(registration.application, bridge);
        bridge
    }
}

fn callback_adapter_odr_member(
    module: &hir::Module,
    application: hir::PersistentCallbackApplicationId,
    generated: hir::PersistentGeneratedCallableId,
) -> Option<hir::OdrMemberRecord> {
    let application = module
        .callback_applications
        .get(application)
        .expect("a concrete callback registration has a persistent application record");
    let group = materialization_context_odr_group(module, application.key().context())?;
    let key = hir::OdrMemberKey::new(
        group,
        hir::OdrMemberRole::CallableBody,
        hir::OdrMemberDiscriminator::GeneratedCallable(generated),
    )
    .expect("a callback adapter is a callable ODR member");
    Some(
        hir::CborIdentityRecord::from_key(key)
            .expect("a callback adapter ODR member identity is hashable"),
    )
}

fn materialization_context_odr_group(
    module: &hir::Module,
    context: hir::CallableMaterializationContext,
) -> Option<hir::OdrGroupId> {
    match context {
        hir::CallableMaterializationContext::NoSubstitution => None,
        hir::CallableMaterializationContext::Application(application) => Some(
            module
                .callable_applications
                .odr(application)
                .expect("a materialization references a concrete callable application")
                .group(),
        ),
        hir::CallableMaterializationContext::InitializationApplication(unit) => {
            let unit = module
                .initialization_units
                .iter()
                .find_map(|(_, candidate)| (candidate.identity.id() == unit).then_some(candidate))
                .expect("a callback materialization references a concrete initialization unit");
            let hir::InitializationUnitKey::GenericDelegatedExtensionApplication {
                property,
                receiver_arguments,
            } = unit.identity.key()
            else {
                panic!(
                    "an initialization callback materialization belongs to a generic delegated extension"
                )
            };
            Some(
                hir::OdrGroupId::from_key(&hir::SpecializationKey::DelegatedProperty {
                    origin: *property,
                    receiver_arguments: receiver_arguments.clone(),
                })
                .expect("a delegated-property ODR group identity is hashable"),
            )
        }
    }
}

fn exact_callback_signature(
    module: &hir::Module,
    signature: hir::FunctionTypeId,
) -> hir::ExactCallableSignature {
    let signature = &module.function_types[signature];
    assert!(!signature.is_suspend, "a managed callback cannot suspend");
    hir::ExactCallableSignature::new(
        hir::Effect::Ordinary,
        None,
        signature
            .parameter_types
            .iter()
            .map(|parameter| module.exact_type_identities[*parameter].id())
            .collect(),
        module.exact_type_identities[signature.return_type].id(),
    )
}
