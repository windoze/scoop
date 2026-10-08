//! Addressable scalar entries reuse the operations used by direct calls.
use super::*;

impl Lowerer {
    pub(in crate::pipeline) fn lower_intrinsic_entry(
        &mut self,
        module: &hir::Module,
        source: hir::FunctionId,
        function: mir::FunctionId,
        intrinsic: scoop_hir::IntrinsicFunctionKind,
    ) -> (Vec<mir::Param>, mir::Type, smir::Body) {
        let declaration = &module.functions[source];
        let member = intrinsic
            .equality_member()
            .expect("addressable scalar entries have a typed equality operation");
        let mut body = self.body_lowerer(
            module,
            function,
            declaration.materialization,
            mir::ImmortalObjectOwner::Callable(declaration.materialization),
        );
        let mut params = Vec::new();
        let mut arguments = Vec::new();
        for (index, parameter) in declaration.params.iter().enumerate() {
            let ty = body.lower_type(parameter.ty);
            let local = body.locals.alloc(mir::Local {
                name: parameter.name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            let selector = if index == 0 {
                hir::LocalValueSelector::This
            } else {
                hir::LocalValueSelector::Parameter {
                    declaration_index: (index - 1) as u32,
                }
            };
            let identity = hir::CborIdentityRecord::from_key(hir::LocalValueKey::new(
                declaration.materialization,
                selector,
            ))
            .expect("intrinsic method parameters have canonical value identities");
            body.local_values.record(function, local, &identity);
            params.push(mir::Param {
                name: parameter.name.clone(),
                ty: ty.clone(),
                local,
            });
            arguments.push(smir::Expr::local(local, ty));
        }
        let result = body.lower_type(declaration.return_ty);
        let value =
            body.lower_primitive_member_values(member, arguments, &result, declaration.span);
        (
            params,
            result,
            smir::Body {
                locals: body.locals,
                statements: vec![smir::Statement {
                    kind: smir::StatementKind::Return { value: Some(value) },
                    span: declaration.span,
                }],
                coroutine_eh: None,
            },
        )
    }
}
