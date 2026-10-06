use super::*;
use crate::call_resolution::named::NamedFunctionLikeProbe;
use crate::overload::{CallArgumentProtocol, NamedCallReceiver, OverloadCall};

enum ExtensionPropertyCommit {
    Current,
    Dependency,
}

impl Lowerer {
    pub(super) fn select_extension_property_candidates(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        properties: &[ExtensionPropertyTarget],
        sink: &mut Vec<hir::Statement>,
        access: ExtensionPropertyAccess,
    ) -> ExtensionPropertySelectionOutcome {
        if properties.is_empty() {
            return ExtensionPropertySelectionOutcome::NoCandidate;
        }
        let no_type_args = [];
        let static_receiver_type = receiver.ty;
        let no_arguments = [];
        let call = OverloadCall {
            explicit_type_args: &no_type_args,
            arg_exprs: &no_arguments,
            span: name.span,
            expected_result: None,
            argument_protocol: CallArgumentProtocol::Ordinary,
        };
        let mut probes = Vec::new();
        let mut commits = Vec::new();
        let mut first_failure = None;
        let mut suppressed = false;
        for property in properties {
            match property {
                ExtensionPropertyTarget::Current(property) => {
                    if !self.property_is_accessible(*property, Some(receiver.ty)) {
                        continue;
                    }
                    let function = self.extension_property_getter(*property);
                    if self.declaration_surface.rejects_function(function) {
                        suppressed = true;
                        continue;
                    }
                    let candidate = crate::CallableCandidate::function(function, Vec::new());
                    match self.probe_named_callable(
                        &name.text,
                        candidate,
                        NamedCallReceiver::Extension(receiver.clone()),
                        call,
                    ) {
                        Ok(probe) => {
                            probes.push(NamedFunctionLikeProbe::Callable(Box::new(probe)));
                            commits.push(ExtensionPropertyCommit::Current);
                        }
                        Err(failure) => {
                            first_failure.get_or_insert(failure);
                        }
                    }
                }
                ExtensionPropertyTarget::Dependency(binding) => {
                    match self.probe_imported_dependency_extension_property(
                        binding,
                        receiver.clone(),
                        name,
                        matches!(access, ExtensionPropertyAccess::Read),
                    ) {
                        Ok(probe) => {
                            probes.push(NamedFunctionLikeProbe::ImportedDependencyProperty(
                                Box::new(probe),
                            ));
                            commits.push(ExtensionPropertyCommit::Dependency);
                        }
                        Err(failure) => {
                            first_failure.get_or_insert(failure);
                        }
                    }
                }
            }
        }
        if probes.is_empty() {
            if let Some(failure) = first_failure {
                self.commit_layer_diagnostics(*failure);
                return if suppressed {
                    ExtensionPropertySelectionOutcome::Failed
                } else {
                    ExtensionPropertySelectionOutcome::NoApplicable
                };
            }
            return if suppressed {
                ExtensionPropertySelectionOutcome::Failed
            } else {
                ExtensionPropertySelectionOutcome::NoCandidate
            };
        }
        let Some(winner) = self.select_named_function_like(
            &name.text,
            "extension property",
            &probes,
            &no_arguments,
            name.span,
        ) else {
            return ExtensionPropertySelectionOutcome::Failed;
        };
        let probe = probes.swap_remove(winner);
        let commit = commits.swap_remove(winner);
        let (write, current_read) = match (probe, commit) {
            (NamedFunctionLikeProbe::Callable(probe), ExtensionPropertyCommit::Current) => {
                let Some(resolved) = self.commit_named_callable(*probe, sink) else {
                    return ExtensionPropertySelectionOutcome::Failed;
                };
                let function = resolved.function();
                let property = self.extension_property_by_getter[&function];
                let receiver = resolved
                    .args
                    .first()
                    .cloned()
                    .expect("an extension getter materializes its receiver argument");
                let callee = self.materialize_resolved_callee(&resolved);
                let write = ResolvedExtensionPropertyWrite {
                    target: ResolvedExtensionPropertyTarget::Current {
                        property,
                        type_args: resolved.type_args,
                    },
                    receiver,
                    value_type: resolved.return_ty,
                    static_receiver_type,
                    has_setter: self.properties[property].capability.setter().is_some(),
                };
                let read = hir::Expr {
                    kind: hir::ExprKind::Call {
                        binding: None,
                        callee: callee.into(),
                        receiver: resolved.source_receiver,
                        args: resolved.args,
                    },
                    ty: resolved.return_ty,
                    span: name.span,
                    origin: self.expression_origin(name.span),
                };
                (write, Some(read))
            }
            (
                NamedFunctionLikeProbe::ImportedDependencyProperty(probe),
                ExtensionPropertyCommit::Dependency,
            ) => {
                let selected = self.commit_imported_dependency_extension_property(*probe);
                let write = ResolvedExtensionPropertyWrite {
                    target: ResolvedExtensionPropertyTarget::Dependency {
                        target: selected.target,
                        name: name.clone(),
                    },
                    receiver: selected.receiver,
                    static_receiver_type: selected.static_receiver_type,
                    value_type: selected.value_type,
                    has_setter: selected.has_setter,
                };
                (write, None)
            }
            _ => unreachable!("extension property probes retain their typed commit protocol"),
        };
        match access {
            ExtensionPropertyAccess::Write => ExtensionPropertySelectionOutcome::Resolved(
                Box::new(SelectedExtensionProperty::Write(write)),
            ),
            ExtensionPropertyAccess::Read => {
                let read = match current_read {
                    Some(read) => self.finish_current_extension_property_read(&write, read, name),
                    None => {
                        let ResolvedExtensionPropertyTarget::Dependency { target, .. } =
                            &write.target
                        else {
                            unreachable!("a dependency property has no prepared local read")
                        };
                        let Some(read) = self.lower_selected_imported_extension_property_read(
                            target,
                            crate::properties::PropertyCallReceiver {
                                value: write.receiver.clone(),
                                static_type: write.static_receiver_type,
                            },
                            name.span,
                        ) else {
                            return ExtensionPropertySelectionOutcome::Failed;
                        };
                        debug_assert_eq!(read.ty, write.value_type);
                        read
                    }
                };
                ExtensionPropertySelectionOutcome::Resolved(Box::new(
                    SelectedExtensionProperty::Read(ResolvedExtensionPropertyRead { read, write }),
                ))
            }
        }
    }

