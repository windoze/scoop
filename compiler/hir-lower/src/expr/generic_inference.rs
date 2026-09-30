//! Nominal candidate inference and diagnostics.

use super::*;

mod context;

use crate::call_resolution::applicability::{
    DeclarationApplicabilityInput, DeclarationTypeArguments,
};
use crate::call_resolution::candidates::{NominalConstructorSource, NominalConstructorView};
use crate::call_resolution::constraints::{ConstraintFailure, ConstraintOrigin};
use crate::call_resolution::contextual::{ArgumentExpression, ArgumentInferenceFailureKind};
use crate::call_resolution::diagnostics::{
    nominal_source_signature, render_nominal_constraint_failure,
};
use crate::call_resolution::probe::{CallInferenceInput, InferredCall};

impl Lowerer {
    pub(crate) fn lower_nominal_arguments(
        &mut self,
        input: NominalArgumentInput<'_>,
    ) -> Option<NominalArguments> {
        let NominalArgumentInput {
            view,
            argument_map,
            expressions: arg_exprs,
            explicit_type_args: explicit_arguments,
            expected_type_args: expected_arguments,
            span,
        } = input;
        debug_assert_eq!(argument_map.source_order.len(), arg_exprs.len());
        let expressions = arg_exprs
            .iter()
            .map(|argument| ArgumentExpression::Source(&argument.expression))
            .collect::<Vec<_>>();
        let patterns = argument_map.inference_patterns(&view.value_parameters);
        let parameter_types = view
            .value_parameters
            .iter()
            .map(|parameter| parameter.ty)
            .collect::<Vec<_>>();
        let InferredCall {
            types,
            values,
            sinks,
            ..
        } = match self.infer_call_arguments(CallInferenceInput {
            declaration: DeclarationApplicabilityInput {
                owner_parameters: &view.owner_parameters,
                callable_parameters: &[],
                type_arguments: DeclarationTypeArguments::Nominal {
                    template: self
                        .nominal_application(view.result_type)
                        .expect("a nominal candidate retains its full result application")
                        .template,
                    expected_arguments,
                },
                explicit_arguments,
                bound_receiver: None,
            },
            parameter_types: &parameter_types,
            return_type: view.result_type,
            expressions: &expressions,
            patterns: &patterns,
            expected_result: None,
            forced_hint: None,
        }) {
            Ok(arguments) => arguments,
            Err(failure) => {
                if let ArgumentInferenceFailureKind::Constraint(constraint) = failure.kind {
                    self.diagnose_nominal_failure(
                        view,
                        argument_map,
                        explicit_arguments,
                        &failure.arguments,
                        constraint,
                        span,
                    );
                }
                return None;
            }
        };
        Some(NominalArguments {
            args: values,
            argument_sinks: sinks,
            type_args: types.owner,
        })
    }

    pub(super) fn diagnose_nominal_failure(
        &mut self,
        view: &NominalConstructorView,
        argument_map: &crate::call_resolution::arguments::CandidateArgumentMap,
        explicit_arguments: &[ResolvedCallTypeArgument],
        arguments: &[Option<hir::Expr>],
        failure: ConstraintFailure,
        span: Span,
    ) {
        let diagnostic_span = if let Some(
            crate::call_resolution::constraints::InferenceVariableId::Owner(variable),
        ) = failure.kind.inference_variable()
        {
            explicit_arguments
                .get(
                    crate::call_resolution::constraints::InferenceVariableId::Owner(variable)
                        .group_index(),
                )
                .map_or(span, |argument| argument.span())
        } else {
            match failure.origin {
                ConstraintOrigin::Argument(input) => arguments
                    .get(input.index())
                    .and_then(Option::as_ref)
                    .map_or(span, |argument| argument.span),
                _ => span,
            }
        };
        let mut reason =
            render_nominal_constraint_failure(self, view, argument_map, arguments, &failure);
        if let Some(crate::call_resolution::constraints::InferenceVariableId::Owner(variable)) =
            failure.kind.inference_variable()
        {
            let failed = crate::call_resolution::constraints::InferenceVariableId::Owner(variable)
                .group_index();
            if matches!(
                explicit_arguments.get(failed),
                Some(ResolvedCallTypeArgument::Infer { .. })
            ) {
                let fixed = explicit_arguments
                    .iter()
                    .zip(&view.owner_parameters)
                    .filter_map(|(argument, parameter)| {
                        let ResolvedCallTypeArgument::Explicit { ty, .. } = argument else {
                            return None;
                        };
                        Some(format!("{} = {}", parameter.name, self.type_name(*ty)))
                    })
                    .collect::<Vec<_>>();
                if !fixed.is_empty() {
                    reason.push_str("; fixed type arguments: ");
                    reason.push_str(&fixed.join(", "));
                }
            }
        }
        self.nominal_candidate_diagnostic(view, diagnostic_span, &reason);
    }

    pub(crate) fn diagnose_nominal_shape_failure(
        &mut self,
        view: &NominalConstructorView,
        span: Span,
        reason: String,
    ) {
        self.nominal_candidate_diagnostic(view, span, &reason);
    }

    fn nominal_candidate_diagnostic(
        &mut self,
        view: &NominalConstructorView,
        span: Span,
        reason: &str,
    ) {
        let target = self.nominal_value_name(view.target);
        let signature = nominal_source_signature(self, view);
        self.error(
            span,
            format!(
                "no applicable candidate for constructor `{target}` in nominal constructor candidate layer:\n  - {signature} — {reason}"
            ),
        );
    }

    fn nominal_value_name(&self, target: NominalConstructorSource) -> String {
        match target {
            NominalConstructorSource::Struct(constructor) => self.structs
                [self.struct_constructors[constructor].owner]
                .name
                .clone(),
            NominalConstructorSource::Class(constructor) => self.classes
                [self.class_constructors[constructor].owner]
                .name
                .clone(),
            NominalConstructorSource::IntrinsicClass(class) => self.classes[class].name.clone(),
            NominalConstructorSource::ImportedArray(owner) => self
                .dependencies
                .as_ref()
                .and_then(|dependencies| dependencies.nominal_declaration(owner))
                .expect("a resolved array retains its dependency declaration")
                .name()
                .to_owned(),
            NominalConstructorSource::Variant(variant) => format!(
                "{}.{}",
                self.enums[variant.enumeration()].name,
                self.enums[variant.enumeration()].variants[variant.local_index() as usize].name,
            ),
        }
    }
}
