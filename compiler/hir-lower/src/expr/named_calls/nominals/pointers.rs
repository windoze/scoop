//! The compiler-provided raw pointer entry participates in ordinary MSC.

use super::*;
use crate::call_resolution::named::{NamedIntrinsicStructOrigin, NamedIntrinsicStructProbe};

impl Lowerer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn collect_pointer_construction_probe(
        mut self,
        origin: NamedIntrinsicStructOrigin,
        call: &ast::CallExpr,
        expected: Option<TypeId>,
        fixed_alias: bool,
        alias: Option<&crate::aliases::AliasTemplate>,
        applicable: &mut Vec<NamedApplicable>,
        failures: &mut Vec<Box<Lowerer>>,
    ) {
        let parameters = match origin.type_parameters(&mut self, call.span) {
            Ok(parameters) => parameters,
            Err(error) => {
                self.error(
                    call.span,
                    format!("invalid pointer type declaration: {error}"),
                );
                failures.push(Box::new(self));
                return;
            }
        };
        let expected = if let Some(alias) = alias {
            let Some(explicit) = self.resolve_call_type_args(&call.type_args) else {
                failures.push(Box::new(self));
                return;
            };
            let Some((target, _)) = self.infer_alias_target(alias, &explicit, expected, call.span)
            else {
                failures.push(Box::new(self));
                return;
            };
            Some(target)
        } else {
            expected
        };
        let mut sink = Vec::new();
        let Some(expression) = self.lower_raw_pointer_init(
            &parameters,
            CallSite {
                type_args: if alias.is_some() {
                    &[]
                } else {
                    &call.type_args
                },
                args: &call.args,
                span: call.span,
            },
            &mut sink,
            expected,
        ) else {
            failures.push(Box::new(self));
            return;
        };
        applicable.push(NamedApplicable {
            probe: NamedFunctionLikeProbe::IntrinsicStruct(NamedIntrinsicStructProbe {
                origin,
                fixed_alias: fixed_alias || alias.is_some_and(|alias| !alias.parameterized()),
                span: call.span,
            }),
            commit: NamedFunctionCommit::Intrinsic(SuccessfulExprLayer {
                state: Box::new(self),
                expression,
                sink,
            }),
        });
    }
}
