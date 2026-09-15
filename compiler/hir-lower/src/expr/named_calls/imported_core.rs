use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::{OptionalSignatureType, SignatureTypeKey};

use super::*;
use crate::imported_core::{ImportedSignatureTypeError, imported_signature_subtype};

struct ImportedCoreCallProbe {
    state: Box<Lowerer>,
    expression: hir::Expr,
    sink: Vec<hir::Statement>,
    parameters: Vec<SignatureTypeKey>,
}

impl Lowerer {
    pub(super) fn lower_imported_core_callable_partition(
        &mut self,
        references: &[hir::ImportedCorePreludeRef],
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
        expected: Option<hir::TypeId>,
    ) -> Result<Option<hir::Expr>, ()> {
        let mut successes = Vec::new();
        let mut failures = Vec::new();
        for &reference in references {
            let mut state = self.clone();
            match state.probe_imported_core_callable(reference, call, expected) {
                Ok(probe) => successes.push(probe),
                Err(failure) => failures.push(failure),
            }
        }
        if successes.is_empty() {
            if failures.len() == 1 {
                self.commit_layer_diagnostics(*failures.pop().expect("one failed candidate"));
            } else {
                let baseline = self.diagnostics.len();
                let mut traces = failures
                    .iter()
                    .flat_map(|failure| failure.diagnostics.iter().skip(baseline))
                    .map(|diagnostic| diagnostic.message.as_str())
                    .collect::<Vec<_>>();
                traces.sort_unstable();
                traces.dedup();
                self.error(
                    call.span,
                    format!(
                        "no applicable imported core candidate for `{}`:\n{}",
                        call.callee.text,
                        traces
                            .iter()
                            .map(|trace| format!("  - {trace}"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ),
                );
            }
            return Ok(None);
        }

        let winners = (0..successes.len())
            .filter(|&candidate| {
                !(0..successes.len()).any(|other| {
                    candidate != other
                        && imported_parameters_more_specific(
                            &successes[other].parameters,
                            &successes[candidate].parameters,
                        )
                })
            })
            .collect::<Vec<_>>();
        let [winner] = winners.as_slice() else {
            self.error(
                call.span,
                format!(
                    "call to `{}` is ambiguous in the core prelude layer",
                    call.callee.text
                ),
            );
            return Err(());
        };
        let winner = successes.swap_remove(*winner);
        *self = *winner.state;
        sink.extend(winner.sink);
        Ok(Some(winner.expression))
    }

    fn probe_imported_core_callable(
        &mut self,
        reference: hir::ImportedCorePreludeRef,
        call: &ast::CallExpr,
        expected: Option<hir::TypeId>,
    ) -> Result<ImportedCoreCallProbe, Box<Lowerer>> {
        let candidate = self.imported_core_callable_candidate(reference);
        if !matches!(
            candidate.target.capability(),
            hir::CoreHirCallableCapabilityV1::ParamFreeCandidate(_)
        ) {
            let _ = self.select_imported_core_callable(reference, call.callee.span);
            return Err(Box::new(self.clone()));
        }
        if !call.type_args.is_empty() {
            self.error(
                call.callee.span,
                "SCOOPC_CAPABILITY_CORE_GENERIC_UNAVAILABLE: imported core calls cannot supply type arguments"
                    .to_string(),
            );
            return Err(Box::new(self.clone()));
        }
        let signature = candidate.target.signature();
        if signature.effect() != scoop_identity::Effect::Ordinary {
            self.error(
                call.callee.span,
                "imported core suspend callables are unavailable in M23-3".to_string(),
            );
            return Err(Box::new(self.clone()));
        }
        if !matches!(signature.receiver(), OptionalSignatureType::Absent) {
            self.error(
                call.callee.span,
                "imported core callables with receivers are unavailable in M23-3".to_string(),
            );
            return Err(Box::new(self.clone()));
        }
        if signature.parameters().len() != call.args.len() {
            self.error(
                call.span,
                format!(
                    "imported core function `{}` expects {} argument(s), found {}",
                    call.callee.text,
                    signature.parameters().len(),
                    call.args.len()
                ),
            );
            return Err(Box::new(self.clone()));
        }
        if call.args.iter().any(|argument| {
            !matches!(argument.name, ast::CallArgumentName::Positional)
                || !matches!(argument.spread, ast::SpreadSyntax::Plain)
        }) {
            self.error(
                call.span,
                "imported core calls require positional, non-spread arguments in M23-3".to_string(),
            );
            return Err(Box::new(self.clone()));
        }

        let parameter_types = match signature
            .parameters()
            .iter()
            .map(|parameter| self.imported_signature_type(parameter))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(parameters) => parameters,
            Err(error) => {
                self.imported_signature_capability_error(call, error);
                return Err(Box::new(self.clone()));
            }
        };
        let result_type = match self.imported_signature_type(signature.result()) {
            Ok(result) => result,
            Err(error) => {
                self.imported_signature_capability_error(call, error);
                return Err(Box::new(self.clone()));
            }
        };
        if let Some(expected) = expected
            && !self.is_subtype(result_type, expected)
        {
            self.error(
                call.span,
                format!(
                    "imported core function `{}` returns {}, which is not compatible with expected {}",
                    call.callee.text,
                    self.type_name(result_type),
                    self.type_name(expected)
                ),
            );
            return Err(Box::new(self.clone()));
        }

        let mut lowered = Vec::with_capacity(call.args.len());
        let mut sink = Vec::new();
        for (argument, parameter) in call.args.iter().zip(parameter_types) {
            let Some(value) = self.lower_expr(&argument.expression, &mut sink, Some(parameter))
            else {
                return Err(Box::new(self.clone()));
            };
            if !self.is_subtype(value.ty, parameter) {
                self.error(
                    argument.span,
                    format!(
                        "imported core function argument must be of type {}, found {}",
                        self.type_name(parameter),
                        self.type_name(value.ty)
                    ),
                );
                return Err(Box::new(self.clone()));
            }
            lowered.push(self.adapt_to(value, parameter));
        }
        let Some(callee) = self.select_imported_core_callable(reference, call.callee.span) else {
            return Err(Box::new(self.clone()));
        };
        Ok(ImportedCoreCallProbe {
            state: Box::new(self.clone()),
            expression: hir::Expr {
                kind: hir::ExprKind::ImportedCoreCall {
                    callee,
                    args: lowered,
                },
                ty: result_type,
                span: call.span,
                origin: self.expression_origin(call.span),
            },
            sink,
            parameters: signature.parameters().to_vec(),
        })
    }

    fn imported_signature_capability_error(
        &mut self,
        call: &ast::CallExpr,
        error: ImportedSignatureTypeError,
    ) {
        let detail = match error {
            ImportedSignatureTypeError::Generic => "generic signature materialization",
            ImportedSignatureTypeError::Structural => {
                "cross-Cone nominal signature materialization"
            }
        };
        self.error(
            call.callee.span,
            format!(
                "SCOOPC_CAPABILITY_CORE_GENERIC_UNAVAILABLE: imported core function `{}` requires unavailable {detail}",
                call.callee.text
            ),
        );
    }
}

fn imported_parameters_more_specific(
    left: &[SignatureTypeKey],
    right: &[SignatureTypeKey],
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| imported_signature_subtype(left, right))
        && left != right
}
