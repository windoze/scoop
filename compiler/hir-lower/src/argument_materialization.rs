use scoop_ast::Span;
use scoop_hir as hir;

use crate::call_resolution::arguments::{
    CandidateArgumentMap, ResolvedParameterInput, ResolvedVarargInput, VarargPartKind,
};
use crate::call_resolution::candidates::NominalConstructorView;
use crate::defaults::SourceParameterCalling;
use crate::{Lowerer, Type};

pub(crate) struct CallableArgumentMaterialization<'a> {
    pub function: hir::FunctionId,
    pub argument_map: &'a CandidateArgumentMap,
    pub type_args: &'a [hir::TypeId],
    pub receiver: Option<hir::Expr>,
    pub source_args: Vec<hir::Expr>,
    pub argument_sinks: Vec<Vec<hir::Statement>>,
    pub call_span: Span,
}

pub(crate) struct NominalArgumentMaterialization<'a> {
    pub view: &'a NominalConstructorView,
    pub argument_map: &'a CandidateArgumentMap,
    pub type_args: &'a [hir::TypeId],
    pub source_args: Vec<hir::Expr>,
    pub argument_sinks: Vec<Vec<hir::Statement>>,
    pub call_span: Span,
}

struct ParameterMaterialization<'a> {
    argument_map: &'a CandidateArgumentMap,
    bindings: &'a [(hir::TypeParamId, hir::TypeId)],
    receiver: Option<&'a hir::Expr>,
    source_args: &'a [hir::Expr],
    call_span: Span,
    sink: &'a mut Vec<hir::Statement>,
}

