//! The compiler-provided raw pointer entry participates in ordinary MSC.

use super::*;
use crate::call_resolution::named::{NamedIntrinsicStructOrigin, NamedIntrinsicStructProbe};

impl Lowerer {
    pub(super) fn collect_pointer_construction_probe(
        mut self,
        origin: NamedIntrinsicStructOrigin,
        call: &ast::CallExpr,
        expected: Option<TypeId>,
        fixed_alias: bool,
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
        let mut sink = Vec::new();
        let Some(expression) = self.lower_raw_pointer_init(
            &parameters,
            CallSite {
                type_args: &call.type_args,
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
                fixed_alias,
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
