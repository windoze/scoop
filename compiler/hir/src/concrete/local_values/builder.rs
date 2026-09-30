use super::*;

impl<'a> LocalValueIdentityBuilder<'a> {
    pub(super) fn new(inputs: LocalValueIdentityInputs<'a>) -> Self {
        Self {
            inputs,
            records: BTreeMap::new(),
            definition_origins: BTreeMap::new(),
            locations_by_key: BTreeMap::new(),
            values_by_binding: HashMap::new(),
            capture_aliases: HashMap::new(),
        }
    }

    pub(super) fn build(mut self) -> Result<LocalValueIdentities, LocalValueIdentityError> {
        self.collect_capture_aliases()?;

        let mut function_locals = Vec::with_capacity(self.inputs.functions.len());
        for (function_id, function) in self.inputs.functions.iter() {
            let Some(locals) = function.kind.locals() else {
                function_locals.push(Vec::new());
                continue;
            };
            let mut identities = vec![None; locals.len()];
            for (local_id, local) in locals.iter() {
                if self.capture_aliases.contains_key(&(function_id, local_id)) {
                    continue;
                }
                let location = LocalValueLocation::FunctionLocal {
                    function: raw_arena_index(function_id),
                    local: raw_arena_index(local_id),
                };
                let identity = self.record(
                    function.materialization,
                    local.selector.clone(),
                    &local.definition,
                    location,
                )?;
                self.bind(function.materialization.context(), local.binding, identity);
                identities[arena_index(local_id)] = Some(identity);
            }
            function_locals.push(identities);
        }

        let class_constructors = self.collect_class_constructors()?;
        let struct_constructors = self.collect_struct_constructors()?;
        let callable_references = self.collect_callable_references()?;
        self.collect_default_local_values()?;

        for ((function, local), alias) in &self.capture_aliases {
            let context = self.inputs.functions[*function].materialization.context();
            let Some(identity) = self.find_captured_value(context, alias.binding) else {
                return Err(LocalValueIdentityError::MissingCapturedValue {
                    function: alias.function,
                    capture: alias.capture,
                    binding: alias.binding.into_raw(),
                });
            };
            function_locals[arena_index(*function)][arena_index(*local)] = Some(identity);
        }

        let lambda_captures = self
            .inputs
            .lambdas
            .iter()
            .map(|(lambda, declaration)| {
                self.collect_captures(
                    self.inputs.functions[declaration.function]
                        .materialization
                        .context(),
                    &declaration.captures,
                    CaptureOwnerLocation::Lambda(raw_arena_index(lambda)),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let anonymous_function_captures = self
            .inputs
            .anonymous_functions
            .iter()
            .map(|(function, declaration)| {
                self.collect_captures(
                    self.inputs.functions[declaration.function]
                        .materialization
                        .context(),
                    &declaration.captures,
                    CaptureOwnerLocation::AnonymousFunction(raw_arena_index(function)),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let callable_reference_captures = self
            .inputs
            .callable_references
            .iter()
            .map(|(reference, declaration)| {
                let context = match &declaration.target {
                    CallableReferenceTarget::Local {
                        callee: Callable::Function(function),
                        ..
                    } => self.inputs.functions[*function].materialization.context(),
                    _ => declaration.identity.materialization().context(),
                };
                self.collect_captures(
                    context,
                    &declaration.captures,
                    CaptureOwnerLocation::CallableReference(raw_arena_index(reference)),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let function_locals = function_locals
            .into_iter()
            .enumerate()
            .map(|(function, locals)| {
                locals
                    .into_iter()
                    .enumerate()
                    .map(|(local, identity)| {
                        identity.ok_or(LocalValueIdentityError::MissingFunctionLocal {
                            function: function as u32,
                            local: local as u32,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(LocalValueIdentities {
            records: self
                .records
                .into_values()
                .map(|(record, _)| record)
                .collect(),
            definition_origins: LocalValueDefinitionOrigins {
                records: self.definition_origins.into_values().collect(),
            },
            function_locals,
            lambda_captures,
            anonymous_function_captures,
            callable_reference_captures,
            callable_references,
            class_constructors,
            struct_constructors,
        })
    }
}