impl Lowerer {
    pub(crate) fn materialize_callable_arguments(
        &mut self,
        request: CallableArgumentMaterialization<'_>,
        sink: &mut Vec<hir::Statement>,
    ) -> (Option<hir::Expr>, Vec<hir::Expr>) {
        let receiver = request.receiver.map(|receiver| {
            self.materialize_temporary("$receiver".to_string(), receiver, request.call_span, sink)
        });
        let mut argument_sinks = request.argument_sinks;
        let source_args = request
            .source_args
            .into_iter()
            .enumerate()
            .map(|(index, argument)| {
                sink.append(&mut argument_sinks[index]);
                self.materialize_temporary(
                    format!("$argument.{index}"),
                    argument,
                    request.call_span,
                    sink,
                )
            })
            .collect::<Vec<_>>();
        let signature = self.signatures[&request.function].clone();
        let bindings = signature
            .type_params
            .iter()
            .zip(request.type_args)
            .map(|(parameter, &argument)| (parameter.id, argument))
            .collect::<Vec<_>>();
        let parameters = signature
            .params
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                let ty = if index == 0
                    && self
                        .foreign_callback_core
                        .is_some_and(|core| core.register == request.function)
                {
                    let ResolvedParameterInput::Explicit(source) =
                        request.argument_map.parameters[index].input
                    else {
                        unreachable!("foreign callback input is required")
                    };
                    source_args[source.index()].ty
                } else {
                    parameter.ty
                };
                (parameter.name.text.clone(), ty)
            })
            .collect::<Vec<_>>();
        let callings = signature
            .params
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                self.source_parameter_calling(
                    crate::defaults::SourceParameterOwner::Function(request.function),
                    index,
                    parameter.ty,
                    &parameter.calling,
                )
            })
            .collect::<Vec<_>>();
        let args = self.materialize_parameter_inputs(
            &parameters,
            &callings,
            ParameterMaterialization {
                argument_map: request.argument_map,
                bindings: &bindings,
                receiver: receiver.as_ref(),
                source_args: &source_args,
                call_span: request.call_span,
                sink,
            },
        );
        (receiver, args)
    }

    pub(crate) fn materialize_nominal_arguments(
        &mut self,
        request: NominalArgumentMaterialization<'_>,
        sink: &mut Vec<hir::Statement>,
    ) -> Vec<hir::Expr> {
        let mut argument_sinks = request.argument_sinks;
        let source_args = request
            .source_args
            .into_iter()
            .enumerate()
            .map(|(index, argument)| {
                sink.append(&mut argument_sinks[index]);
                self.materialize_temporary(
                    format!("$argument.{index}"),
                    argument,
                    request.call_span,
                    sink,
                )
            })
            .collect::<Vec<_>>();
        let parameters = request
            .view
            .value_parameters
            .iter()
            .map(|parameter| (parameter.name.clone(), parameter.ty))
            .collect::<Vec<_>>();
        let callings = request
            .view
            .value_parameters
            .iter()
            .map(|parameter| parameter.calling)
            .collect::<Vec<_>>();
        let bindings = request
            .view
            .owner_parameters
            .iter()
            .zip(request.type_args)
            .map(|(parameter, &argument)| (parameter.id, argument))
            .collect::<Vec<_>>();
        self.materialize_parameter_inputs(
            &parameters,
            &callings,
            ParameterMaterialization {
                argument_map: request.argument_map,
                bindings: &bindings,
                receiver: None,
                source_args: &source_args,
                call_span: request.call_span,
                sink,
            },
        )
    }

    fn materialize_parameter_inputs(
        &mut self,
        parameters: &[(String, hir::TypeId)],
        callings: &[SourceParameterCalling],
        context: ParameterMaterialization<'_>,
    ) -> Vec<hir::Expr> {
        assert_eq!(callings.len(), parameters.len());
        assert_eq!(context.argument_map.parameters.len(), parameters.len());

        let mut materialized = Vec::with_capacity(parameters.len());
        for ((parameter_input, calling), (name, declared_ty)) in context
            .argument_map
            .parameters
            .iter()
            .zip(callings)
            .zip(parameters)
        {
            let parameter_ty = self.instantiate_method_ty(*declared_ty, context.bindings);
            let value = match &parameter_input.input {
                ResolvedParameterInput::Explicit(input) => {
                    let value = context.source_args[input.index()].clone();
                    debug_assert!(self.is_subtype(value.ty, parameter_ty));
                    self.adapt_to(value, parameter_ty)
                }
                ResolvedParameterInput::Default(template) => self.instantiate_default(
                    *template,
                    context.bindings,
                    context.receiver,
                    &materialized,
                    context.call_span,
                    context.sink,
                ),
                ResolvedParameterInput::Vararg(input) => {
                    let SourceParameterCalling::Vararg {
                        element_type,
                        array_type,
                        omission: _,
                    } = *calling
                    else {
                        unreachable!("a vararg plan belongs to a vararg parameter")
                    };
                    let element_type = self.instantiate_method_ty(element_type, context.bindings);
                    let array_type = self.instantiate_method_ty(array_type, context.bindings);
                    debug_assert_eq!(array_type, parameter_ty);
                    match input {
                        ResolvedVarargInput::WholeArray(input) => {
                            let value = context.source_args[input.index()].clone();
                            debug_assert!(self.types_equal(value.ty, array_type));
                            value
                        }
                        ResolvedVarargInput::Parts(parts) => {
                            let parts = parts
                                .iter()
                                .map(|part| {
                                    let value = context.source_args[part.input.index()].clone();
                                    match part.kind {
                                        VarargPartKind::Element => {
                                            debug_assert!(self.is_subtype(value.ty, element_type));
                                            hir::ArrayAssemblyPart::Element(
                                                self.adapt_to(value, element_type),
                                            )
                                        }
                                        VarargPartKind::CopyArray => {
                                            debug_assert!(self.types_equal(value.ty, array_type));
                                            hir::ArrayAssemblyPart::CopyArray(value)
                                        }
                                    }
                                })
                                .collect();
                            self.array_assembly(element_type, array_type, parts, context.call_span)
                        }
                        ResolvedVarargInput::Empty => self.array_assembly(
                            element_type,
                            array_type,
                            Vec::new(),
                            context.call_span,
                        ),
                        ResolvedVarargInput::Default(template) => self.instantiate_default(
                            *template,
                            context.bindings,
                            context.receiver,
                            &materialized,
                            context.call_span,
                            context.sink,
                        ),
                    }
                }
            };
            debug_assert!(self.types_equal(value.ty, parameter_ty));
            materialized.push(self.materialize_temporary(
                format!("$parameter.{name}"),
                value,
                context.call_span,
                context.sink,
            ));
        }
        materialized
    }

    fn array_assembly(
        &self,
        element_type: hir::TypeId,
        array_type: hir::TypeId,
        parts: Vec<hir::ArrayAssemblyPart>,
        span: Span,
    ) -> hir::Expr {
        let Type::Class(result_type) = self.types[array_type] else {
            unreachable!("a vararg parameter has an exact Array application")
        };
        hir::Expr {
            kind: hir::ExprKind::ArrayAssembly(hir::ArrayAssembly {
                element_type,
                parts,
                result_type,
            }),
            ty: array_type,
            span,
            origin: self.expression_origin(span),
        }
    }

    fn materialize_temporary(
        &mut self,
        name: String,
        value: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> hir::Expr {
        let ty = value.ty;
        let local = self.alloc_local(name, ty, false);
        sink.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local },
                init: value,
            },
            span,
        });
        hir::Expr {
            kind: hir::ExprKind::Local(local),
            ty,
            span,
            origin: self.expression_origin(span),
        }
    }
}
