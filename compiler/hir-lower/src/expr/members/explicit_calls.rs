use super::*;

impl Lowerer {
    pub(super) fn lower_explicit_named_call(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
        direct_required: RequiredCallableModifiers,
    ) -> Option<hir::Expr> {
        if name.text == "invoke" && matches!(self.types[receiver.ty], Type::Function(_)) {
            return self.lower_named_call_on_receiver(
                receiver,
                name,
                call,
                sink,
                expected,
                direct_required,
            );
        }
        if name.text == "invoke" && matches!(self.types[receiver.ty], Type::FunPtr(_)) {
            self.error(
                call.span,
                "FunPtr values are not callable in Scoop".to_string(),
            );
            return None;
        }
        let mut first_failure = None;
        let member_property = self
            .find_accessible_nominal_property(receiver.ty, &name.text)
            .map(|(property, _, _)| property);
        let property = match self
            .probe_expr_layer(|state, _| state.member_property_read(receiver.clone(), name))
        {
            Ok(layer) => Some(layer),
            Err(failure) if failure.diagnostics.len() > self.diagnostics.len() => {
                first_failure = Some(failure);
                None
            }
            Err(_) => None,
        };
        let mut members = self.methods_by_name(receiver.ty, &name.text);
        members.retain(|candidate| {
            Self::matches_required_modifiers(
                self.signatures[&candidate.function].modifiers,
                direct_required,
            )
        });
        if !members.is_empty() && matches!(self.types[receiver.ty], Type::Interface(..)) {
            let before = members.len();
            members.retain(|candidate| {
                let signature = &self.signatures[&candidate.function];
                signature.type_params.len() == signature.owner_type_param_count
            });
            if members.is_empty() && before != 0 {
                let found = self.type_name(receiver.ty);
                let mut failure = self.clone();
                failure.error(
                    name.span,
                    format!(
                        "generic member function `{}` cannot be called through interface type `{found}`",
                        name.text
                    ),
                );
                first_failure = Some(Box::new(failure));
            }
        }
        if !members.is_empty() {
            match self.probe_member_call_partition(
                members,
                &name.text,
                receiver.clone(),
                call,
                expected,
                false,
            ) {
                PropertyExtensionInvokeOutcome::Resolved(layer) => {
                    return Some(self.commit_expr_layer(layer, sink));
                }
                PropertyExtensionInvokeOutcome::Blocked => return None,
                PropertyExtensionInvokeOutcome::Failed(failure) => {
                    self.commit_layer_diagnostics(*failure);
                    return None;
                }
                PropertyExtensionInvokeOutcome::NoApplicable(failure) => {
                    first_failure = failure;
                }
            }
        }

        if let Some(property) = &property {
            match property.state.probe_property_member_invoke_partition(
                property.expression.clone(),
                call,
                expected,
                direct_required.infix,
            ) {
                PropertyExtensionInvokeOutcome::Resolved(mut layer) => {
                    let mut setup = property.sink.clone();
                    setup.append(&mut layer.sink);
                    layer.sink = setup;
                    return Some(self.commit_expr_layer(layer, sink));
                }
                PropertyExtensionInvokeOutcome::Blocked => return None,
                PropertyExtensionInvokeOutcome::Failed(failure) => {
                    self.commit_layer_diagnostics(*failure);
                    return None;
                }
                PropertyExtensionInvokeOutcome::NoApplicable(failure) => {
                    if let Some(failure) = failure {
                        first_failure.get_or_insert(failure);
                    }
                }
            }
        }

        let mut extension_layers = match direct_required.operator {
            Some(operator) => self.named_executable_extension_operator_layers(operator),
            None => self.named_executable_extension_call_layers(&name.text),
        };
        let mut invoke_layers = self.named_executable_extension_call_layers("invoke");
        let mut extension_property_layers = self.named_extension_property_layers(&name.text);
        Self::retain_highest_rank_origins(&mut extension_layers);
        Self::retain_highest_rank_origins(&mut invoke_layers);
        Self::retain_highest_rank_origins(&mut extension_property_layers);
        for rank in 0..=3 {
            let suppressed_extensions =
                Self::suppressed_extensions_at_rank(&extension_layers, rank, |function| {
                    Self::matches_required_modifiers(
                        self.signatures[&function].modifiers,
                        direct_required,
                    )
                });
            let suppressed_invokes =
                Self::suppressed_extensions_at_rank(&invoke_layers, rank, |function| {
                    Self::matches_required_modifiers(
                        self.signatures[&function].modifiers,
                        RequiredCallableModifiers {
                            operator: Some(hir::OperatorKind::Invoke),
                            infix: direct_required.infix,
                            ..Default::default()
                        },
                    )
                });
            let extensions =
                Self::extension_candidates_at_rank(&extension_layers, rank, |target| {
                    self.extension_call_target_matches_required(target, direct_required)
                });
            if !extensions.is_empty() {
                match self.probe_extension_call_partition(
                    &extensions,
                    name,
                    receiver.clone(),
                    call,
                    expected,
                    false,
                    "extension candidate",
                ) {
                    PropertyExtensionInvokeOutcome::Resolved(layer) => {
                        return Some(self.commit_expr_layer(layer, sink));
                    }
                    PropertyExtensionInvokeOutcome::Blocked => return None,
                    PropertyExtensionInvokeOutcome::Failed(failure) => {
                        self.commit_layer_diagnostics(*failure);
                        return None;
                    }
                    PropertyExtensionInvokeOutcome::NoApplicable(failure) => {
                        if let Some(failure) = failure {
                            first_failure.get_or_insert(failure);
                        }
                    }
                }
            }

            let invokes = Self::extension_candidates_at_rank(&invoke_layers, rank, |target| {
                self.extension_call_target_matches_required(
                    target,
                    RequiredCallableModifiers {
                        operator: Some(hir::OperatorKind::Invoke),
                        infix: direct_required.infix,
                        ..Default::default()
                    },
                )
            });
            if let Some(property) = &property
                && !invokes.is_empty()
            {
                match self.probe_property_extension_invoke_partition(
                    vec![PropertyExtensionInvokeInput::new(
                        PropertyExtensionInvokeOrigin::Member(
                            member_property.expect("a successful member property read has an id"),
                        ),
                        property.clone(),
                        invokes,
                    )],
                    call,
                    expected,
                    Self::extension_layer_name(rank),
                ) {
                    PropertyExtensionInvokeOutcome::Resolved(layer) => {
                        return Some(self.commit_expr_layer(layer, sink));
                    }
                    PropertyExtensionInvokeOutcome::Blocked => return None,
                    PropertyExtensionInvokeOutcome::Failed(failure) => {
                        self.commit_layer_diagnostics(*failure);
                        return None;
                    }
                    PropertyExtensionInvokeOutcome::NoApplicable(failure) => {
                        if let Some(failure) = failure {
                            first_failure.get_or_insert(failure);
                        }
                    }
                }
            }

            let property_candidates =
                Self::property_candidates_at_rank(&extension_property_layers, rank);
            if !property_candidates.is_empty() {
                let mut extension_property_state = self.clone();
                let mut extension_property_sink = Vec::new();
                match extension_property_state.resolve_extension_property_candidates_outcome(
                    receiver.clone(),
                    name,
                    &property_candidates,
                    &mut extension_property_sink,
                ) {
                    crate::properties::ExtensionPropertyCandidateOutcome::Resolved(property) => {
                        match extension_property_state.probe_property_member_invoke_partition(
                            property.read,
                            call,
                            expected,
                            direct_required.infix,
                        ) {
                            PropertyExtensionInvokeOutcome::Resolved(mut layer) => {
                                extension_property_sink.append(&mut layer.sink);
                                layer.sink = extension_property_sink;
                                return Some(self.commit_expr_layer(layer, sink));
                            }
                            PropertyExtensionInvokeOutcome::Blocked => return None,
                            PropertyExtensionInvokeOutcome::Failed(failure) => {
                                self.commit_layer_diagnostics(*failure);
                                return None;
                            }
                            PropertyExtensionInvokeOutcome::NoApplicable(failure) => {
                                if let Some(failure) = failure {
                                    first_failure.get_or_insert(failure);
                                }
                            }
                        }
                    }
                    crate::properties::ExtensionPropertyCandidateOutcome::Failed => {
                        self.commit_layer_diagnostics(extension_property_state);
                        return None;
                    }
                    crate::properties::ExtensionPropertyCandidateOutcome::NoApplicable => {
                        if extension_property_state.diagnostics.len() > self.diagnostics.len() {
                            first_failure.get_or_insert(Box::new(extension_property_state));
                        }
                    }
                    crate::properties::ExtensionPropertyCandidateOutcome::NoCandidate => {}
                }
            }

            let mut property_extension_inputs = Vec::new();
            for property_layer in &extension_property_layers {
                for invoke_layer in &invoke_layers {
                    if property_layer
                        .kind
                        .call_rank()
                        .max(invoke_layer.kind.call_rank())
                        != rank
                    {
                        continue;
                    }
                    let invokes = invoke_layer
                        .candidates
                        .iter()
                        .filter(|target| {
                            self.extension_call_target_matches_required(
                                target,
                                RequiredCallableModifiers {
                                    operator: Some(hir::OperatorKind::Invoke),
                                    infix: direct_required.infix,
                                    ..Default::default()
                                },
                            )
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    if property_layer.candidates.is_empty() || invokes.is_empty() {
                        continue;
                    }
                    let mut property_state = self.clone();
                    let mut property_sink = Vec::new();
                    match property_state.resolve_extension_property_candidates_outcome(
                        receiver.clone(),
                        name,
                        &property_layer.candidates,
                        &mut property_sink,
                    ) {
                        crate::properties::ExtensionPropertyCandidateOutcome::Resolved(
                            property,
                        ) => {
                            property_extension_inputs.push(PropertyExtensionInvokeInput::new(
                                PropertyExtensionInvokeOrigin::Extension(property.identity()),
                                SuccessfulExprLayer {
                                    state: Box::new(property_state),
                                    expression: property.read,
                                    sink: property_sink,
                                },
                                invokes,
                            ));
                        }
                        crate::properties::ExtensionPropertyCandidateOutcome::Failed => {
                            self.commit_layer_diagnostics(property_state);
                            return None;
                        }
                        crate::properties::ExtensionPropertyCandidateOutcome::NoApplicable => {
                            if property_state.diagnostics.len() > self.diagnostics.len() {
                                first_failure.get_or_insert(Box::new(property_state));
                            }
                        }
                        crate::properties::ExtensionPropertyCandidateOutcome::NoCandidate => {}
                    }
                }
            }
            if !property_extension_inputs.is_empty() {
                match self.probe_property_extension_invoke_partition(
                    property_extension_inputs,
                    call,
                    expected,
                    Self::extension_layer_name(rank),
                ) {
                    PropertyExtensionInvokeOutcome::Resolved(layer) => {
                        return Some(self.commit_expr_layer(layer, sink));
                    }
                    PropertyExtensionInvokeOutcome::Blocked => return None,
                    PropertyExtensionInvokeOutcome::Failed(failure) => {
                        self.commit_layer_diagnostics(*failure);
                        return None;
                    }
                    PropertyExtensionInvokeOutcome::NoApplicable(failure) => {
                        if let Some(failure) = failure {
                            first_failure.get_or_insert(failure);
                        }
                    }
                }
            }
            if suppressed_extensions || suppressed_invokes {
                if let Some(failure) = first_failure {
                    self.commit_layer_diagnostics(*failure);
                }
                return None;
            }
        }

        if let Some(failure) = first_failure {
            self.commit_layer_diagnostics(*failure);
        } else if let Some(message) = self.inaccessible_method_message(receiver.ty, &name.text) {
            self.error(name.span, message);
        } else {
            let found = self.type_name(receiver.ty);
            let capability = if direct_required.infix {
                "infix callable"
            } else {
                "method"
            };
            self.error(
                name.span,
                format!("type `{found}` has no {capability} `{}`", name.text),
            );
        }
        None
    }

    fn extension_candidates_at_rank<T: Clone>(
        layers: &[crate::imports::lookup::LookupLayer<T>],
        rank: usize,
        predicate: impl Fn(&T) -> bool,
    ) -> Vec<T> {
        layers
            .iter()
            .filter(|layer| layer.kind.call_rank() == rank)
            .flat_map(|layer| layer.candidates.iter())
            .filter(|candidate| predicate(candidate))
            .cloned()
            .collect()
    }

    fn suppressed_extensions_at_rank<T>(
        layers: &[crate::imports::lookup::LookupLayer<T>],
        rank: usize,
        predicate: impl Fn(hir::FunctionId) -> bool,
    ) -> bool {
        layers
            .iter()
            .filter(|layer| layer.kind.call_rank() == rank)
            .flat_map(|layer| layer.suppressed_callables.iter().copied())
            .any(predicate)
    }

    fn property_candidates_at_rank(
        layers: &[crate::imports::lookup::LookupLayer<
            crate::imports::lookup::calls::ExtensionPropertyTarget,
        >],
        rank: usize,
    ) -> Vec<crate::imports::lookup::calls::ExtensionPropertyTarget> {
        layers
            .iter()
            .filter(|layer| layer.kind.call_rank() == rank)
            .flat_map(|layer| layer.candidates.iter().cloned())
            .collect()
    }

    fn retain_highest_rank_origins<T: Clone + PartialEq>(
        layers: &mut [crate::imports::lookup::LookupLayer<T>],
    ) {
        let mut seen = Vec::new();
        for layer in layers {
            layer.candidates.retain(|candidate| {
                if seen.contains(candidate) {
                    false
                } else {
                    seen.push(candidate.clone());
                    true
                }
            });
        }
    }

    fn extension_layer_name(rank: usize) -> &'static str {
        match rank {
            0 => "exact import",
            1 => "current package",
            2 => "star import",
            3 => "core prelude",
            _ => unreachable!("extension layer ranks are a closed four-layer set"),
        }
    }
}
