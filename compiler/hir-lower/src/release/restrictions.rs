use super::*;

impl Lowerer {
    pub(crate) fn reject_release_expression(&mut self, expression: &ast::Expr) -> bool {
        if self.current_release.is_none() {
            return false;
        }
        let operation = match expression {
            ast::Expr::This { .. } => "`this`",
            ast::Expr::SuperMethodCall { .. }
            | ast::Expr::QualifiedInterfaceSuperAccess { .. }
            | ast::Expr::QualifiedInterfaceSuperMethodCall { .. } => "`super`",
            ast::Expr::Lambda { .. } => "a lambda",
            ast::Expr::AnonymousFunction { .. } => "an anonymous function",
            ast::Expr::CallableReference { .. } => "a callable reference",
            ast::Expr::Try(_) => "`try`",
            _ => return false,
        };
        self.error(
            expression.span(),
            format!("{operation} is not allowed in a `release` block"),
        );
        true
    }

    pub(crate) fn reject_release_statement(&mut self, statement: &ast::Statement) -> bool {
        if self.current_release.is_none() {
            return false;
        }
        let operation = match statement.kind {
            ast::StatementKind::Return { .. } => "`return`",
            ast::StatementKind::Throw(_) => "`throw`",
            ast::StatementKind::Try(_) => "`try`",
            ast::StatementKind::LocalFunction(_) => "a local function",
            _ => return false,
        };
        self.error(
            statement.span,
            format!("{operation} is not allowed in a `release` block"),
        );
        true
    }
}
