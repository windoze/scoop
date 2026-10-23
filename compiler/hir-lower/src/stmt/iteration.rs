use super::*;
use crate::expr::{CallSite, RequiredCallableModifiers};

mod head;
use head::IterationHead;

impl Lowerer {
    pub(super) fn lower_for(&mut self, source: &ast::For) -> Option<Vec<hir::Statement>> {
        self.with_pattern_transaction(|state| state.lower_for_inner(source))
    }

    fn lower_for_inner(&mut self, source: &ast::For) -> Option<Vec<hir::Statement>> {
        let IterationHead {
            setup: mut statements,
            condition_setup,
            condition: cond,
            element_setup: mut body,
            element,
        } = self.lower_iteration_head(&source.iterable, source.span)?;
        let target = self.fresh_loop();
        self.push_scope();
        let planned = (|| {
            let binding =
                self.lower_irrefutable_binding_from_subject(&source.pattern, element, false)?;
            body.extend(binding);
            self.push_loop(target, source.label.as_ref());
            for statement in &source.body.statements {
                self.lower_statement(statement, &mut body);
            }
            self.pop_loop(target);
            Some(())
        })();
        self.pop_scope();
        planned?;

        statements.push(hir::Statement {
            kind: hir::StatementKind::While {
                loop_kind: hir::LoopKind::While,
                target,
                condition_setup,
                cond,
                body,
            },
            span: source.span,
        });
        Some(statements)
    }
}
