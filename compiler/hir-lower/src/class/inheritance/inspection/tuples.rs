use super::*;

impl Lowerer {
    pub(super) fn inspect_tuple(
        &mut self,
        context: CodingContext,
        elements: &[TypeId],
        value: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        self.inspection_bind("__inspection_tuple", value, span, sink);
        let view = self.inspection_type("ValueView", span)?;
        let mut readers = Vec::new();
        for (index, &ty) in elements.iter().enumerate() {
            readers.push(
                self.lower_synthesized_lambda(false, &[], view, span, |state, _| {
                    let value = state.inspection_capture("__inspection_tuple", span)?;
                    let field = state.coding_expr(
                        hir::ExprKind::FieldAccess {
                            receiver: Box::new(value),
                            field: hir::FieldRef::TupleIndex(index as u32),
                        },
                        ty,
                        span,
                    );
                    let mut statements = Vec::new();
                    let value = state.inspection_read(context, field, span, &mut statements)?;
                    Some(crate::stmt::ValueBlock {
                        statements,
                        value: Some(value),
                    })
                })?,
            );
        }
        let reader_type = self.intern_function_type(false, Vec::new(), view);
        let reader_ty = self.function_types[reader_type].canonical_type;
        let readers = self.inspection_array(reader_ty, readers, span);
        self.inspection_bind("__inspection_elements", readers, span, sink);
        let size = self.inspection_long(elements.len(), span);
        let element =
            self.lower_synthesized_lambda(false, &[size.ty], view, span, |state, parameters| {
                let readers = state.inspection_capture("__inspection_elements", span)?;
                let reader = state.inspection_index(readers, parameters[0].clone(), span);
                let value = state.coding_expr(
                    hir::ExprKind::CallableCall {
                        callee: Box::new(reader),
                        function_type: reader_type,
                        args: Vec::new(),
                    },
                    view,
                    span,
                );
                Some(crate::stmt::ValueBlock {
                    statements: Vec::new(),
                    value: Some(value),
                })
            })?;
        let tuple = self.inspection_construct("TupleView", vec![size, element], span, sink)?;
        self.inspection_variant("ValueShape", "Tuple", vec![tuple], span)
    }
}
