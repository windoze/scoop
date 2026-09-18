use scoop_ast as ast;
use scoop_hir as hir;

use super::{ImportedArgumentMap, ImportedDependencyCallProbe};
use crate::Lowerer;
use crate::call_resolution::arguments::ArgumentShapeFailure;

impl Lowerer {
    pub(in super::super) fn probe_imported_dependency_callable(
        &self,
        binding: &hir::DirectImportedTargetBinding,
        call: &ast::CallExpr,
        expected: Option<hir::TypeId>,
    ) -> Result<ImportedDependencyCallProbe, Box<Lowerer>> {
        let mut state = self.clone();
        let candidate = match state
            .dependencies
            .as_ref()
            .expect("ordinary lowering carries a dependency selection plan")
            .callable_candidate(binding)
        {
            Ok(candidate) => candidate,
            Err(error) => {
                state.error(
                    call.callee.span,
                    format!("invalid imported dependency callable: {error}"),
                );
                return Err(Box::new(state));
            }
        };
        let interface = candidate.interface();
        let expected_type_arguments = interface.type_parameters().binders().len();
        if call.type_args.len() != expected_type_arguments {
            state.error(
                call.callee.span,
                format!(
                    "dependency function `{}` expects {expected_type_arguments} type argument(s), found {}",
                    call.callee.text,
                    call.type_args.len()
                ),
            );
            return Err(Box::new(state));
        }
        let Some(source) = candidate.source_interface() else {
            state.error(
                call.callee.span,
                format!(
                    "invalid imported dependency callable `{}`: source interface is missing",
                    call.callee.text
                ),
            );
            return Err(Box::new(state));
        };
        let argument_map =
            match ImportedArgumentMap::source(source.parameters().parameters(), &call.args) {
                Ok(map) => map,
                Err(error) => {
                    state.imported_dependency_shape_error(call, error);
                    return Err(Box::new(state));
                }
            };

        let receiver = match interface.owner() {
            hir::PublicDeclarationOwnerV1::TopLevel => None,
            hir::PublicDeclarationOwnerV1::Extension => {
                let Some(receiver_signature) = interface.receiver() else {
                    state.error(
                        call.callee.span,
                        format!(
                            "invalid imported dependency extension `{}`: receiver type is missing",
                            call.callee.text
                        ),
                    );
                    return Err(Box::new(state));
                };
                let receiver_type = state.imported_signature_type(receiver_signature).ok();
                let Some(receiver) = state.lower_current_this(call.callee.span) else {
                    return Err(Box::new(state));
                };
                if let Some(receiver_type) = receiver_type {
                    if !state.is_subtype(receiver.ty, receiver_type) {
                        return Err(Box::new(state));
                    }
                    Some(state.adapt_to(receiver, receiver_type))
                } else {
                    Some(receiver)
                }
            }
            hir::PublicDeclarationOwnerV1::Nominal(_) => None,
        };
        let parameter_types = interface
            .parameters()
            .parameters()
            .iter()
            .map(|parameter| {
                state
                    .imported_signature_type(parameter.value_type())
                    .unwrap_or(state.any)
            })
            .collect::<Vec<_>>();
        let result_type = state
            .imported_signature_type(interface.result())
            .unwrap_or(state.any);
        if candidate.capability().is_some()
            && let Some(expected) = expected
            && !state.is_subtype(result_type, expected)
        {
            state.error(
                call.span,
                format!(
                    "dependency function `{}` returns {}, which is not compatible with expected {}",
                    call.callee.text,
                    state.type_name(result_type),
                    state.type_name(expected)
                ),
            );
            return Err(Box::new(state));
        }

        let mut source_args = Vec::with_capacity(call.args.len());
        let mut argument_sinks = Vec::with_capacity(call.args.len());
        let mut integer_arguments = Vec::with_capacity(call.args.len());
        let mut source_parameter_types = Vec::with_capacity(call.args.len());
        for (argument, signature) in call.args.iter().zip(argument_map.source_parameters()) {
            let parameter = state.imported_signature_type(signature).ok();
            let mut argument_sink = Vec::new();
            let Some(value) = state.lower_expr(&argument.expression, &mut argument_sink, parameter)
            else {
                return Err(Box::new(state));
            };
            if let Some(parameter) = parameter
                && !state.is_subtype(value.ty, parameter)
            {
                state.error(
                    argument.span,
                    format!(
                        "dependency function argument must be of type {}, found {}",
                        state.type_name(parameter),
                        state.type_name(value.ty)
                    ),
                );
                return Err(Box::new(state));
            }
            integer_arguments.push(match state.types[value.ty] {
                hir::Type::Integer(kind) => Some(kind),
                _ => None,
            });
            source_parameter_types.push(parameter.unwrap_or(state.any));
            source_args.push(match parameter {
                Some(parameter) => state.adapt_to(value, parameter),
                None => value,
            });
            argument_sinks.push(argument_sink);
        }
        let mut forwarding_parameters = Vec::with_capacity(
            source_parameter_types.len() + usize::from(interface.receiver().is_some()),
        );
        if let Some(receiver) = interface.receiver() {
            forwarding_parameters
                .push(state.imported_signature_type(receiver).unwrap_or(state.any));
        }
        forwarding_parameters.extend(source_parameter_types);

        if candidate.capability().is_none() {
            state.imported_dependency_capability_error(&candidate, &argument_map, call.span);
            return Err(Box::new(state));
        }
        let default_plan = match state.prepare_imported_defaults(&candidate, &argument_map) {
            Ok(plan) => plan,
            Err(error) => {
                state.error(call.span, error.to_string());
                return Err(Box::new(state));
            }
        };

        Ok(ImportedDependencyCallProbe {
            declaration_file: state.current_file,
            declaration_span: call.callee.span,
            state: Box::new(state),
            candidate,
            receiver,
            source_args,
            argument_sinks,
            argument_map,
            default_plan,
            parameter_types,
            forwarding_parameters,
            result_type,
            integer_arguments,
            call_span: call.span,
        })
    }

    fn imported_dependency_shape_error(
        &mut self,
        call: &ast::CallExpr,
        error: ArgumentShapeFailure,
    ) {
        self.error(
            call.span,
            format!(
                "dependency function `{}` {}",
                call.callee.text,
                error.describe()
            ),
        );
    }
}
