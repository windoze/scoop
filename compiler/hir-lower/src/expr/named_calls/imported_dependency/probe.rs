use scoop_ast as ast;
use scoop_hir as hir;

use super::{ImportedArgumentMap, ImportedDependencyCallProbe};
use crate::Lowerer;
use crate::call_resolution::arguments::ArgumentShapeFailure;
use crate::expr::CallSite;

enum ImportedDependencyCallReceiver {
    Implicit,
    Explicit(hir::Expr),
}

impl Lowerer {
    pub(in super::super) fn probe_imported_dependency_callable(
        &self,
        binding: &hir::DirectImportedTargetBinding,
        call: &ast::CallExpr,
        expected: Option<hir::TypeId>,
    ) -> Result<ImportedDependencyCallProbe, Box<Lowerer>> {
        self.probe_imported_dependency_callable_with_receiver(
            binding,
            &call.callee,
            CallSite {
                type_args: &call.type_args,
                args: &call.args,
                span: call.span,
            },
            expected,
            ImportedDependencyCallReceiver::Implicit,
            false,
        )
    }

    pub(in crate::expr) fn probe_imported_dependency_extension_callable(
        &self,
        binding: &hir::DirectImportedTargetBinding,
        receiver: hir::Expr,
        name: &ast::Ident,
        call: CallSite<'_>,
        expected: Option<hir::TypeId>,
        operator_set: bool,
    ) -> Result<ImportedDependencyCallProbe, Box<Lowerer>> {
        self.probe_imported_dependency_callable_with_receiver(
            binding,
            name,
            call,
            expected,
            ImportedDependencyCallReceiver::Explicit(receiver),
            operator_set,
        )
    }

    fn probe_imported_dependency_callable_with_receiver(
        &self,
        binding: &hir::DirectImportedTargetBinding,
        name: &ast::Ident,
        call: CallSite<'_>,
        expected: Option<hir::TypeId>,
        receiver_source: ImportedDependencyCallReceiver,
        operator_set: bool,
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
                    name.span,
                    format!("invalid imported dependency callable: {error}"),
                );
                return Err(Box::new(state));
            }
        };
        let interface = candidate.interface();
        let expected_type_arguments = interface.type_parameters().binders().len();
        if call.type_args.len() != expected_type_arguments {
            state.error(
                name.span,
                format!(
                    "dependency function `{}` expects {expected_type_arguments} type argument(s), found {}",
                    name.text,
                    call.type_args.len()
                ),
            );
            return Err(Box::new(state));
        }
        let Some(source) = candidate.source_interface() else {
            state.error(
                name.span,
                format!(
                    "invalid imported dependency callable `{}`: source interface is missing",
                    name.text
                ),
            );
            return Err(Box::new(state));
        };
        let argument_map = match if operator_set {
            ImportedArgumentMap::source_operator_set(source.parameters().parameters(), call.args)
        } else {
            ImportedArgumentMap::source(source.parameters().parameters(), call.args)
        } {
            Ok(map) => map,
            Err(error) => {
                state.imported_dependency_shape_error(name, call, error);
                return Err(Box::new(state));
            }
        };

        let receiver = match interface.owner() {
            hir::PublicDeclarationOwnerV1::TopLevel => match receiver_source {
                ImportedDependencyCallReceiver::Implicit => None,
                ImportedDependencyCallReceiver::Explicit(_) => {
                    state.error(
                        name.span,
                        format!("dependency function `{}` is not an extension", name.text),
                    );
                    return Err(Box::new(state));
                }
            },
            hir::PublicDeclarationOwnerV1::Extension => {
                let Some(receiver_signature) = interface.receiver() else {
                    state.error(
                        name.span,
                        format!(
                            "invalid imported dependency extension `{}`: receiver type is missing",
                            name.text
                        ),
                    );
                    return Err(Box::new(state));
                };
                let receiver_type = state.imported_signature_type(receiver_signature).ok();
                let receiver = match receiver_source {
                    ImportedDependencyCallReceiver::Implicit => {
                        let Some(receiver) = state.lower_current_this(name.span) else {
                            return Err(Box::new(state));
                        };
                        receiver
                    }
                    ImportedDependencyCallReceiver::Explicit(receiver) => receiver,
                };
                if let Some(receiver_type) = receiver_type {
                    if !state.is_subtype(receiver.ty, receiver_type) {
                        state.error(
                            name.span,
                            format!(
                                "dependency extension `{}` expects receiver {}, found {}",
                                name.text,
                                state.type_name(receiver_type),
                                state.type_name(receiver.ty),
                            ),
                        );
                        return Err(Box::new(state));
                    }
                    Some(state.adapt_to(receiver, receiver_type))
                } else {
                    Some(receiver)
                }
            }
            hir::PublicDeclarationOwnerV1::Nominal(_) => {
                if matches!(receiver_source, ImportedDependencyCallReceiver::Explicit(_)) {
                    state.error(
                        name.span,
                        format!("dependency function `{}` is not an extension", name.text),
                    );
                    return Err(Box::new(state));
                }
                None
            }
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
                    name.text,
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
            state.imported_dependency_capability_error(
                &candidate,
                Some(&argument_map),
                "dependency callable",
                call.span,
            );
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
            declaration_span: name.span,
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
        name: &ast::Ident,
        call: CallSite<'_>,
        error: ArgumentShapeFailure,
    ) {
        self.error(
            call.span,
            format!("dependency function `{}` {}", name.text, error.describe()),
        );
    }
}
