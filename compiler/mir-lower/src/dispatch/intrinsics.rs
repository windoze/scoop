//! Callable bodies for intrinsic methods selected by actual conformances.

use super::*;
use crate::closures::primitive::PrimitiveReference;

impl Lowerer {
    pub(crate) fn interface_method_entry(
        &mut self,
        module: &hir::Module,
        source: hir::FunctionId,
    ) -> mir::FunctionId {
        if let Some(&function) = self.function_map.get(&source) {
            return function;
        }
        let declaration = &module.functions[source];
        let hir::FunctionKind::Intrinsic(intrinsic) = &declaration.kind else {
            unreachable!("ordinary interface implementations are declared before dispatch")
        };
        let operation = PrimitiveReference::from_intrinsic(intrinsic.kind)
            .expect("core intrinsic interface implementations are primitive members");
        let function = self.declare_function(module, source);
        let materialization = declaration.materialization;
        let mut lowerer = self.body_lowerer(
            module,
            function,
            materialization,
            mir::ImmortalObjectOwner::Callable(materialization),
        );
        let params = intrinsic
            .parameters
            .iter()
            .enumerate()
            .map(|(index, source)| {
                let ty = lowerer.lower_type(*source);
                let name = format!("arg{index}");
                let local = lowerer.locals.alloc(mir::Local {
                    name: name.clone(),
                    ty: ty.clone(),
                    mutable: false,
                });
                mir::Param {
                    name: name.clone(),
                    ty,
                    local,
                }
            })
            .collect::<Vec<_>>();
        lowerer.local_values.record_generated_dispatch_parameters(
            function,
            materialization,
            &params,
        );
        let argument = |index: usize| {
            let parameter = &params[index];
            smir::Expr::local(parameter.local, parameter.ty.clone())
        };
        let return_ty = lowerer.lower_type(declaration.return_ty);
        let value = match operation {
            PrimitiveReference::Unary(operation) => {
                lowerer.unary_reference_value(operation, argument(0), &return_ty)
            }
            PrimitiveReference::Binary(operation) => lowerer.binary_reference_value(
                operation,
                argument(0),
                argument(1),
                &return_ty,
                declaration.span,
            ),
        };
        let mut statements = lowerer
            .prelude
            .into_iter()
            .map(|kind| smir::Statement {
                kind,
                span: declaration.span,
            })
            .collect::<Vec<_>>();
        statements.push(smir::Statement {
            kind: smir::StatementKind::Return { value },
            span: declaration.span,
        });
        let body = smir::Body {
            locals: lowerer.locals,
            statements,
            coroutine_eh: None,
        };
        let body = finish_cfg_body(
            &mut self.local_values,
            &mut self.coroutines,
            function,
            materialization,
            cfg::lower(body, return_ty.clone(), &self.enums.defs, &self.classes),
        );
        let entry = &mut self.functions[function];
        entry.params = params;
        entry.return_ty = return_ty;
        entry.body = body;
        function
    }
}
