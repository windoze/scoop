//! Provider-owned Scoop ABI entries for ordinary source extern declarations.
use super::*;

impl Lowerer {
    pub(in crate::pipeline) fn lower_native_function(
        &mut self,
        module: &hir::Module,
        source: hir::FunctionId,
        function: mir::FunctionId,
        external: hir::ExternFunctionId,
    ) -> (Vec<mir::Param>, mir::Type, smir::Body) {
        let declaration = &module.functions[source];
        let external = self.extern_map[&external];
        let signature = &self.extern_functions[external];
        let mut locals = Arena::new();
        let mut params = Vec::new();
        let mut args = Vec::new();
        for (index, ty) in signature.params.iter().enumerate() {
            let name = format!("arg{index}");
            let local = locals.alloc(mir::Local {
                name: name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            let selector = hir::LocalValueSelector::Parameter {
                declaration_index: index as u32,
            };
            let identity = hir::CborIdentityRecord::from_key(hir::LocalValueKey::new(
                declaration.materialization,
                selector,
            ))
            .expect("source extern parameters have canonical value identities");
            self.local_values.record(function, local, &identity);
            params.push(mir::Param {
                name,
                ty: ty.clone(),
                local,
            });
            args.push(smir::Expr::new(ty.clone(), smir::ExprKind::Local(local)));
        }
        let result = signature.result.scoop_type().clone();
        let call = smir::Expr::new(
            result.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee: mir::Callee::Extern(external),
                },
                args,
                return_ty: result.clone(),
            }),
        );
        let span = declaration.span;
        let statements = if result == mir::Type::Unit {
            vec![
                smir::Statement {
                    kind: smir::StatementKind::Expr(call),
                    span,
                },
                smir::Statement {
                    kind: smir::StatementKind::Return { value: None },
                    span,
                },
            ]
        } else {
            vec![smir::Statement {
                kind: smir::StatementKind::Return { value: Some(call) },
                span,
            }]
        };
        (
            params,
            result,
            smir::Body {
                locals,
                statements,
                coroutine_eh: None,
            },
        )
    }
}
