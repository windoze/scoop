use scoop_hir as hir;

use super::ImportedDependencyCallProbe;
use super::arguments::ImportedParameterInput;
use crate::Lowerer;

impl Lowerer {
    pub(in crate::expr) fn commit_imported_dependency_callable(
        &mut self,
        probe: ImportedDependencyCallProbe,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let ImportedDependencyCallProbe {
            state,
            candidate,
            receiver,
            source_args,
            mut argument_sinks,
            argument_map,
            default_plan,
            parameter_types,
            result_type,
            call_span,
            ..
        } = probe;
        *self = *state;
        if candidate.interface().effects().safety() == hir::CallableSafetyV1::Unsafe {
            self.require_unsafe_operation(call_span, "calling an unsafe dependency function");
        }

        let receiver = receiver.map(|receiver| {
            self.materialize_temporary(
                "$dependency.receiver".to_string(),
                receiver,
                call_span,
                sink,
            )
        });
        let source_args = source_args
            .into_iter()
            .enumerate()
            .map(|(index, argument)| {
                sink.append(&mut argument_sinks[index]);
                self.materialize_temporary(
                    format!("$dependency.argument.{index}"),
                    argument,
                    call_span,
                    sink,
                )
            })
            .collect::<Vec<_>>();
        let parameter_names = candidate
            .source_interface()
            .expect("callable candidates retain their validated source interface")
            .parameters()
            .parameters()
            .iter()
            .map(|parameter| parameter.name().as_str().to_owned())
            .collect::<Vec<_>>();
        let mut parameter_values = Vec::with_capacity(parameter_types.len());
        for ((input, parameter), name) in argument_map
            .parameters()
            .iter()
            .zip(parameter_types)
            .zip(parameter_names)
        {
            let value = match input {
                ImportedParameterInput::Explicit(source) => {
                    self.adapt_to(source_args[*source].clone(), parameter)
                }
                ImportedParameterInput::Default(template) => {
                    let Some(prepared) = default_plan.get(*template) else {
                        self.error(
                            call_span,
                            format!("dependency default plan is missing template {template:?}"),
                        );
                        return None;
                    };
                    match self.materialize_imported_default(
                        &candidate,
                        prepared,
                        receiver.as_ref(),
                        &parameter_values,
                        call_span,
                        sink,
                    ) {
                        Ok(value) => self.adapt_to(value, parameter),
                        Err(error) => {
                            self.error(
                                call_span,
                                format!("failed to materialize dependency default: {error}"),
                            );
                            return None;
                        }
                    }
                }
                ImportedParameterInput::Vararg => {
                    self.imported_dependency_capability_error(
                        &candidate,
                        argument_map.has_vararg(),
                        "dependency callable",
                        call_span,
                    );
                    return None;
                }
            };
            parameter_values.push(self.materialize_temporary(
                format!("$dependency.parameter.{name}"),
                value,
                call_span,
                sink,
            ));
        }

        let mut args = Vec::with_capacity(parameter_values.len() + usize::from(receiver.is_some()));
        args.extend(receiver);
        args.extend(parameter_values);

        let callee = match self.select_imported_dependency_callable_use(candidate) {
            Ok(callee) => callee,
            Err(error) => {
                self.error(
                    call_span,
                    format!("failed to select imported dependency callable: {error}"),
                );
                return None;
            }
        };
        Some(hir::Expr {
            kind: hir::ExprKind::ImportedDependencyCall { callee, args },
            ty: result_type,
            span: call_span,
            origin: self.expression_origin(call_span),
        })
    }
}
