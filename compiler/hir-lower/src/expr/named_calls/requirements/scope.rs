use super::*;

impl Lowerer {
    pub(super) fn build_member_requirement(
        &mut self,
        contract: &Contract,
    ) -> Result<hir::MemberRequirementImplementation, String> {
        let outer_locals = std::mem::take(&mut self.locals);
        let outer_paths = std::mem::take(&mut self.definition_paths);
        let outer_smart_casts = std::mem::take(&mut self.smart_casts);
        let before = self.diagnostics.len();
        self.push_scope();
        self.push_safety_context(hir::Safety::Unsafe);
        self.push_suspension_context(crate::SuspensionContext::SuspendFunction);
        let span = Span::new(0, 0);
        let mut inputs = Vec::new();
        let receiver = matches!(contract.call, hir::RequiredCallKind::Method { .. }).then(|| {
            let local = self.alloc_hidden("requirement.receiver", contract.subject);
            inputs.push(local);
            hir::Expr {
                kind: ExprKind::Local(local),
                ty: contract.subject,
                span,
                origin: self.expression_origin(span),
            }
        });
        let arguments = contract
            .arguments
            .iter()
            .enumerate()
            .map(|(index, argument)| {
                let name = format!("$requirement.argument.{index}");
                let local = self.alloc_hidden("requirement.argument", argument.ty);
                inputs.push(local);
                self.scopes.declare(name.clone(), local);
                ast::CallArgument {
                    name: argument.name.as_ref().map_or(
                        ast::CallArgumentName::Positional,
                        |name| {
                            ast::CallArgumentName::Named(ast::Ident {
                                text: name.to_owned(),
                                span,
                            })
                        },
                    ),
                    spread: if argument.spread {
                        ast::SpreadSyntax::Spread(span)
                    } else {
                        ast::SpreadSyntax::Plain
                    },
                    expression: ast::Expr::Var(ast::Ident {
                        text: name.to_owned(),
                        span,
                    }),
                    span,
                }
            })
            .collect::<Vec<_>>();
        let mut statements = Vec::new();
        let result =
            self.select_member_requirement(contract, receiver, &arguments, span, &mut statements);
        let locals = std::mem::replace(&mut self.locals, outer_locals).into_arena();
        self.definition_paths = outer_paths;
        self.smart_casts = outer_smart_casts;
        self.pop_suspension_context();
        self.pop_safety_context();
        self.pop_scope();
        self.diagnostics.truncate(before);
        result.map(|result| hir::MemberRequirementImplementation {
            inputs,
            body: hir::Body { locals, statements },
            result,
        })
    }
}
