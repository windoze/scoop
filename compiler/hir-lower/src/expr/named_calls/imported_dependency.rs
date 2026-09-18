//! Candidate-local probing and winner-only commit for ordinary dependencies.

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::Effect;

use super::*;
use crate::call_resolution::arguments::ArgumentShapeFailure;
use crate::call_resolution::specificity::DeclarationForwardingView;

mod arguments;

use arguments::{ImportedArgumentMap, ImportedParameterInput};

pub(crate) struct ImportedDependencyCallProbe {
    state: Box<Lowerer>,
    candidate: hir::ImportedDependencyCallableCandidate,
    receiver: Option<hir::Expr>,
    source_args: Vec<hir::Expr>,
    argument_sinks: Vec<Vec<hir::Statement>>,
    argument_map: ImportedArgumentMap,
    parameter_types: Vec<hir::TypeId>,
    forwarding_parameters: Vec<hir::TypeId>,
    result_type: hir::TypeId,
    integer_arguments: Vec<Option<hir::IntegerKind>>,
    declaration_file: usize,
    declaration_span: ast::Span,
    call_span: ast::Span,
}

impl ImportedDependencyCallProbe {
    pub(crate) fn forwarding(&self) -> DeclarationForwardingView<'_> {
        DeclarationForwardingView::nominal_parameters(&[], &self.forwarding_parameters)
    }

    pub(crate) fn parameterized(&self) -> bool {
        !self.candidate.interface().type_parameters().is_empty()
    }

    pub(crate) fn defaults(&self) -> usize {
        self.argument_map.defaults()
    }

    pub(crate) fn vararg(&self) -> bool {
        self.argument_map.has_vararg()
    }

    pub(crate) fn source_argument_integer(&self, index: usize) -> Option<hir::IntegerKind> {
        self.integer_arguments[index]
    }

    pub(crate) fn signature(&self, state: &Lowerer, name: &str) -> String {
        let parameters = self
            .candidate
            .source_interface()
            .expect("callable candidates retain their validated source interface")
            .parameters()
            .parameters()
            .iter()
            .zip(&self.parameter_types)
            .map(|(parameter, ty)| {
                format!("{}: {}", parameter.name().as_str(), state.type_name(*ty))
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{name}({parameters}): {}",
            state.type_name(self.result_type)
        )
    }

    pub(crate) const fn declaration_location(&self) -> (usize, ast::Span) {
        (self.declaration_file, self.declaration_span)
    }
}

impl Lowerer {
    pub(super) fn probe_imported_dependency_callable(
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

        Ok(ImportedDependencyCallProbe {
            declaration_file: state.current_file,
            declaration_span: call.callee.span,
            state: Box::new(state),
            candidate,
            receiver,
            source_args,
            argument_sinks,
            argument_map,
            parameter_types,
            forwarding_parameters,
            result_type,
            integer_arguments,
            call_span: call.span,
        })
    }

    pub(super) fn commit_imported_dependency_callable(
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
            parameter_types,
            result_type,
            call_span,
            ..
        } = probe;
        *self = *state;
        if candidate.capability().is_none() {
            self.imported_dependency_capability_error(&candidate, &argument_map, call_span);
            return None;
        }
        if argument_map.defaults() != 0 {
            let template = argument_map
                .parameters()
                .iter()
                .find_map(|input| match input {
                    ImportedParameterInput::Default(template) => Some(*template),
                    ImportedParameterInput::Explicit(_) | ImportedParameterInput::Vararg => None,
                })
                .expect("a nonzero imported default count retains a template key");
            self.error(
                call_span,
                format!(
                    "SCOOP_HIR_CROSS_CONE_LAYOUT_REQUIRED: imported default argument {template:?} requires dependency-template materialization"
                ),
            );
            return None;
        }
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
        let mut args = Vec::with_capacity(parameter_types.len() + usize::from(receiver.is_some()));
        if let Some(receiver) = receiver {
            args.push(receiver);
        }
        for ((input, parameter), name) in argument_map
            .parameters()
            .iter()
            .zip(parameter_types)
            .zip(parameter_names)
        {
            let ImportedParameterInput::Explicit(source) = input else {
                unreachable!("an executable dependency callable has no materialized vararg/default")
            };
            let value = self.adapt_to(source_args[*source].clone(), parameter);
            args.push(self.materialize_temporary(
                format!("$dependency.parameter.{name}"),
                value,
                call_span,
                sink,
            ));
        }

        let reference = match self
            .dependencies
            .as_mut()
            .expect("ordinary lowering carries a dependency selection plan")
            .select_callable(candidate)
        {
            Ok(reference) => reference,
            Err(error) => {
                self.error(
                    call_span,
                    format!("failed to select imported dependency callable: {error}"),
                );
                return None;
            }
        };
        let existing = self
            .imported_dependency_callables
            .iter()
            .find_map(|(id, use_)| (use_.reference() == reference).then_some(id));
        let callee = existing.unwrap_or_else(|| {
            self.imported_dependency_callables
                .alloc(hir::ImportedDependencyCallableUse::new(reference))
        });
        Some(hir::Expr {
            kind: hir::ExprKind::ImportedDependencyCall { callee, args },
            ty: result_type,
            span: call_span,
            origin: self.expression_origin(call_span),
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

    fn imported_dependency_capability_error(
        &mut self,
        candidate: &hir::ImportedDependencyCallableCandidate,
        arguments: &ImportedArgumentMap,
        span: ast::Span,
    ) {
        let interface = candidate.interface();
        let (code, requirement) =
            if matches!(interface.owner(), hir::PublicDeclarationOwnerV1::Nominal(_))
                || interface.access() == hir::PublicLookupAccessV1::PublicSlot
            {
                (
                    "SCOOP_HIR_CROSS_CONE_DISPATCH_REQUIRED",
                    "member/dispatch capability from M23-6",
                )
            } else if !interface.type_parameters().is_empty()
                || matches!(candidate.target(), hir::ImportedTarget::GenericFunction(_))
                || interface.effects().execution() == Effect::Suspend
                || arguments.has_vararg()
            {
                (
                    "SCOOP_HIR_CROSS_CONE_GENERIC_REQUIRED",
                    "generic/ODR capability from M23-7",
                )
            } else if interface.effects().implementation() != hir::CallableImplementationV1::Scoop {
                (
                    "SCOOP_HIR_CROSS_CONE_NATIVE_REQUIRED",
                    "native closure capability from M23-10",
                )
            } else {
                (
                    "SCOOP_HIR_CROSS_CONE_LAYOUT_REQUIRED",
                    "layout/ABI capability from M23-6",
                )
            };
        self.error(
            span,
            format!("{code}: dependency callable requires {requirement}"),
        );
    }
}
