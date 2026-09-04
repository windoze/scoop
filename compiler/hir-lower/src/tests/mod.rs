//! Test builders (M4 AST) and the M1 test suite, adapted to the M4
//! AST/HIR contracts and the multi-file `lower` entry point.
//!
//! Every test compiles the user file together with a minimal
//! `scoop.core` (`core_file()`: the `Option<T>` enum, the `Throwable`
//! exception root, the M10 coroutine protocol, plus the M7 `io.scoop`
//! overloads and their backing intrinsic), mirroring the driver's
//! sysroot convention — core files first, the user file last.

mod basics;
mod builders;
mod core;
mod m10;
mod m11;
mod m12;
mod m14;
mod m17;
mod m18;
mod m19;
mod m2;
mod m20;
mod m21_consts;
mod m21_delegates;
mod m21_interfaces;
mod m21_properties;
mod m21_top_level_storage;
mod m21_visibility;
mod m3;
mod m4;
mod m5;
mod m6;
mod m7;
mod m8;
mod m9;

use super::*;
use ast::{
    BinOp, Block, CallExpr, Decl, Expr, FieldAccess, FieldDecl, FieldSelector, FunctionBody,
    FunctionDecl, Ident, Param, SourceFile, Statement, StatementKind, StructDecl as AstStructDecl,
    TypeRef, TypeRefKind, UnOp, ValDecl, VariantDecl, VariantDeclKind, VariantFieldDecl,
};

pub(crate) use builders::*;
pub(crate) use core::*;

fn binding_local(pattern: &hir::Pattern) -> Option<hir::LocalId> {
    match pattern {
        hir::Pattern::Binding { local } => Some(*local),
        _ => None,
    }
}

fn local_init<'body>(body: &'body hir::Body, name: &str) -> &'body hir::Expr {
    body.statements
        .iter()
        .find_map(|statement| {
            let hir::StatementKind::ValDecl { pattern, init } = &statement.kind else {
                return None;
            };
            let local = binding_local(pattern)?;
            (body.locals[local].name == name).then_some(init)
        })
        .unwrap_or_else(|| panic!("local `{name}` must have an initializer"))
}

fn expression_statement(body: &hir::Body, index: usize) -> &hir::Expr {
    body.statements
        .iter()
        .filter_map(|statement| match &statement.kind {
            hir::StatementKind::Expr(expr) => Some(expr),
            _ => None,
        })
        .nth(index)
        .unwrap_or_else(|| panic!("source expression statement {index} must exist"))
}

fn return_value(statements: &[hir::Statement]) -> &hir::Expr {
    statements
        .iter()
        .rev()
        .find_map(|statement| match &statement.kind {
            hir::StatementKind::Return { value: Some(value) } => Some(value),
            _ => None,
        })
        .expect("a return with a value must exist")
}
