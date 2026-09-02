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
mod m2;
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
