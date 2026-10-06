//! Checked external signatures use the ordinary candidate argument engine.

use hir::ImportedCallableSource;
use scoop_ast as ast;
use scoop_hir as hir;

use super::super::{
    ImportedArgumentMap, ImportedCallImplementation, ImportedCallReceiver,
    ImportedCallableCandidate, ImportedDependencyCallProbe, ImportedProbeCall,
};
use crate::Lowerer;
use crate::call_resolution::applicability::DeclarationTypeArguments;
use crate::call_resolution::constraints::ConstraintOrigin;
use crate::call_resolution::contextual::{ArgumentExpression, ArgumentInferenceFailureKind};
use crate::call_resolution::probe::{CallInferenceInput, InferredCall};
impl Lowerer {
    pub(super) fn probe_imported_native(
        mut self: Box<Self>,
        candidate: ImportedCallableCandidate,
        signature: super::super::signature::DependencySignature,
        name: &ast::Ident,
        call: ImportedProbeCall<'_>,
        receiver: ImportedCallReceiver,
        argument_map: ImportedArgumentMap,
    ) -> Result<ImportedDependencyCallProbe, Box<Lowerer>> {
        let expressions = call.arguments.expressions();
        let diagnostics_before = self.diagnostics.len();
        let integer_intrinsic = matches!(
            candidate.interface().effects().implementation(),
            hir::CallableImplementationV1::Intrinsic(hir::IntrinsicFunctionKind::Integer(_))
        );
        let InferredCall {
            values: source_args,
            sinks: argument_sinks,
            parameter_types,
            return_type: result_type,
            numeric_arguments,
            ..
        } = match self.infer_call_arguments(CallInferenceInput {
            signature: &signature,
            argument_map: argument_map.mapping(),
            type_arguments: DeclarationTypeArguments::Callable {
                owner_arguments: &[],
            },
            explicit_arguments: &[],
            bound_receiver: None,
            expressions: &expressions,
            expected_result: None,
            forced_hint: None,
        }) {
            Ok(arguments) => arguments,
            Err(failure) => {
                if let ArgumentInferenceFailureKind::Constraint(constraint) = &failure.kind {
                    let ConstraintOrigin::Argument(input) = constraint.origin else {
                        unreachable!(
                            "closed call inference only adds explicit argument constraints"
                        )
                    };
                    let index = input.index();
                    let value = failure.arguments[index]
                        .as_ref()
                        .expect("argument constraints refer to typed source inputs");
                    let expected = argument_map
                        .mapping()
                        .forwarding_parameter_types(&signature.value_parameters)[index];
                    let mut message = format!(
                        "dependency {} argument must be of type {}, found {}",
                        candidate.description(),
                        self.type_name(expected),
                        self.type_name(value.ty),
                    );
                    if integer_intrinsic
                        && let (hir::Type::Integer(expected), hir::Type::Integer(found)) =
                            (&self.types[expected], &self.types[value.ty])
                        && let Some(hint) =
                            Self::primitive_integer_conversion_hint(*expected, *found)
                    {
                        message.push_str(&hint);
                    }
                    self.error(call.arguments.span(index), message);
                } else if integer_intrinsic
                    && let ArgumentInferenceFailureKind::Expression(expression) = &failure.kind
                    && let ArgumentExpression::Source(source) = expressions[expression.source_index]
                    && let Some(found) = crate::expr::integer_literal_default_kind(source)
                {
                    let expected = argument_map
                        .mapping()
                        .forwarding_parameter_types(&signature.value_parameters)
                        [expression.source_index];
                    if let hir::Type::Integer(expected) = self.types[expected]
                        && let Some(hint) = Self::primitive_integer_conversion_hint(expected, found)
                        && let Some(diagnostic) = self.diagnostics[diagnostics_before..]
                            .iter_mut()
                            .find(|diagnostic| diagnostic.span == Some(expression.span))
                    {
                        diagnostic.message.push_str(&hint);
                    }
                }
                return Err(self);
            }
        };
        if !candidate.executable() {
            self.imported_dependency_capability_error(
                &candidate,
                argument_map.has_vararg(),
                "dependency callable",
                call.span,
            );
            return Err(self);
        }
        let default_plan = match self.prepare_imported_defaults(&candidate, &argument_map) {
            Ok(plan) => plan,
            Err(error) => {
                self.error(call.span, error.to_string());
                return Err(self);
            }
        };
        Ok(ImportedDependencyCallProbe {
            implementation: ImportedCallImplementation::Native,
            declaration_file: self.current_file,
            declaration_span: name.span,
            state: self,
            candidate,
            signature,
            declared_receiver: None,
            receiver,
            source_args,
            argument_sinks,
            argument_map,
            default_plan,
            parameter_types,
            result_type,
            numeric_arguments,
            call_span: call.span,
        })
    }
}
