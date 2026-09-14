use super::super::*;
use super::{core_file, gc_api_declarations, make_core_public};

/// Lower a user file together with the minimal `scoop.core`, mirroring
/// the driver's sysroot convention (core files first, user file last).
pub(crate) fn lower_user(user: SourceFile) -> Result<hir::ExportHirOutput, Vec<Diagnostic>> {
    lower(&[core_file(), user]).map(|output| output.export)
}

pub(crate) fn lower_user_output(user: SourceFile) -> Result<hir::Output, Vec<Diagnostic>> {
    lower(&[core_file(), user])
}

// --- M8: exceptions ---

/// `throw expr` (M8).
pub(crate) fn throw_stmt(value: Expr) -> Statement {
    Statement {
        kind: StatementKind::Throw(value),
        span: sp(),
    }
}

/// `try { body } catch... finally...` (M8).
pub(crate) fn try_stmt(
    body: Vec<Statement>,
    catches: Vec<ast::CatchClause>,
    finally_body: Option<Vec<Statement>>,
) -> Statement {
    Statement {
        kind: StatementKind::Try(ast::Try {
            body: block(body),
            catches,
            finally_body: finally_body.map(block),
            span: sp(),
        }),
        span: sp(),
    }
}

/// `catch (name: T) { body }`.
pub(crate) fn catch_clause(name: &str, ty: TypeRef, body: Vec<Statement>) -> ast::CatchClause {
    ast::CatchClause {
        name: ident(name),
        ty,
        body: block(body),
        span: sp(),
    }
}

/// `catch (name: T) { body }` with an explicit clause span (diagnostic
/// position assertions).
pub(crate) fn catch_clause_at(
    name: &str,
    ty: TypeRef,
    body: Vec<Statement>,
    span: Span,
) -> ast::CatchClause {
    ast::CatchClause {
        name: ident(name),
        ty,
        body: block(body),
        span,
    }
}

/// Lower a user file with the full core exception hierarchy available
/// (M8 tests).
pub(crate) fn lower_user_with_exceptions(
    user: SourceFile,
) -> Result<hir::ExportHirOutput, Vec<Diagnostic>> {
    lower(&[core_file(), user]).map(|output| output.export)
}

/// Lower a user file with the core GC facilities available (M9 tests).
pub(crate) fn lower_user_with_gc(
    user: SourceFile,
) -> Result<hir::ExportHirOutput, Vec<Diagnostic>> {
    let mut gc = file(gc_api_declarations());
    make_core_public(&mut gc);
    lower(&[core_file(), gc, user]).map(|output| output.export)
}
