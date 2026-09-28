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
}
