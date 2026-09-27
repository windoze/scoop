use super::*;

mod applications;
mod expressions;
mod regions;
mod statements;

impl Lowerer {
    pub(in crate::effects) fn generic_call_sites(&self) -> Vec<GenericCallSite> {
        let mut out = Vec::new();
        for caller in self.effect_callable_ids() {
            let calls = match caller {
                GenericCallable::Imported(_) => {
                    unreachable!("only current declarations infer new requirements")
                }
                GenericCallable::Function(id) => {
                    let hir::FunctionKind::User(body) = &self.functions[id].kind else {
                        continue;
                    };
                    self.generic_calls_in_body(body)
                }
                GenericCallable::ClassConstructor(id) => {
                    self.generic_calls_in_class_constructor(&self.class_constructors[id])
                }
                GenericCallable::StructConstructor(id) => {
                    self.generic_calls_in_struct_constructor(&self.struct_constructors[id])
                }
            };
            out.extend(calls.into_iter().map(|call| GenericCallSite {
                caller,
                callee: call.callee,
                arguments: call.arguments,
                span: call.span,
            }));
        }
        out
    }
}
