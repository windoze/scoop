//! Checked external signatures use the ordinary candidate argument engine.

use hir::ImportedCallableSource;
use scoop_ast as ast;
use scoop_hir as hir;

use super::super::{
    ImportedArgumentMap, ImportedCallImplementation, ImportedCallReceiver,
    ImportedCallableCandidate, ImportedDependencyCallProbe, ImportedProbeCall,
};
use crate::Lowerer;
use crate::call_resolution::applicability::{
    DeclarationApplicabilityInput, DeclarationTypeArguments,
};
use crate::call_resolution::constraints::ConstraintOrigin;
use crate::call_resolution::contextual::{ArgumentInferenceFailureKind, ArgumentPattern};
use crate::call_resolution::probe::{CallInferenceInput, InferredCall};
use crate::imported_core::{ImportedSignatureTypeError, ImportedTypeBindings};

struct ResolvedNativeSignature {
    parameters: Vec<hir::TypeId>,
    result: hir::TypeId,
    patterns: Vec<ArgumentPattern>,
}

impl ResolvedNativeSignature {
    fn resolve(
        state: &mut Lowerer,
        candidate: &ImportedCallableCandidate,
        arguments: &ImportedArgumentMap,
    ) -> Result<Self, ImportedSignatureTypeError> {
        let interface = candidate.interface();
        let parameters = interface.parameters().parameters();
        let mut types = ImportedTypeBindings::new();
        for signature in parameters
            .iter()
            .map(|parameter| parameter.value_type())
            .chain(std::iter::once(interface.result()))
            .chain(arguments.source_parameters())
        {
            if let std::collections::btree_map::Entry::Vacant(entry) =
                types.entry(signature.clone())
            {
                entry.insert(state.imported_signature_type(signature)?);
            }
        }
        Ok(Self {
            parameters: parameters
                .iter()
                .map(|parameter| types[parameter.value_type()])
                .collect(),
            result: types[interface.result()],
            patterns: arguments
                .source_parameters()
                .iter()
                .enumerate()
                .map(|(index, signature)| ArgumentPattern {
                    ty: types[signature],
                    exact: arguments.is_array_input(index),
                })
                .collect(),
        })
    }
}

impl Lowerer {
    pub(super) fn probe_imported_native(
        mut self: Box<Self>,
        candidate: ImportedCallableCandidate,
        name: &ast::Ident,
        call: ImportedProbeCall<'_>,
        receiver: ImportedCallReceiver,
        argument_map: ImportedArgumentMap,
    ) -> Result<ImportedDependencyCallProbe, Box<Lowerer>> {
        let signature = match ResolvedNativeSignature::resolve(&mut self, &candidate, &argument_map)
        {
            Ok(signature) => signature,
            Err(error) => {
                self.error(call.span, error.diagnostic("dependency callable signature"));
                return Err(self);
            }
        };
        let InferredCall {
            values: source_args,
            sinks: argument_sinks,
            parameter_types,
            return_type: result_type,
            integer_arguments,
            ..
        } = match self.infer_call_arguments(CallInferenceInput {
            declaration: DeclarationApplicabilityInput {
                owner_parameters: &[],
                callable_parameters: &[],
                type_arguments: DeclarationTypeArguments::Callable {
                    owner_arguments: &[],
                },
                explicit_arguments: &[],
                bound_receiver: None,
            },
            parameter_types: &signature.parameters,
            return_type: signature.result,
            expressions: &call.arguments.expressions(),
            patterns: &signature.patterns,
            expected_result: None,
            forced_hint: None,
        }) {
            Ok(arguments) => arguments,
            Err(failure) => {
                if let ArgumentInferenceFailureKind::Constraint(constraint) = failure.kind {
                    let ConstraintOrigin::Argument(input) = constraint.origin else {
                        unreachable!(
                            "closed call inference only adds explicit argument constraints"
                        )
                    };
                    let index = input.index();
                    let value = failure.arguments[index]
                        .as_ref()
                        .expect("argument constraints refer to typed source inputs");
                    self.error(
                        call.arguments.span(index),
                        format!(
                            "dependency {} argument must be of type {}, found {}",
                            candidate.description(),
                            self.type_name(signature.patterns[index].ty),
                            self.type_name(value.ty),
                        ),
                    );
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
            receiver,
            source_args,
            argument_sinks,
            argument_map,
            default_plan,
            parameter_types,
            result_type,
            integer_arguments,
            call_span: call.span,
        })
    }
}
