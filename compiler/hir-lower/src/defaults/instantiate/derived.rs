use super::*;

impl Lowerer {
    /// Substitute the two already evaluated operands into an open derivation.
    /// Reuse the ordinary expression copier so bound targets and type-bearing
    /// operations follow exactly the same rules as exported default bodies.
    pub(crate) fn inline_derived_equality(
        &mut self,
        application: hir::DerivedEqualityApplicationId,
        receiver: hir::Expr,
        other: hir::Expr,
        bindings: Vec<(hir::TypeParamId, hir::TypeId)>,
    ) -> hir::Expr {
        let application = self.derived_equality_applications[application].clone();
        let [
            hir::Statement {
                kind: hir::StatementKind::Return { value: Some(value) },
                ..
            },
        ] = application.body.statements.as_slice()
        else {
            unreachable!("an open equality derivation is a complete short-circuit expression")
        };
        debug_assert_eq!(application.body.locals.len(), 2);
        let mut context = InstantiationContext {
            bindings,
            statement_span: receiver.span,
            locals: vec![receiver, other],
            captures: HashMap::new(),
            loop_targets: Vec::new(),
            evaluation: InstantiationEvaluation::Template,
        };
        self.instantiate_default_expr(value, &mut context)
    }
}
