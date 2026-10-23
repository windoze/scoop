use super::*;

impl Lowerer {
    pub(in crate::expr::callable_references) fn bound_inner_reference_candidates(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        span: Span,
        expected: Option<TypeId>,
    ) -> Result<Vec<ReferenceCandidate>, ()> {
        let expected = self
            .expected_function_signature(expected)
            .map(|(_, signature)| signature.return_type);
        let Some((receiver, owner, expected)) =
            self.bound_inner_constructor(receiver, name, expected)?
        else {
            return Ok(Vec::new());
        };
        let call = ast::CallExpr {
            callee: name.clone(),
            type_args: Vec::new(),
            args: Vec::new(),
            span,
        };
        let plans =
            if let Some(crate::Owner::Class(class)) = self.nominal_owners.get(&owner).copied() {
                self.nominal_constructor_plans(
                    crate::NominalTarget::Class(class),
                    (&call).into(),
                    expected,
                    false,
                )?
            } else {
                self.imported_nominal_plans(owner, (&call).into(), expected, false)
            };
        let mut candidates = self.reference_constructor_plans(plans, name, span)?;
        for candidate in &mut candidates {
            let ReferenceCandidate::Constructor(candidate) = candidate else {
                unreachable!("constructor plans contain constructor references");
            };
            candidate.enclosing = Some(receiver.clone());
        }
        Ok(candidates)
    }
}
