use super::callbacks::{exact_callback_signature, materialization_context_odr_group};
use super::*;

impl BodyLowerer<'_> {
    pub(crate) fn ensure_callback_bridge(
        &mut self,
        target: hir::CallableTarget,
        signature: hir::FunctionTypeId,
        span: Span,
    ) -> mir::CallbackBridgeId {
        match target {
            hir::CallableTarget::Local(hir::Callable::Function(source)) => {
                self.ensure_local_callback_bridge(source, signature, span)
            }
            hir::CallableTarget::Imported(source) => {
                self.ensure_external_callback_bridge(source, signature)
            }
        }
    }

    fn lower_callback_storage_types(&mut self, signature: hir::FunctionTypeId) {
        self.lower_type(self.module.unit);
        let signature = &self.module.function_types[signature];
        let mut pointees = signature.parameter_types.clone();
        if signature.return_type != self.module.unit {
            pointees.push(signature.return_type);
        }
        for pointee in pointees {
            let pointer = self
                .module
                .types
                .iter()
                .find_map(|(id, ty)| (ty.kind == hir::TypeKind::Ptr(pointee)).then_some(id))
                .expect("callback storage types are complete in concrete HIR");
            self.lower_type(pointer);
        }
    }

    fn ensure_external_callback_bridge(
        &mut self,
        source: hir::ImportedDependencyCallableUseId,
        source_signature: hir::FunctionTypeId,
    ) -> mir::CallbackBridgeId {
        self.lower_callback_storage_types(source_signature);
        let source = self.imported_dependency_callable_map[&source].callable;
        let reference = self.external_callables[source].reference();
        let scoop_identity::StrongCallableDefinitionOwner::Function(function) =
            reference.implementation()
        else {
            unreachable!("native callback sources are parameter-free functions")
        };
        let materialization = hir::CallableMaterialization::new(
            hir::CallableTemplateOwner::Function(function),
            hir::CallableMaterializationContext::NoSubstitution,
        );
        let identity = mir::StaticCallbackBridgeIdentity::new(
            materialization,
            exact_callback_signature(self.module, source_signature),
            self.module.exact_type_identities[self.module.unit].id(),
            None,
        )
        .expect("an external callback has a parameter-free storage bridge");
        if let Some((id, _)) = self
            .callback_bridges
            .iter()
            .find(|(_, bridge)| bridge.identity() == &identity)
        {
            return id;
        }
        let bridge_function = self
            .external_callables
            .iter()
            .find_map(|(id, callable)| {
                (callable.reference().provider() == reference.provider()
                    && callable.reference().implementation()
                        == scoop_identity::StrongCallableDefinitionOwner::GeneratedCallable(
                            identity.callable_record().id(),
                        ))
                .then_some(id)
            })
            .expect("external callback selection includes the defining Cone storage bridge");
        let signature = self.lower_function_type_id(source_signature);
        self.callback_bridges.alloc(mir::CallbackBridge::external(
            source,
            bridge_function,
            signature,
            identity,
        ))
    }

    fn ensure_local_callback_bridge(
        &mut self,
        source: hir::FunctionId,
        source_signature: hir::FunctionTypeId,
        span: Span,
    ) -> mir::CallbackBridgeId {
        self.lower_callback_storage_types(source_signature);
        let source_materialization = self.module.functions[source].materialization;
        let source = self.function_map[&source];
        let signature = self.lower_function_type_id(source_signature);
        if let Some(callback) = self.callback_by_target.get(&(source, signature)) {
            return *callback;
        }
        let span = source_span(span);

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
                self.module.exact_type_identities[self.module.unit].id(),
                odr_group,
            )
            .expect("a static no-GC callback has one persistent storage-bridge identity"),
        );
        self.callback_by_target
            .insert((source, signature), callback);
        callback
    }
}