    fn finish_current_extension_property_read(
        &mut self,
        write: &ResolvedExtensionPropertyWrite,
        read: hir::Expr,
        name: &ast::Ident,
    ) -> hir::Expr {
        let ResolvedExtensionPropertyTarget::Current { property, .. } = &write.target else {
            unreachable!("only a current property carries a prepared local read")
        };
        let callee = match read.kind {
            hir::ExprKind::Call {
                callee: hir::CallableTarget::Local(callee),
                ..
            } => callee,
            _ => unreachable!("a current extension property read is a call"),
        };
        self.check_call_effects(callee, name.span);
        let declaration = self.properties[*property].clone();
        self.record_property_initialization_dependency(&declaration, name.span);
        read
    }

    fn extension_property_getter(&self, property: hir::PropertyId) -> hir::FunctionId {
        match self.property_getters[self.properties[property].capability.getter()].implementation {
            hir::PropertyAccessorImplementation::Body(function) => function,
            hir::PropertyAccessorImplementation::Storage
            | hir::PropertyAccessorImplementation::Constant
            | hir::PropertyAccessorImplementation::StorageBody(_)
            | hir::PropertyAccessorImplementation::AbstractSlot(_) => {
                unreachable!("extension properties have concrete getter bodies")
            }
        }
    }
}
