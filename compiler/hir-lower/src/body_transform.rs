//! In-place completion of already typed local callable and nominal bodies.
use crate::Lowerer;
use scoop_hir as hir;
mod expressions;
mod statements;

pub(crate) trait BodyTransform {
    fn expression(&mut self, lowerer: &mut Lowerer, expr: &mut hir::Expr) {
        expressions::walk(self, lowerer, expr);
    }
    fn statement(&mut self, lowerer: &mut Lowerer, statement: &mut hir::Statement) {
        statements::walk(self, lowerer, statement);
    }
    fn statements(&mut self, lowerer: &mut Lowerer, statements: &mut [hir::Statement]) {
        for statement in statements {
            self.statement(lowerer, statement);
        }
    }
    fn pattern(&mut self, lowerer: &mut Lowerer, pattern: &mut hir::Pattern) {
        statements::pattern(self, lowerer, pattern);
    }
}
pub(crate) use expressions::walk as walk_expression;
pub(crate) use statements::walk as walk_statement;
