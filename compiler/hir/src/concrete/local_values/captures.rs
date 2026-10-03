use super::*;

impl LocalValueIdentityBuilder<'_> {
    pub(super) fn collect_capture_aliases(&mut self) -> Result<(), LocalValueIdentityError> {
        for (function_id, function) in self.inputs.functions.iter() {
            let raw_function = raw_arena_index(function_id);
            for (capture_index, capture) in function.capture_parameters.iter().enumerate() {
                let FunctionKind::User(body) = &function.kind else {
                    return Err(LocalValueIdentityError::LocalFunctionRequiresBody {
                        function: raw_function,
                    });
                };
                if !function
                    .params
                    .get(capture_index)
                    .is_some_and(|parameter| parameter.local == capture.local)
                    || arena_index(capture.local) >= body.locals.len()
                {
                    return Err(LocalValueIdentityError::MissingCaptureParameter {
                        function: raw_function,
                        capture: capture_index as u32,
                    });
                }
                if self
                    .capture_aliases
                    .insert(
                        (function_id, capture.local),
                        CaptureAlias {
                            function: raw_function,
                            capture: capture_index as u32,
                            binding: capture.binding,
                        },
                    )
                    .is_some()
                {
                    return Err(LocalValueIdentityError::DuplicateCaptureParameter {
                        function: raw_function,
                        local: raw_arena_index(capture.local),
                    });
                }
                let binding = body.locals[capture.local].binding;
                if binding != capture.binding {
                    // Imported capture parameters have their own bindings but
                    // remain aliases when nested callables capture them again.
                    self.values_by_binding
                        .entry(BindingKey {
                            context: function.materialization.context(),
                            binding,
                        })
                        .or_default()
                        .push(LocalValueBinding::Capture(capture.binding));
                }
            }
        }
        Ok(())
    }

    pub(super) fn find_captured_value(
        &self,
        mut context: CallableMaterializationContext,
        mut binding: BindingId,
    ) -> Option<PersistentLocalValueId> {
        let mut visited = HashSet::new();
        loop {
            let key = BindingKey { context, binding };
            if !visited.insert(key) {
                return None;
            }
            if let Some(identities) = self.values_by_binding.get(&key) {
                let mut lexical = identities.iter().filter_map(|value| match value {
                    LocalValueBinding::Lexical(identity) => Some(*identity),
                    LocalValueBinding::Definition(_) | LocalValueBinding::Capture(_) => None,
                });
                if let Some(identity) = lexical.next() {
                    return lexical
                        .all(|candidate| candidate == identity)
                        .then_some(identity);
                }
                match identities.as_slice() {
                    [LocalValueBinding::Definition(identity)] => return Some(*identity),
                    [LocalValueBinding::Capture(source)] => {
                        binding = *source;
                        continue;
                    }
                    [] => unreachable!("a binding index entry is non-empty"),
                    _ => return None,
                }
            }
            let CallableMaterializationContext::Application(application) = context else {
                return None;
            };
            let record = self.inputs.callable_applications.get(application)?;
            context = match record.key().instantiation_owner() {
                CallableInstantiationOwner::EnclosingCallableApplication(parent) => {
                    CallableMaterializationContext::Application(parent)
                }
                CallableInstantiationOwner::EnclosingInitializationApplication(unit) => {
                    CallableMaterializationContext::InitializationApplication(unit)
                }
                // A local function can have its own type arguments while its
                // lexical parent has no substitution context.
                CallableInstantiationOwner::NoOwner => {
                    CallableMaterializationContext::NoSubstitution
                }
                CallableInstantiationOwner::ExactNominalOwner(_) => return None,
            };
        }
    }
}
