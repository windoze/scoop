//! AST → HIR 的最小 lowering（TODO T0701）。
//!
//! 说明：
//! - 这里的 lowering 仅用于 `dump-hir` 的调试输出，因此实现上优先保证“稳定输出 + 不 panic”；。
//! - 完整 lowering（含类型推断结果、更多语法节点）会在后续任务（TODO T0702+）逐步补齐。

use std::collections::{HashMap, HashSet};

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::parser::{ParseError, parse_file};
use crate::resolve::{Index, ResolveError};
use crate::session::Session;
use crate::source::SourceFile;
use crate::span::Span;
use crate::syntax::string_literal::parse_string_literal_utf8;
use crate::ty::{
    BuiltinTypes, EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeParamType, TypeStore,
    ValueTypeKind,
};

use super::{
    Block, CallArg, Capture, ClosureExpr, ClosureId, EffectOpRef, EnumLayout, EnumLayoutIndex,
    EnumVariantFieldLayout, EnumVariantLayout, Expr, ExprKind, File, FunDecl, HandleArm,
    HandleArmKind, HandleBinder, HandleExpr, HandleOp, Item, LiteralKind, MemberAccess, MemberRef, ObjectInit,
    ObjectInitIndex, ObjectInitStep, ObjectProperty, Param, Stmt, StmtKind, StructFieldLayout,
    StructLayout, StructLayoutIndex, StructLitField, SymbolId, ValDecl, ValueRef, WhenArm, WhenPat,
};

/// HIR lowering 错误（目前仅包装 parser/resolve 错误）。
#[derive(Debug, Error, Diagnostic)]
pub enum HirLowerError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Resolve(#[from] ResolveError),
}

/// 一次 lowering 的产物：HIR + 对应的 `TypeStore`。
///
/// 说明：HIR 节点里的 `TypeId` 仅在同一个 `TypeStore` 里可解码/展示。
#[derive(Debug)]
pub struct LoweredHir {
    pub file: File,
    pub types: TypeStore,
    /// 由本次 lowering 过程中收集到的 struct 字段布局信息（供早期 LLVM codegen 查询）。
    pub struct_layouts: StructLayoutIndex,
    /// 由本次 lowering 过程中收集到的 enum variant 布局信息（供早期 LLVM codegen 查询）。
    pub enum_layouts: EnumLayoutIndex,
    /// `@Extern` 外部函数信息（供 LLVM codegen 声明正确的符号名与 ABI）。
    pub extern_funs: super::ExternFunIndex,
    /// `object` / `companion object` 的初始化信息（供早期 LLVM codegen 查询）。
    pub object_inits: ObjectInitIndex,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum SymbolKey {
    Local { decl_span: crate::span::Span },
    TopLevel { fqn: String },
}

/// 一个最小的 symbol interner：把“解析后的符号键”映射为一个紧凑的 `SymbolId`。
///
/// 说明：
/// - 该表仅用于 HIR dump/fixtures 的稳定标识，并不试图提供跨 session 的全局稳定性；
/// - `SymbolId` 的分配顺序依赖 traversal 顺序，但 traversal 对同一个 AST 是确定的，因此 golden 可回归。
#[derive(Debug, Default)]
struct SymbolInterner {
    next: u32,
    by_key: HashMap<SymbolKey, SymbolId>,
}

impl SymbolInterner {
    fn intern_local(&mut self, decl_span: crate::span::Span) -> SymbolId {
        self.intern(SymbolKey::Local { decl_span })
    }

    fn intern_top_level(&mut self, fqn: String) -> SymbolId {
        self.intern(SymbolKey::TopLevel { fqn })
    }

    fn intern(&mut self, key: SymbolKey) -> SymbolId {
        if let Some(id) = self.by_key.get(&key).copied() {
            return id;
        }

        let id = SymbolId(self.next);
        self.next = self.next.saturating_add(1);
        self.by_key.insert(key, id);
        id
    }
}

/// HIR lowering 的上下文（按单文件构建，用于 `dump-hir` 与 HIR fixtures）。
struct HirLowering<'a> {
    source: &'a SourceFile,
    file: &'a ast::File,
    index: &'a Index,
    /// `type fqn -> ast::TypeKind` 的最小索引，用于决定 nominal type 是 ref 还是 value。
    type_kinds: &'a HashMap<String, ast::TypeKind>,
    symbols: SymbolInterner,
    /// 本文件内的“局部 symbol → 是否可变（var）”信息。
    ///
    /// 用途：closure capture set 需要知道捕获目标是否为 `var`，以便在后续 MIR lowering（T0714）
    /// 侧把可变捕获降为 box/alias 语义。
    local_mutability: HashMap<SymbolId, bool>,
    next_closure: u32,
    /// 类型表（HIR 内所有 `TypeId` 必须来自同一个 store）。
    types: &'a mut TypeStore,
    builtins: BuiltinTypes,
    /// type parameter 作用域栈：用于 lowering `T` 这类抽象类型引用。
    type_param_scopes: Vec<HashMap<String, TypeId>>,
}

impl<'a> HirLowering<'a> {
    const ASYNC_AWAIT_FQN: &'static str = "scoop.core.Async.await";
    const TASK_SPAWN_INT_FQN: &'static str = "scoop.core.__scoop_task_spawn_int";
    const TASK_JOIN_INT_FQN: &'static str = "scoop.core.__scoop_task_join_int";

    fn new(
        source: &'a SourceFile,
        file: &'a ast::File,
        index: &'a Index,
        type_kinds: &'a HashMap<String, ast::TypeKind>,
        types: &'a mut TypeStore,
        builtins: BuiltinTypes,
    ) -> Self {
        Self {
            source,
            file,
            index,
            type_kinds,
            symbols: SymbolInterner::default(),
            local_mutability: HashMap::new(),
            next_closure: 0,
            types,
            builtins,
            type_param_scopes: Vec::new(),
        }
    }

    fn intern_local_symbol(&mut self, decl_span: Span, mutable: bool) -> SymbolId {
        let id = self.symbols.intern_local(decl_span);
        match self.local_mutability.get(&id).copied() {
            // 同一 decl_span 不应出现冲突的 mutability，但为了降低与 resolver 交互时的脆弱性：
            // - 若任一方认为它是 `var`，则提升为 `var`；
            // - 否则保持 `val`。
            Some(prev) => {
                let _ = self.local_mutability.insert(id, prev || mutable);
            }
            None => {
                self.local_mutability.insert(id, mutable);
            }
        }
        id
    }

    fn lower_file(&mut self) -> File {
        let pkg_prefix = package_prefix(self.source, self.file.package.as_ref());
        let mut items = Vec::with_capacity(self.file.items.len());

        for item in &self.file.items {
            items.push(self.lower_item(&pkg_prefix, item));
        }

        File { items }
    }

    fn lower_item(&mut self, pkg_prefix: &str, item: &ast::Item) -> Item {
        match item {
            ast::Item::Fun(fun) => Item::Fun(self.lower_fun_decl(pkg_prefix, fun)),
            ast::Item::Val(v) => Item::Val(self.lower_val_decl(pkg_prefix, v, ValScope::TopLevel)),
            ast::Item::TypeAlias(ta) => Item::Todo {
                span: ta.span,
                kind: "typealias",
            },
            ast::Item::Type(ty) => Item::Todo {
                span: ty.span,
                kind: "type",
            },
            ast::Item::Object(obj) => Item::Todo {
                span: obj.span,
                kind: "object",
            },
            ast::Item::ExtensionProperty(p) => Item::Todo {
                span: p.span,
                kind: "extension_property",
            },
        }
    }

    fn lower_fun_decl(&mut self, pkg_prefix: &str, fun: &ast::FunDecl) -> FunDecl {
        // 进入函数作用域：先把 type params lower 成 `TypeId`，保证签名与 body 内引用一致。
        self.push_type_params(&fun.type_params);

        let name = fun.name.text(self.source).to_string();
        let fqn = if pkg_prefix.is_empty() {
            name.clone()
        } else {
            format!("{pkg_prefix}.{name}")
        };

        let params: Vec<Param> = fun
            .params
            .iter()
            .map(|p| {
                let name = p.name.text(self.source).to_string();
                let id = self.intern_local_symbol(p.name.span, false);
                let ty =
                    p.ty.as_ref()
                        .map(|t| self.lower_type_ref(t))
                        .unwrap_or(self.builtins.any);
                Param {
                    span: p.name.span,
                    id,
                    name,
                    ty,
                }
            })
            .collect();

        let receiver_ty = fun.receiver.as_ref().map(|t| self.lower_type_ref(t));

        // 当前阶段：未接入返回类型推断，缺省时用 `Any` 占位。
        //
        // T0623：`async fun foo(): T` 对外暴露 `Task<T>`：
        // - 早期 HIR 里 task 先用 word-sized `UInt` 句柄承载（与 `spawn/join` 一致）；
        // - 真正的 `Task<T>` nominal type lowering 与 ABI 会在后续任务中补齐。
        let is_async_fun = fun.modifiers.contains(&ast::Modifier::Async);
        let _inner_return_ty = fun
            .return_ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .unwrap_or(self.builtins.any);
        let return_ty = if is_async_fun {
            self.builtins.uint
        } else {
            _inner_return_ty
        };

        let effects = self.lower_effect_row_expr(fun.effects.as_ref());
        let ty = self.types.ty_function(
            receiver_ty,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            effects,
        );

        let body = match &fun.body {
            ast::FunBody::Block(b) => {
                let lowered = self.lower_block(pkg_prefix, b);
                if is_async_fun {
                    Some(self.rewrite_async_fun_block(lowered, true))
                } else {
                    Some(lowered)
                }
            }
            ast::FunBody::Missing => None,
        };

        self.pop_type_params();

        FunDecl {
            span: fun.span,
            fqn,
            name,
            ty,
            params,
            return_ty,
            body,
        }
    }

    /// 在“已绑定 type params”的语境下降低一个函数声明。
    ///
    /// 用途：
    /// - 单态化（monomorphization）生成具体实例：把 `T` 等 type param 直接映射到具体 `TypeId`
    ///   后再构造 HIR（避免再次生成 `TypeKind::Param`）。
    fn lower_fun_decl_with_bound_type_params(
        &mut self,
        pkg_prefix: &str,
        fun: &ast::FunDecl,
    ) -> FunDecl {
        let name = fun.name.text(self.source).to_string();
        let fqn = if pkg_prefix.is_empty() {
            name.clone()
        } else {
            format!("{pkg_prefix}.{name}")
        };

        let params: Vec<Param> = fun
            .params
            .iter()
            .map(|p| {
                let name = p.name.text(self.source).to_string();
                let id = self.intern_local_symbol(p.name.span, false);
                let ty =
                    p.ty.as_ref()
                        .map(|t| self.lower_type_ref(t))
                        .unwrap_or(self.builtins.any);
                Param {
                    span: p.name.span,
                    id,
                    name,
                    ty,
                }
            })
            .collect();

        let receiver_ty = fun.receiver.as_ref().map(|t| self.lower_type_ref(t));

        // 当前阶段：未接入返回类型推断，缺省时用 `Any` 占位。
        //
        // T0623：monomorph/hir 视图下同样把 `async fun` 的返回类型降为 task handle（`UInt`）。
        let is_async_fun = fun.modifiers.contains(&ast::Modifier::Async);
        let _inner_return_ty = fun
            .return_ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .unwrap_or(self.builtins.any);
        let return_ty = if is_async_fun {
            self.builtins.uint
        } else {
            _inner_return_ty
        };

        let effects = self.lower_effect_row_expr(fun.effects.as_ref());
        let ty = self.types.ty_function(
            receiver_ty,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            effects,
        );

        let body = match &fun.body {
            ast::FunBody::Block(b) => {
                let lowered = self.lower_block(pkg_prefix, b);
                if is_async_fun {
                    Some(self.rewrite_async_fun_block(lowered, true))
                } else {
                    Some(lowered)
                }
            }
            ast::FunBody::Missing => None,
        };

        FunDecl {
            span: fun.span,
            fqn,
            name,
            ty,
            params,
            return_ty,
            body,
        }
    }

    fn rewrite_async_fun_block(&mut self, mut block: Block, wrap_tail_expr: bool) -> Block {
        // T0623：把 `async fun` 的返回值包装成 task 句柄：
        // - `return expr` → `return __scoop_task_spawn_int(expr)`（early stage 仅支持 Int）；
        // - block tail expr（隐式返回）同样做一次包装。

        for stmt in &mut block.stmts {
            match &mut stmt.kind {
                StmtKind::While { body, .. } => {
                    // 这里用 `replace` 把 body move 出来，避免对 Block 增加 Default 约束。
                    let placeholder = Block {
                        span: body.span,
                        ty: body.ty,
                        stmts: Vec::new(),
                    };
                    let old = std::mem::replace(body, placeholder);
                    *body = self.rewrite_async_fun_block(old, false);
                }
                StmtKind::Return { value } => {
                    if let Some(v) = value.take() {
                        *value = Some(self.wrap_task_spawn_int_call(stmt.span, v));
                    }
                }
                _ => {}
            }
        }

        if wrap_tail_expr {
            // 隐式返回：若 block 末尾是表达式语句，则将其值包装为 task handle。
            if let Some(last) = block.stmts.last_mut() {
                if let StmtKind::Expr(expr) = &mut last.kind {
                    // 用占位符把 expr move 出来，避免对 Expr 增加 Default 约束。
                    let expr_span = expr.span;
                    let expr_ty = expr.ty;
                    let old = std::mem::replace(
                        expr,
                        Expr {
                            span: expr_span,
                            ty: expr_ty,
                            kind: ExprKind::Missing,
                        },
                    );
                    *expr = self.wrap_task_spawn_int_call(expr_span, old);
                }
            }
        }

        // 重新计算 block 类型：保持与 `lower_block` 的规则一致。
        block.ty = block
            .stmts
            .last()
            .and_then(|s| match &s.kind {
                StmtKind::Expr(e) => Some(e.ty),
                _ => None,
            })
            .unwrap_or(self.builtins.unit);

        block
    }

    fn wrap_task_spawn_int_call(&mut self, at: Span, value: Expr) -> Expr {
        // `__scoop_task_spawn_int(value)` → task handle (`UInt`)。
        let fqn = Self::TASK_SPAWN_INT_FQN.to_string();
        let callee = Expr {
            span: at,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn,
            }),
        };

        Expr {
            span: at,
            ty: self.builtins.uint,
            kind: ExprKind::Call {
                callee: Box::new(callee),
                args: vec![CallArg::Positional(value)],
            },
        }
    }

    fn lower_val_decl(&mut self, pkg_prefix: &str, v: &ast::ValDecl, scope: ValScope) -> ValDecl {
        let init = v.init.as_ref().map(|e| self.lower_expr(pkg_prefix, e));

        let declared_ty = v.ty.as_ref().map(|t| self.lower_type_ref(t));

        let ty = declared_ty
            .or_else(|| init.as_ref().map(|e| e.ty))
            .unwrap_or(self.builtins.any);

        let (id, name) = match v.name() {
            Some(id) => {
                let name = id.text(self.source).to_string();
                let sym = match scope {
                    ValScope::TopLevel => {
                        let fqn = if pkg_prefix.is_empty() {
                            name.clone()
                        } else {
                            format!("{pkg_prefix}.{name}")
                        };
                        self.symbols.intern_top_level(fqn)
                    }
                    ValScope::Local => {
                        self.intern_local_symbol(id.span, v.kind == ast::ValKind::Var)
                    }
                };
                (Some(sym), Some(name))
            }
            None => (None, None),
        };

        ValDecl {
            span: v.span,
            id,
            name,
            mutable: v.kind == ast::ValKind::Var,
            ty,
            init,
        }
    }

    fn lower_block(&mut self, pkg_prefix: &str, b: &ast::Block) -> Block {
        let mut stmts = Vec::with_capacity(b.stmts.len());
        for s in &b.stmts {
            stmts.push(self.lower_stmt(pkg_prefix, s));
        }

        // 当前阶段：用 block 最后一条“表达式语句”的类型作为 block 类型，否则视为 Unit。
        let ty = stmts
            .last()
            .and_then(|s| match &s.kind {
                StmtKind::Expr(e) => Some(e.ty),
                _ => None,
            })
            .unwrap_or(self.builtins.unit);

        Block {
            span: b.span,
            ty,
            stmts,
        }
    }

    fn lower_stmt(&mut self, pkg_prefix: &str, s: &ast::Stmt) -> Stmt {
        let (kind, ty) = match &s.kind {
            ast::StmtKind::Empty => (StmtKind::Empty, self.builtins.unit),
            ast::StmtKind::Expr(e) => {
                // 赋值在 AST 中以表达式节点承载，但在 HIR 中视为语句（便于后续 MIR lowering）。
                if let ast::ExprKind::Assign { lhs, eq_span, rhs } = &e.kind {
                    let lhs = self.lower_expr(pkg_prefix, lhs);
                    let rhs = self.lower_expr(pkg_prefix, rhs);
                    (
                        StmtKind::Assign {
                            lhs,
                            eq_span: *eq_span,
                            rhs,
                        },
                        self.builtins.unit,
                    )
                } else {
                    let e = self.lower_expr(pkg_prefix, e);
                    (StmtKind::Expr(e), self.builtins.unit)
                }
            }
            ast::StmtKind::Val(v) => {
                let v = self.lower_val_decl(pkg_prefix, v, ValScope::Local);
                (StmtKind::Val(v), self.builtins.unit)
            }
            ast::StmtKind::Return { value, .. } => {
                let value = value.as_ref().map(|e| self.lower_expr(pkg_prefix, e));
                (StmtKind::Return { value }, self.builtins.nothing)
            }
            ast::StmtKind::Missing => (StmtKind::Todo("missing_stmt"), self.builtins.unit),
            ast::StmtKind::While { cond, body, .. } => (
                StmtKind::While {
                    cond: self.lower_expr(pkg_prefix, cond),
                    body: self.lower_block(pkg_prefix, body),
                },
                self.builtins.unit,
            ),
            ast::StmtKind::Break { break_span } => (
                StmtKind::Break {
                    break_span: *break_span,
                },
                self.builtins.unit,
            ),
            ast::StmtKind::Continue { continue_span } => (
                StmtKind::Continue {
                    continue_span: *continue_span,
                },
                self.builtins.unit,
            ),
            ast::StmtKind::ComptimeBlock { .. } => {
                (StmtKind::Todo("comptime_block"), self.builtins.unit)
            }
            ast::StmtKind::ComptimeIf(_) => (StmtKind::Todo("comptime_if"), self.builtins.unit),
            ast::StmtKind::ComptimeFor(_) => (StmtKind::Todo("comptime_for"), self.builtins.unit),
        };

        Stmt {
            span: s.span,
            ty,
            kind,
        }
    }

    fn lower_expr(&mut self, pkg_prefix: &str, e: &ast::Expr) -> Expr {
        let (kind, ty) = match &e.kind {
            ast::ExprKind::Missing => (ExprKind::Missing, self.builtins.any),
            ast::ExprKind::IntLit => (ExprKind::Literal(LiteralKind::Int), self.builtins.int),
            ast::ExprKind::StringLit => {
                (ExprKind::Literal(LiteralKind::String), self.builtins.string)
            }
            ast::ExprKind::UnitLit => (ExprKind::Literal(LiteralKind::Unit), self.builtins.unit),
            ast::ExprKind::InterpolatedString { raw, parts } => {
                let parts = parts
                    .iter()
                    .map(|p| match p {
                        ast::InterpolatedStringPart::Text { span } => {
                            super::InterpolatedStringPart::Text { span: *span }
                        }
                        ast::InterpolatedStringPart::Expr { expr } => {
                            super::InterpolatedStringPart::Expr {
                                expr: self.lower_expr(pkg_prefix, expr),
                            }
                        }
                    })
                    .collect();
                (
                    ExprKind::InterpolatedString { raw: *raw, parts },
                    self.builtins.string,
                )
            }
            ast::ExprKind::Ident(id) => self.lower_ident_expr(id),
            ast::ExprKind::Block(b) => {
                let b = self.lower_block(pkg_prefix, b);
                let ty = b.ty;
                (ExprKind::Block(b), ty)
            }
            ast::ExprKind::UnsafeBlock { body, .. } => {
                // `@Unsafe { ... }` 仅影响 typecheck 的 unsafe context，
                // 在 HIR/codegen 层面当前可按普通 block 表达式处理（T1004）。
                let b = self.lower_block(pkg_prefix, body);
                let ty = b.ty;
                (ExprKind::Block(b), ty)
            }
            ast::ExprKind::Call { callee, args } => {
                if let Some((kind, ty)) =
                    self.try_lower_effect_op_call_expr(pkg_prefix, callee, args)
                {
                    (kind, ty)
                } else {
                    let callee = Box::new(self.lower_expr(pkg_prefix, callee));
                    let args = args
                        .iter()
                        .map(|arg| self.lower_call_arg(pkg_prefix, arg))
                        .collect();
                    (ExprKind::Call { callee, args }, self.builtins.any)
                }
            }
            ast::ExprKind::NamedArg { .. } => (ExprKind::Todo("named_arg"), self.builtins.any),
            ast::ExprKind::TupleLit { elements } => {
                let elements: Vec<Expr> = elements
                    .iter()
                    .map(|e| self.lower_expr(pkg_prefix, e))
                    .collect();
                let ty = if elements.is_empty() {
                    self.builtins.unit
                } else {
                    self.types.ty_tuple(elements.iter().map(|e| e.ty).collect())
                };
                (ExprKind::TupleLit { elements }, ty)
            }
            ast::ExprKind::Lambda(lam) => self.lower_lambda_expr(pkg_prefix, e.span, lam),
            ast::ExprKind::StructLit { ty, fields } => {
                self.lower_struct_lit_expr(pkg_prefix, e.span, ty, fields)
            }
            ast::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond = Box::new(self.lower_expr(pkg_prefix, cond));
                let then_branch = Box::new(self.lower_expr(pkg_prefix, then_branch));
                let else_branch = else_branch
                    .as_ref()
                    .map(|e| Box::new(self.lower_expr(pkg_prefix, e)));
                (
                    ExprKind::If {
                        cond,
                        then_branch,
                        else_branch,
                    },
                    self.builtins.any,
                )
            }
            ast::ExprKind::When { subject, arms } => {
                let subject = Box::new(self.lower_expr(pkg_prefix, subject));
                let arms = arms
                    .iter()
                    .map(|a| self.lower_when_arm(pkg_prefix, a))
                    .collect();
                (ExprKind::When { subject, arms }, self.builtins.any)
            }
            ast::ExprKind::Handle {
                body,
                arms,
                finally,
            } => {
                let handle = self.lower_handle_expr(pkg_prefix, body, arms, finally.as_ref());
                (ExprKind::Handle(handle), self.builtins.any)
            }
            ast::ExprKind::Async { body } => {
                // T0619：`async { ... }` 是 `Async` effect 的语法糖。
                //
                // 当前阶段（最小可回归落点）：
                // - 直接 lower 为一个 `handle` 表达式（immediate-resume arm），拦截 `Async.await`；
                // - handler 的语义为“同步 join”：`await task` 会调用 runtime helper 取回 `Int` 结果并立即恢复；
                // - 更完整的 executor/跨线程 resume 语义留给后续任务（T0917）。

                let body = self.lower_block(pkg_prefix, body);

                // synth：构造 `scoop.core.Async` 的 TypePath（不依赖 import）。
                let synth_span = e.span;
                let effect_path = ast::TypePath {
                    span: synth_span,
                    segments: vec![
                        ast::Ident::synthetic(synth_span, "scoop"),
                        ast::Ident::synthetic(synth_span, "core"),
                        ast::Ident::synthetic(synth_span, "Async"),
                    ],
                    args: Vec::new(),
                };
                let effect_ty = self.lower_type_path(&effect_path);

                // 为避免与真实源码 binding span 冲突，这里为 binder/resume 分配两个零长度 span。
                let binder_decl_span = Span::new(e.span.start, e.span.start);
                let resume_decl_span = Span::new(e.span.end, e.span.end);

                let binder_id = self.intern_local_symbol(binder_decl_span, false);
                let resume_id = self.intern_local_symbol(resume_decl_span, false);

                let binder_name = "value".to_string();
                let resume_name = "resume".to_string();

                let binder = HandleBinder {
                    span: binder_decl_span,
                    id: binder_id,
                    name: binder_name.clone(),
                    // NOTE: 当前阶段 Task 运行期表示为 word-sized handle；这里用 `UInt` 承载它。
                    ty: self.builtins.uint,
                };

                let op = HandleOp {
                    span: e.span,
                    effect_ty,
                    op: EffectOpRef {
                        span: e.span,
                        fqn: Self::ASYNC_AWAIT_FQN.to_string(),
                    },
                    binders: vec![binder],
                };

                let resume_ref = ValueRef::Local {
                    id: resume_id,
                    name: resume_name.clone(),
                    decl_span: resume_decl_span,
                };
                let binder_ref = ValueRef::Local {
                    id: binder_id,
                    name: binder_name.clone(),
                    decl_span: binder_decl_span,
                };

                let resume_callee = Expr {
                    span: resume_decl_span,
                    ty: self.builtins.any,
                    kind: ExprKind::VarRef(resume_ref),
                };
                let binder_arg = Expr {
                    span: binder_decl_span,
                    ty: self.builtins.uint,
                    kind: ExprKind::VarRef(binder_ref),
                };

                // `await task` 的最小可执行语义：调用 sysroot task helper 取回结果（目前只支持 `Int`）。
                let join_fqn = Self::TASK_JOIN_INT_FQN.to_string();
                let join_callee = Expr {
                    span: binder_decl_span,
                    ty: self.builtins.any,
                    kind: ExprKind::VarRef(ValueRef::TopLevel {
                        id: self.symbols.intern_top_level(join_fqn.clone()),
                        fqn: join_fqn,
                    }),
                };
                let join_call = Expr {
                    span: binder_decl_span,
                    ty: self.builtins.int,
                    kind: ExprKind::Call {
                        callee: Box::new(join_callee),
                        args: vec![CallArg::Positional(binder_arg)],
                    },
                };

                let arm_body = Expr {
                    span: e.span,
                    ty: self.builtins.unit,
                    kind: ExprKind::Call {
                        callee: Box::new(resume_callee),
                        args: vec![CallArg::Positional(join_call)],
                    },
                };

                let arm = HandleArm {
                    span: e.span,
                    op,
                    kind: HandleArmKind::ImmediateResume { resume: resume_id },
                    body: arm_body,
                };

                let handle = HandleExpr {
                    body,
                    arms: vec![arm],
                    finally: None,
                };
                (ExprKind::Handle(handle), self.builtins.any)
            }
            ast::ExprKind::Spawn { body } => {
                // T0620：`spawn { ... }`（结构化并发最小模型）。
                //
                // 当前阶段（最小可回归落点）：
                // - `spawn` 的 body 先按普通 block 表达式执行并产出一个 `Int` 值；
                // - 该值通过 runtime helper 包装为一个 task handle（后续可替换为更完整的 `Task<T>` 模型）。
                //
                // NOTE: 这里刻意不使用 lambda/closure，以避免依赖 closure codegen（尚未接入）。

                let body = self.lower_block(pkg_prefix, body);
                let body_expr = Expr {
                    span: body.span,
                    ty: body.ty,
                    kind: ExprKind::Block(body),
                };

                let fqn = Self::TASK_SPAWN_INT_FQN.to_string();
                let callee = Expr {
                    span: e.span,
                    ty: self.builtins.any,
                    kind: ExprKind::VarRef(ValueRef::TopLevel {
                        id: self.symbols.intern_top_level(fqn.clone()),
                        fqn,
                    }),
                };

                (
                    ExprKind::Call {
                        callee: Box::new(callee),
                        args: vec![CallArg::Positional(body_expr)],
                    },
                    // task handle：用 word-sized `UInt` 表示。
                    self.builtins.uint,
                )
            }
            ast::ExprKind::Await { await_span, expr } => {
                // T0619：`await expr`（async/await）作为 `Async.await(...)` 的语法糖。
                //
                // NOTE: 这里直接 lower 为 HIR `Perform`，不依赖 resolver 对 `Async.await` 的成员解析写回；
                // 这样能避免“语法糖节点需要合成表达式 ident”的复杂度。
                let inner = self.lower_expr(pkg_prefix, expr);
                let op = EffectOpRef {
                    span: *await_span,
                    fqn: Self::ASYNC_AWAIT_FQN.to_string(),
                };
                (
                    ExprKind::Perform {
                        op,
                        args: vec![CallArg::Positional(inner)],
                    },
                    self.builtins.int,
                )
            }
            ast::ExprKind::Join { join_span, expr } => {
                // T0620：`join expr`（结构化并发最小模型）。
                //
                // 当前阶段：join 仅支持 `Int` 句柄，并返回 `Int`（后续会替换为 `await Task<T>`）。
                let inner = self.lower_expr(pkg_prefix, expr);

                let fqn = Self::TASK_JOIN_INT_FQN.to_string();
                let callee = Expr {
                    span: *join_span,
                    ty: self.builtins.any,
                    kind: ExprKind::VarRef(ValueRef::TopLevel {
                        id: self.symbols.intern_top_level(fqn.clone()),
                        fqn,
                    }),
                };

                (
                    ExprKind::Call {
                        callee: Box::new(callee),
                        args: vec![CallArg::Positional(inner)],
                    },
                    self.builtins.int,
                )
            }
            ast::ExprKind::MemberAccess { receiver, member } => {
                self.lower_member_access_expr(pkg_prefix, receiver, member)
            }
            ast::ExprKind::SpliceField { .. } => {
                (ExprKind::Todo("splice_field"), self.builtins.any)
            }
            ast::ExprKind::SafeMemberAccess { .. } => {
                (ExprKind::Todo("safe_member_access"), self.builtins.any)
            }
            ast::ExprKind::NotNullAssert { .. } => {
                (ExprKind::Todo("not_null_assert"), self.builtins.any)
            }
            ast::ExprKind::Unary { op, op_span, expr } => {
                let expr = Box::new(self.lower_expr(pkg_prefix, expr));
                let ty = match op {
                    ast::UnaryOp::Not => {
                        if expr.ty == self.builtins.bool_ {
                            self.builtins.bool_
                        } else {
                            self.builtins.any
                        }
                    }
                    ast::UnaryOp::Neg | ast::UnaryOp::BitNot => {
                        if self.is_integer_type(expr.ty) {
                            expr.ty
                        } else {
                            self.builtins.any
                        }
                    }
                };
                (
                    ExprKind::Unary {
                        op: *op,
                        op_span: *op_span,
                        expr,
                    },
                    ty,
                )
            }
            ast::ExprKind::Binary {
                lhs,
                op,
                op_span,
                rhs,
            } => {
                let lhs = Box::new(self.lower_expr(pkg_prefix, lhs));
                let rhs = Box::new(self.lower_expr(pkg_prefix, rhs));
                let ty = self.lower_binary_expr_type(&lhs, &rhs, *op);
                (
                    ExprKind::Binary {
                        lhs,
                        op: *op,
                        op_span: *op_span,
                        rhs,
                    },
                    ty,
                )
            }
            ast::ExprKind::Assign { .. } => (ExprKind::Todo("assign"), self.builtins.any),
            ast::ExprKind::TypeCheck { .. } => (ExprKind::Todo("type_check"), self.builtins.any),
            ast::ExprKind::Cast { .. } => (ExprKind::Todo("cast"), self.builtins.any),
            ast::ExprKind::WithUpdate { .. } => (ExprKind::Todo("with_update"), self.builtins.any),
        };

        Expr {
            span: e.span,
            ty,
            kind,
        }
    }

    fn alloc_closure_id(&mut self) -> ClosureId {
        let id = ClosureId(self.next_closure);
        self.next_closure = self.next_closure.saturating_add(1);
        id
    }

    fn lower_lambda_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        lam: &ast::LambdaExpr,
    ) -> (ExprKind, TypeId) {
        let id = self.alloc_closure_id();

        let params: Vec<Param> = lam
            .params
            .iter()
            .map(|p| {
                let name = p.name.text(self.source).to_string();
                let ty =
                    p.ty.as_ref()
                        .map(|t| self.lower_type_ref(t))
                        .unwrap_or(self.builtins.any);
                Param {
                    span: p.name.span,
                    id: self.intern_local_symbol(p.name.span, false),
                    name,
                    ty,
                }
            })
            .collect();

        let body = Box::new(self.lower_expr(pkg_prefix, lam.body.as_ref()));
        let captures = compute_closure_captures(&params, body.as_ref(), &self.local_mutability);
        (
            ExprKind::Closure(ClosureExpr {
                span,
                id,
                captures,
                params,
                body,
            }),
            self.builtins.any,
        )
    }

    /// 把 AST 的 struct literal（`Type { field: expr, ... }`）降低为 HIR 表示。
    ///
    /// 说明：
    /// - 当前 lowering 不做字段存在性/类型匹配检查（这些属于 typecheck，参见 TODO T0423）；
    /// - 这里只保留“目标类型 + 字段初始化表达式列表”，供早期 LLVM codegen（T0811）构造值。
    fn lower_struct_lit_expr(
        &mut self,
        pkg_prefix: &str,
        _span: Span,
        ty: &ast::TypePath,
        fields: &[ast::StructLitField],
    ) -> (ExprKind, TypeId) {
        let ty_id = self.lower_type_path(ty);

        let lowered_fields = fields
            .iter()
            .map(|f| StructLitField {
                span: f.span,
                name: f.name.text(self.source).to_string(),
                name_span: f.name.span,
                colon_span: f.colon_span,
                value: self.lower_expr(pkg_prefix, &f.value),
            })
            .collect::<Vec<_>>();

        (
            ExprKind::StructLit {
                ty: ty_id,
                fields: lowered_fields,
            },
            ty_id,
        )
    }

    fn lower_member_access_expr(
        &mut self,
        pkg_prefix: &str,
        receiver: &ast::Expr,
        member: &ast::MemberIdent,
    ) -> (ExprKind, TypeId) {
        let receiver = Box::new(self.lower_expr(pkg_prefix, receiver));

        let resolved = member
            .resolved
            .as_ref()
            .map(|r| self.lower_resolved_member_ref(r));

        let member = MemberAccess {
            span: member.span,
            name: self.source.slice(member.span).to_string(),
            resolved,
        };

        (
            ExprKind::MemberAccess { receiver, member },
            self.builtins.any,
        )
    }

    fn lower_resolved_member_ref(&mut self, resolved: &ast::ResolvedMemberRef) -> MemberRef {
        match resolved {
            ast::ResolvedMemberRef::Value { fqn } => MemberRef::Value {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn: fqn.clone(),
            },
            ast::ResolvedMemberRef::Fun { fqn } => MemberRef::Fun {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn: fqn.clone(),
            },
            ast::ResolvedMemberRef::ExtensionValue { fqn } => MemberRef::ExtensionValue {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn: fqn.clone(),
            },
            ast::ResolvedMemberRef::ExtensionFun { fqn } => MemberRef::ExtensionFun {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn: fqn.clone(),
            },
        }
    }

    fn lower_when_arm(&mut self, pkg_prefix: &str, arm: &ast::WhenArm) -> WhenArm {
        WhenArm {
            span: arm.span,
            pat: self.lower_when_pat(&arm.pat),
            guard: arm.guard.as_ref().map(|e| self.lower_expr(pkg_prefix, e)),
            arrow_span: arm.arrow_span,
            body: self.lower_expr(pkg_prefix, &arm.body),
        }
    }

    fn lower_when_pat(&mut self, pat: &ast::WhenPat) -> WhenPat {
        match pat {
            ast::WhenPat::Else { span } => WhenPat::Else { span: *span },
            ast::WhenPat::Or { span, pats } => WhenPat::Or {
                span: *span,
                pats: pats.iter().map(|p| self.lower_when_pat(p)).collect(),
            },
            ast::WhenPat::Wildcard { span } => WhenPat::Wildcard { span: *span },
            ast::WhenPat::Rest { span } => WhenPat::Rest { span: *span },
            ast::WhenPat::Is { is_span, ty } => WhenPat::Is {
                span: Span::new(is_span.start, ty.span().end),
                ty: self.lower_type_ref(ty),
            },
            ast::WhenPat::Bind { ident } => WhenPat::Bind {
                span: ident.span,
                id: self.intern_local_symbol(ident.span, false),
                name: ident.text(self.source).to_string(),
            },
            ast::WhenPat::Tuple { span, elements } => WhenPat::Tuple {
                span: *span,
                elements: elements.iter().map(|e| self.lower_when_pat(e)).collect(),
            },
            ast::WhenPat::Variant { span, name, args } => WhenPat::Variant {
                span: *span,
                name_span: name.span,
                name: name.text(self.source).to_string(),
                args: args.iter().map(|a| self.lower_when_pat(a)).collect(),
            },
            ast::WhenPat::IntLit { span } => WhenPat::IntLit { span: *span },
            ast::WhenPat::StringLit { span } => WhenPat::StringLit { span: *span },
            ast::WhenPat::BoolLit { span } => {
                let value = self.source.slice(*span) == "true";
                WhenPat::BoolLit { span: *span, value }
            }
        }
    }

    /// 尝试把一个调用表达式识别为“effect op call”（例如 `Raise.raise(e)`），并 lower 为 HIR `Perform`。
    ///
    /// 若不匹配该形态，则返回 None，让外层按普通 `Call` 处理。
    fn try_lower_effect_op_call_expr(
        &mut self,
        pkg_prefix: &str,
        callee: &ast::Expr,
        args: &[ast::Expr],
    ) -> Option<(ExprKind, TypeId)> {
        let ast::ExprKind::MemberAccess { member, .. } = &callee.kind else {
            return None;
        };
        let Some(ast::ResolvedMemberRef::Fun { fqn }) = member.resolved.as_ref() else {
            return None;
        };
        if !self.is_effect_op_fqn(fqn) {
            return None;
        }

        let op = EffectOpRef {
            span: member.span,
            fqn: fqn.clone(),
        };
        let args = args
            .iter()
            .map(|arg| self.lower_call_arg(pkg_prefix, arg))
            .collect();
        Some((ExprKind::Perform { op, args }, self.builtins.any))
    }

    fn is_effect_op_fqn(&self, fqn: &str) -> bool {
        let Some(syms) = self.index.by_fqn.get(fqn) else {
            return false;
        };
        syms.fun
            .iter()
            .any(|o| o.sig.kind == ast::FunDeclKind::EffectOp)
    }

    fn lower_handle_expr(
        &mut self,
        pkg_prefix: &str,
        body: &ast::Block,
        arms: &[ast::HandleArm],
        finally: Option<&ast::Block>,
    ) -> HandleExpr {
        let body = self.lower_block(pkg_prefix, body);
        let arms = arms
            .iter()
            .map(|arm| self.lower_handle_arm(pkg_prefix, arm))
            .collect();
        let finally = finally.map(|b| self.lower_block(pkg_prefix, b));
        HandleExpr {
            body,
            arms,
            finally,
        }
    }

    fn lower_handle_arm(&mut self, pkg_prefix: &str, arm: &ast::HandleArm) -> HandleArm {
        let kind = match arm.kind {
            ast::HandleArmKind::NonResuming => HandleArmKind::NonResuming,
            ast::HandleArmKind::ImmediateResume { resume_span } => HandleArmKind::ImmediateResume {
                resume: self.intern_local_symbol(resume_span, false),
            },
            ast::HandleArmKind::EscapeContinuation { k_span } => HandleArmKind::EscapeContinuation {
                continuation: self.intern_local_symbol(k_span, false),
            },
        };
        HandleArm {
            span: arm.span,
            op: self.lower_handle_op(pkg_prefix, &arm.op),
            kind,
            body: self.lower_expr(pkg_prefix, &arm.body),
        }
    }

    fn lower_handle_op(&mut self, _pkg_prefix: &str, op: &ast::HandleOp) -> HandleOp {
        let effect_ty = self.lower_type_path(&op.effect);
        let effect_fqn = self.index.type_ref_to_fqn_in_file(
            self.source,
            self.file,
            &ast::TypeRef::Path(op.effect.clone()),
        );

        let op_name = op.op.text(self.source).to_string();
        let op_fqn = match effect_fqn {
            Some(effect_fqn) => format!("{effect_fqn}.{op_name}"),
            None => format!("{}.{}", self.source.slice(op.effect.span), op_name),
        };

        let binders = op
            .binders
            .iter()
            .map(|b| self.lower_handle_binder(b))
            .collect();

        HandleOp {
            span: op.span,
            effect_ty,
            op: EffectOpRef {
                span: op.op.span,
                fqn: op_fqn,
            },
            binders,
        }
    }

    fn lower_handle_binder(&mut self, b: &ast::HandleBinder) -> HandleBinder {
        let ty =
            b.ty.as_ref()
                .map(|t| self.lower_type_ref(t))
                .unwrap_or(self.builtins.any);
        HandleBinder {
            span: b.span,
            id: self.intern_local_symbol(b.name.span, false),
            name: b.name.text(self.source).to_string(),
            ty,
        }
    }

    fn lower_call_arg(&mut self, pkg_prefix: &str, arg: &ast::Expr) -> CallArg {
        match &arg.kind {
            ast::ExprKind::NamedArg { name, value, .. } => CallArg::Named {
                name: name.text(self.source).to_string(),
                name_span: name.span,
                value: self.lower_expr(pkg_prefix, value),
            },
            _ => CallArg::Positional(self.lower_expr(pkg_prefix, arg)),
        }
    }

    fn lower_ident_expr(&mut self, id: &ast::ValueIdent) -> (ExprKind, TypeId) {
        let text = self.source.slice(id.span);
        if text == "true" {
            return (
                ExprKind::Literal(LiteralKind::Bool(true)),
                self.builtins.bool_,
            );
        }
        if text == "false" {
            return (
                ExprKind::Literal(LiteralKind::Bool(false)),
                self.builtins.bool_,
            );
        }

        let Some(resolved) = id.resolved.as_ref() else {
            // 典型场景：enum variant ctor 的 callee（`Some(1)`）/0-参数 variant 值（`None`）；
            // resolver 会保留为“未 resolve”，让 typecheck 在期望类型语境下决议。
            return (
                ExprKind::UnresolvedIdent {
                    name: text.to_string(),
                },
                self.builtins.any,
            );
        };

        let resolved = match resolved {
            ast::ResolvedValueRef::Local { name, decl_span } => ValueRef::Local {
                id: self.intern_local_symbol(*decl_span, false),
                name: name.clone(),
                decl_span: *decl_span,
            },
            ast::ResolvedValueRef::TopLevel { fqn } => ValueRef::TopLevel {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn: fqn.clone(),
            },
        };

        (ExprKind::VarRef(resolved), self.builtins.any)
    }

    fn is_integer_type(&self, ty: TypeId) -> bool {
        if ty == self.builtins.int || ty == self.builtins.uint {
            return true;
        }

        matches!(
            self.types.kind(ty),
            TypeKind::Value(ValueTypeKind::IntN(_) | ValueTypeKind::UIntN(_))
        )
    }

    /// 对齐 typecheck 阶段的最小规则：整数二元运算要求“相同的整数类型”，但允许一侧是整数字面量。
    ///
    /// 说明：HIR lowering 目前仅用于 dump/fixtures 与早期 codegen，因此这里的规则只覆盖：
    /// - 算术/位运算：`T op T -> T`（一侧为 int literal 时可吸收为另一侧的整数类型）
    /// - 移位：`T << Int -> T` / `T >> Int -> T`
    /// - 比较：`T < T -> Bool` 等
    /// - 相等：`T == T -> Bool` / `Bool == Bool -> Bool`
    fn lower_binary_expr_type(&self, lhs: &Expr, rhs: &Expr, op: ast::BinaryOp) -> TypeId {
        let unify_int_same_type = |lhs: &Expr, rhs: &Expr| -> Option<TypeId> {
            if lhs.ty == rhs.ty && self.is_integer_type(lhs.ty) {
                return Some(lhs.ty);
            }

            let lhs_is_int_lit = matches!(lhs.kind, ExprKind::Literal(LiteralKind::Int));
            let rhs_is_int_lit = matches!(rhs.kind, ExprKind::Literal(LiteralKind::Int));

            if lhs_is_int_lit && self.is_integer_type(rhs.ty) {
                return Some(rhs.ty);
            }
            if rhs_is_int_lit && self.is_integer_type(lhs.ty) {
                return Some(lhs.ty);
            }

            None
        };

        match op {
            // arithmetic + bitwise: T op T -> T
            ast::BinaryOp::Add
            | ast::BinaryOp::Sub
            | ast::BinaryOp::Mul
            | ast::BinaryOp::Div
            | ast::BinaryOp::Rem
            | ast::BinaryOp::BitAnd
            | ast::BinaryOp::BitXor
            | ast::BinaryOp::BitOr => unify_int_same_type(lhs, rhs).unwrap_or(self.builtins.any),

            // shifts: T << Int -> T
            ast::BinaryOp::Shl | ast::BinaryOp::Shr => {
                if self.is_integer_type(lhs.ty) && rhs.ty == self.builtins.int {
                    lhs.ty
                } else {
                    self.builtins.any
                }
            }

            // comparisons: T < T -> Bool
            ast::BinaryOp::Lt | ast::BinaryOp::Le | ast::BinaryOp::Gt | ast::BinaryOp::Ge => {
                if unify_int_same_type(lhs, rhs).is_some() {
                    self.builtins.bool_
                } else {
                    self.builtins.any
                }
            }

            // equality: (T == T) -> Bool; (Bool == Bool) -> Bool
            ast::BinaryOp::Eq | ast::BinaryOp::Ne => {
                if lhs.ty == self.builtins.bool_ && rhs.ty == self.builtins.bool_ {
                    return self.builtins.bool_;
                }
                if unify_int_same_type(lhs, rhs).is_some() {
                    return self.builtins.bool_;
                }
                self.builtins.any
            }

            // boolean logic: Bool op Bool -> Bool
            ast::BinaryOp::LogAnd | ast::BinaryOp::LogOr => {
                if lhs.ty == self.builtins.bool_ && rhs.ty == self.builtins.bool_ {
                    self.builtins.bool_
                } else {
                    self.builtins.any
                }
            }

            // elvis not lowered in current HIR dump mode
            ast::BinaryOp::Elvis => self.builtins.any,
        }
    }

    fn lower_type_ref(&mut self, t: &ast::TypeRef) -> TypeId {
        match t {
            ast::TypeRef::Path(p) => self.lower_type_path(p),
            ast::TypeRef::Tuple(tt) => {
                if tt.elements.is_empty() {
                    return self.builtins.unit;
                }
                let elements = tt.elements.iter().map(|e| self.lower_type_ref(e)).collect();
                self.types.ty_tuple(elements)
            }
            ast::TypeRef::Nullable { inner, .. } => {
                let inner = self.lower_type_ref(inner);
                self.types.ty_option(inner)
            }
            ast::TypeRef::Function(fun) => {
                let receiver = fun.receiver.as_ref().map(|r| self.lower_type_ref(r));
                let params = fun.params.iter().map(|p| self.lower_type_ref(p)).collect();
                let return_ty = self.lower_type_ref(&fun.return_ty);
                let effects = self.lower_effect_row_expr(fun.effects.as_ref());
                self.types.ty_function(receiver, params, return_ty, effects)
            }
            ast::TypeRef::Star { .. } | ast::TypeRef::EffectRowArg { .. } => self.builtins.any,
        }
    }

    fn lower_type_path(&mut self, p: &ast::TypePath) -> TypeId {
        // 单段名且无实参：优先解析为当前作用域的 type parameter。
        if p.segments.len() == 1 && p.args.is_empty() {
            let name = p.segments[0].text(self.source);
            if let Some(id) = self.lookup_type_param(name) {
                return id;
            }
        }

        let fqn = self.index.type_ref_to_fqn_in_file(
            self.source,
            self.file,
            &ast::TypeRef::Path(p.clone()),
        );

        let Some(fqn) = fqn else {
            return self.builtins.any;
        };

        // 分离：普通 type args vs use-site effect row arg（`eff ...`）。
        let mut eff_arg: Option<&ast::EffectRowExpr> = None;
        let mut type_args: Vec<&ast::TypeRef> = Vec::new();
        for a in &p.args {
            match a {
                ast::TypeRef::EffectRowArg { row, .. } => {
                    eff_arg.get_or_insert(row);
                }
                other => type_args.push(other),
            }
        }

        // 少数 builtin/special-case：不走 nominal。
        match fqn.as_str() {
            "scoop.core.Any" => return self.builtins.any,
            "scoop.core.String" => return self.builtins.string,
            "scoop.core.Unit" => return self.builtins.unit,
            "scoop.core.Nothing" => return self.builtins.nothing,
            "scoop.core.Bool" => return self.builtins.bool_,
            "scoop.core.Int" => return self.builtins.int,
            "scoop.core.UInt" => return self.builtins.uint,
            "scoop.core.Option" => {
                let inner = type_args
                    .first()
                    .map(|t| self.lower_type_ref(t))
                    .unwrap_or(self.builtins.any);
                return self.types.ty_option(inner);
            }
            _ => {}
        }

        // `Int32`/`UInt64` 这类固定位宽整数：若出现在 sysroot/type env 中，直接 lowering 为内建整数族。
        if let Some(bits) = fqn
            .strip_prefix("scoop.core.Int")
            .and_then(|s| s.parse::<u16>().ok())
        {
            return self.types.ty_int_n(bits);
        }
        if let Some(bits) = fqn
            .strip_prefix("scoop.core.UInt")
            .and_then(|s| s.parse::<u16>().ok())
        {
            return self.types.ty_uint_n(bits);
        }

        let args = type_args
            .iter()
            .map(|a| self.lower_type_ref(a))
            .collect::<Vec<_>>();
        let eff = eff_arg.map(|e| self.lower_effect_row_expr(Some(e)));
        self.intern_nominal(fqn, args, eff)
    }

    fn lower_effect_row_expr(&mut self, expr: Option<&ast::EffectRowExpr>) -> EffectRow {
        let Some(expr) = expr else {
            return EffectRow::pure();
        };
        if expr.terms.is_empty() {
            return EffectRow::pure();
        }

        let mut terms: Vec<TypeId> = Vec::with_capacity(expr.terms.len());
        for term in &expr.terms {
            terms.push(self.lower_type_path(term));
        }
        EffectRow::new(terms)
    }

    fn intern_nominal(&mut self, fqn: String, args: Vec<TypeId>, eff: Option<EffectRow>) -> TypeId {
        let nominal = NominalType { fqn, args, eff };

        // 尝试用 `type_kinds` 判断 struct/enum（value type）vs class/interface/effect（ref type）。
        let kind = self.type_kinds.get(&nominal.fqn).copied();
        match kind {
            Some(ast::TypeKind::Struct | ast::TypeKind::Enum) => self
                .types
                .intern(TypeKind::Value(ValueTypeKind::Nominal(nominal))),
            _ => self
                .types
                .intern(TypeKind::Ref(RefTypeKind::Nominal(nominal))),
        }
    }

    fn push_type_params(&mut self, params: &[ast::TypeParam]) {
        if params.is_empty() {
            self.type_param_scopes.push(HashMap::new());
            return;
        }

        let decl_file = self.source.path().to_path_buf();
        let mut frame = HashMap::new();
        for p in params {
            let name = p.name.text(self.source).to_string();
            let id = self.types.ty_param(TypeParamType {
                name: name.clone(),
                decl_file: decl_file.clone(),
                decl_span: p.name.span,
            });
            frame.insert(name, id);
        }
        self.type_param_scopes.push(frame);
    }

    /// 直接注入一组“使用点 type param 绑定”（name → TypeId）。
    ///
    /// 用途：
    /// - 单态化实例生成：把 `T` 等抽象类型替换为调用点推断出的具体类型。
    fn push_type_param_bindings(&mut self, bindings: impl IntoIterator<Item = (String, TypeId)>) {
        let mut frame = HashMap::new();
        for (name, id) in bindings {
            frame.insert(name, id);
        }
        self.type_param_scopes.push(frame);
    }

    fn pop_type_params(&mut self) {
        let _ = self.type_param_scopes.pop();
    }

    fn lookup_type_param(&self, name: &str) -> Option<TypeId> {
        self.type_param_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }
}

/// 计算 closure（lambda）的 capture set（自由变量集合）。
///
/// 规则（最小实现，供 T0711）：
/// - 只统计 `VarRef(Local)`；
/// - 以该 lambda 的 params 与其 body 内引入的局部声明（`val/var`、`when` binder、`handle` binder）为“本地声明”；
/// - 只把“在 body 中被引用但不属于本地声明”的 local 视为 capture；
/// - 遇到嵌套 closure 时不深入（由内层 closure 自己计算 captures）。
fn compute_closure_captures(
    params: &[Param],
    body: &Expr,
    local_mutability: &HashMap<SymbolId, bool>,
) -> Vec<Capture> {
    let mut declared: HashSet<SymbolId> = params.iter().map(|p| p.id).collect();
    collect_declared_locals_in_expr(body, &mut declared);

    let mut used: HashMap<SymbolId, Capture> = HashMap::new();
    collect_used_locals_in_expr(body, &mut used);

    let mut captures: Vec<Capture> = used
        .into_values()
        .filter(|c| !declared.contains(&c.id))
        .collect();

    for c in &mut captures {
        c.mutable = local_mutability.get(&c.id).copied().unwrap_or(false);
    }

    // 稳定排序：按声明位置排序（同位置用 SymbolId 兜底）。
    captures.sort_by(|a, b| {
        a.decl_span
            .start
            .cmp(&b.decl_span.start)
            .then_with(|| a.decl_span.end.cmp(&b.decl_span.end))
            .then_with(|| a.id.as_u32().cmp(&b.id.as_u32()))
    });

    captures
}

fn collect_declared_locals_in_expr(expr: &Expr, declared: &mut HashSet<SymbolId>) {
    match &expr.kind {
        ExprKind::Missing
        | ExprKind::Literal(_)
        | ExprKind::VarRef(_)
        | ExprKind::UnresolvedIdent { .. }
        | ExprKind::Todo(_) => {}
        ExprKind::StructLit { fields, .. } => {
            for f in fields {
                collect_declared_locals_in_expr(&f.value, declared);
            }
        }
        ExprKind::TupleLit { elements } => {
            for e in elements {
                collect_declared_locals_in_expr(e, declared);
            }
        }
        ExprKind::InterpolatedString { parts, .. } => {
            for p in parts {
                if let super::InterpolatedStringPart::Expr { expr } = p {
                    collect_declared_locals_in_expr(expr, declared);
                }
            }
        }
        ExprKind::Unary { expr, .. } => collect_declared_locals_in_expr(expr.as_ref(), declared),
        ExprKind::Binary { lhs, rhs, .. } => {
            collect_declared_locals_in_expr(lhs.as_ref(), declared);
            collect_declared_locals_in_expr(rhs.as_ref(), declared);
        }
        ExprKind::Block(block) => collect_declared_locals_in_block(block, declared),
        ExprKind::Closure(_) => {
            // 嵌套 closure：由其自身计算 capture set。
        }
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_declared_locals_in_expr(cond, declared);
            collect_declared_locals_in_expr(then_branch, declared);
            if let Some(e) = else_branch.as_deref() {
                collect_declared_locals_in_expr(e, declared);
            }
        }
        ExprKind::When { subject, arms } => {
            collect_declared_locals_in_expr(subject, declared);
            for arm in arms {
                collect_declared_locals_in_when_pat(&arm.pat, declared);
                if let Some(g) = &arm.guard {
                    collect_declared_locals_in_expr(g, declared);
                }
                collect_declared_locals_in_expr(&arm.body, declared);
            }
        }
        ExprKind::MemberAccess { receiver, .. } => {
            collect_declared_locals_in_expr(receiver, declared)
        }
        ExprKind::Call { callee, args } => {
            collect_declared_locals_in_expr(callee, declared);
            for arg in args {
                match arg {
                    CallArg::Positional(e) => collect_declared_locals_in_expr(e, declared),
                    CallArg::Named { value, .. } => {
                        collect_declared_locals_in_expr(value, declared)
                    }
                }
            }
        }
        ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    CallArg::Positional(e) => collect_declared_locals_in_expr(e, declared),
                    CallArg::Named { value, .. } => {
                        collect_declared_locals_in_expr(value, declared)
                    }
                }
            }
        }
        ExprKind::Handle(handle) => {
            collect_declared_locals_in_block(&handle.body, declared);
            for arm in &handle.arms {
                for b in &arm.op.binders {
                    declared.insert(b.id);
                }
                collect_declared_locals_in_expr(&arm.body, declared);
            }
            if let Some(finally) = &handle.finally {
                collect_declared_locals_in_block(finally, declared);
            }
        }
    }
}

fn collect_declared_locals_in_block(block: &Block, declared: &mut HashSet<SymbolId>) {
    for stmt in &block.stmts {
        match &stmt.kind {
            StmtKind::Val(v) => {
                if let Some(id) = v.id {
                    declared.insert(id);
                }
                if let Some(init) = &v.init {
                    collect_declared_locals_in_expr(init, declared);
                }
            }
            StmtKind::Expr(e) => collect_declared_locals_in_expr(e, declared),
            StmtKind::Assign { lhs, rhs, .. } => {
                collect_declared_locals_in_expr(lhs, declared);
                collect_declared_locals_in_expr(rhs, declared);
            }
            StmtKind::While { cond, body } => {
                collect_declared_locals_in_expr(cond, declared);
                collect_declared_locals_in_block(body, declared);
            }
            StmtKind::Return { value } => {
                if let Some(v) = value {
                    collect_declared_locals_in_expr(v, declared);
                }
            }
            StmtKind::Empty
            | StmtKind::Break { .. }
            | StmtKind::Continue { .. }
            | StmtKind::Todo(_) => {}
        }
    }
}

fn collect_declared_locals_in_when_pat(pat: &WhenPat, declared: &mut HashSet<SymbolId>) {
    match pat {
        WhenPat::Or { pats, .. } => {
            for p in pats {
                collect_declared_locals_in_when_pat(p, declared);
            }
        }
        WhenPat::Bind { id, .. } => {
            declared.insert(*id);
        }
        WhenPat::Tuple { elements, .. } => {
            for e in elements {
                collect_declared_locals_in_when_pat(e, declared);
            }
        }
        WhenPat::Variant { args, .. } => {
            for a in args {
                collect_declared_locals_in_when_pat(a, declared);
            }
        }
        WhenPat::Else { .. }
        | WhenPat::Wildcard { .. }
        | WhenPat::Rest { .. }
        | WhenPat::Is { .. }
        | WhenPat::IntLit { .. }
        | WhenPat::StringLit { .. }
        | WhenPat::BoolLit { .. } => {}
    }
}

fn collect_used_locals_in_expr(expr: &Expr, used: &mut HashMap<SymbolId, Capture>) {
    match &expr.kind {
        ExprKind::Missing
        | ExprKind::Literal(_)
        | ExprKind::UnresolvedIdent { .. }
        | ExprKind::Todo(_) => {}
        ExprKind::VarRef(v) => {
            let ValueRef::Local {
                id,
                name,
                decl_span,
            } = v
            else {
                return;
            };
            used.entry(*id).or_insert_with(|| Capture {
                id: *id,
                name: name.clone(),
                decl_span: *decl_span,
                mutable: false,
            });
        }
        ExprKind::StructLit { fields, .. } => {
            for f in fields {
                collect_used_locals_in_expr(&f.value, used);
            }
        }
        ExprKind::TupleLit { elements } => {
            for e in elements {
                collect_used_locals_in_expr(e, used);
            }
        }
        ExprKind::InterpolatedString { parts, .. } => {
            for p in parts {
                if let super::InterpolatedStringPart::Expr { expr } = p {
                    collect_used_locals_in_expr(expr, used);
                }
            }
        }
        ExprKind::Unary { expr, .. } => collect_used_locals_in_expr(expr.as_ref(), used),
        ExprKind::Binary { lhs, rhs, .. } => {
            collect_used_locals_in_expr(lhs.as_ref(), used);
            collect_used_locals_in_expr(rhs.as_ref(), used);
        }
        ExprKind::Block(block) => {
            for stmt in &block.stmts {
                match &stmt.kind {
                    StmtKind::Expr(e) => collect_used_locals_in_expr(e, used),
                    StmtKind::Val(v) => {
                        if let Some(init) = &v.init {
                            collect_used_locals_in_expr(init, used);
                        }
                    }
                    StmtKind::Assign { lhs, rhs, .. } => {
                        collect_used_locals_in_expr(lhs, used);
                        collect_used_locals_in_expr(rhs, used);
                    }
                    StmtKind::While { cond, body } => {
                        collect_used_locals_in_expr(cond, used);
                        // while body 是一个 block；其内部的局部声明不影响“使用”收集。
                        collect_used_locals_in_expr(
                            &Expr {
                                span: body.span,
                                ty: body.ty,
                                kind: ExprKind::Block(body.clone()),
                            },
                            used,
                        );
                    }
                    StmtKind::Return { value } => {
                        if let Some(v) = value {
                            collect_used_locals_in_expr(v, used);
                        }
                    }
                    StmtKind::Empty
                    | StmtKind::Break { .. }
                    | StmtKind::Continue { .. }
                    | StmtKind::Todo(_) => {}
                }
            }
        }
        ExprKind::Closure(_) => {
            // 嵌套 closure：由其自身计算 capture set。
        }
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_used_locals_in_expr(cond, used);
            collect_used_locals_in_expr(then_branch, used);
            if let Some(e) = else_branch.as_deref() {
                collect_used_locals_in_expr(e, used);
            }
        }
        ExprKind::When { subject, arms } => {
            collect_used_locals_in_expr(subject, used);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    collect_used_locals_in_expr(g, used);
                }
                collect_used_locals_in_expr(&arm.body, used);
            }
        }
        ExprKind::MemberAccess { receiver, .. } => collect_used_locals_in_expr(receiver, used),
        ExprKind::Call { callee, args } => {
            collect_used_locals_in_expr(callee, used);
            for arg in args {
                match arg {
                    CallArg::Positional(e) => collect_used_locals_in_expr(e, used),
                    CallArg::Named { value, .. } => collect_used_locals_in_expr(value, used),
                }
            }
        }
        ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    CallArg::Positional(e) => collect_used_locals_in_expr(e, used),
                    CallArg::Named { value, .. } => collect_used_locals_in_expr(value, used),
                }
            }
        }
        ExprKind::Handle(handle) => {
            // handle body / arm body 里的 var refs 都算“使用”；binder 是否 capture 由 declared 集合处理。
            collect_used_locals_in_expr(
                &Expr {
                    span: handle.body.span,
                    ty: handle.body.ty,
                    kind: ExprKind::Block(handle.body.clone()),
                },
                used,
            );
            for arm in &handle.arms {
                collect_used_locals_in_expr(&arm.body, used);
            }
            if let Some(finally) = &handle.finally {
                collect_used_locals_in_expr(
                    &Expr {
                        span: finally.span,
                        ty: finally.ty,
                        kind: ExprKind::Block(finally.clone()),
                    },
                    used,
                );
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValScope {
    TopLevel,
    Local,
}

/// 为 `scoop dump-hir` 生成 HIR（最小实现）。
///
/// 流程：
/// 1) parse 源文件为 AST；
/// 2) 构建 sysroot + 当前文件的 `Index`；
/// 3) 运行 resolver（headers + bodies）把绑定结果写回 AST；
/// 4) 在一个新的 `TypeStore` 中 intern builtin types，收集 struct 布局信息，并把 AST 降为 HIR
///    （未覆盖节点用 `Any` 占位）。
pub fn lower_for_dump(session: &Session, source: &SourceFile) -> Result<LoweredHir, HirLowerError> {
    let mut ast = parse_file(source)?;

    let index = {
        // 注意：`check_file_bodies` 需要 `&mut ast`，因此这里把构建 index 的临时借用放在独立作用域中，
        // 避免把 `&ast` 存到更长生命周期的容器里导致借用冲突。
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &session.sysroot().files {
            pairs.push((&f.source, &f.ast));
        }
        pairs.push((source, &ast));
        Index::build(&pairs)?
    };

    let headers = crate::resolve::check_file_headers(source, &ast, &index)?;
    crate::resolve::check_file_bodies(source, &mut ast, &index, &headers)?;

    let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        pairs.push((&f.source, &f.ast));
    }
    pairs.push((source, &ast));
    let type_kinds = collect_type_decl_kinds(&pairs);

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();

    // 先降 HIR（保持 fixtures 中 `TypeId` 分配顺序稳定），再补充 struct 布局索引供后端使用。
    let file = {
        let mut ctx = HirLowering::new(source, &ast, &index, &type_kinds, &mut types, builtins);
        ctx.lower_file()
    };

    // T0828：收集 object/companion object 的 once 初始化信息（不影响 HIR dump 输出稳定性）。
    let object_inits = collect_object_inits(source, &ast, &index, &type_kinds, &mut types, builtins);

    // T1006：收集 `@Extern` 外部函数的符号名与 ABI（side table；不影响 dump-hir 输出）。
    let extern_funs = collect_extern_funs(source, &ast);

    // T0811：早期 LLVM codegen 需要知道 struct 的字段顺序与字段类型，用于生成字段 GEP 索引。
    let struct_layouts = collect_struct_layouts(&pairs, &index);
    // T0813：早期 LLVM codegen 需要知道 enum 的 variant tag 与 payload 字段类型，用于生成判别与解构。
    let enum_layouts = collect_enum_layouts(&pairs, &index);
    Ok(LoweredHir {
        file,
        types,
        struct_layouts,
        enum_layouts,
        extern_funs,
        object_inits,
    })
}

/// 将给定的 `ast::FunDecl` 在“已绑定 type params”的语境下降低为 HIR（用于单态化，T0712）。
///
/// 说明：
/// - 该函数假设调用方已在 AST 上运行 resolver（headers + bodies），以便 `ValueIdent.resolved` 等信息可用；
/// - `type_bindings` 用于把函数声明处的 `T` 等 type params 映射为具体 `TypeId`（调用点推断结果）。
pub(crate) fn lower_fun_with_type_bindings(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    type_kinds: &HashMap<String, ast::TypeKind>,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
    fun: &ast::FunDecl,
    type_bindings: impl IntoIterator<Item = (String, TypeId)>,
) -> FunDecl {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut ctx = HirLowering::new(source, file, index, type_kinds, types, builtins);
    ctx.push_type_param_bindings(type_bindings);
    let out = ctx.lower_fun_decl_with_bound_type_params(&pkg_prefix, fun);
    ctx.pop_type_params();
    out
}

fn package_prefix(source: &SourceFile, package: Option<&ast::PackageDecl>) -> String {
    let Some(p) = package else {
        return String::new();
    };

    let mut out = String::new();
    for (idx, seg) in p.path.iter().enumerate() {
        if idx != 0 {
            out.push('.');
        }
        out.push_str(seg.text(source));
    }
    out
}

fn collect_type_decl_kinds(pairs: &[(&SourceFile, &ast::File)]) -> HashMap<String, ast::TypeKind> {
    let mut out: HashMap<String, ast::TypeKind> = HashMap::new();
    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            let ast::Item::Type(ty) = item else {
                continue;
            };
            let name = ty.name.text(source).to_string();
            let fqn = if pkg_prefix.is_empty() {
                name
            } else {
                format!("{pkg_prefix}.{name}")
            };
            out.insert(fqn, ty.kind);
        }
    }
    out
}

fn collect_object_inits(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    type_kinds: &HashMap<String, ast::TypeKind>,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> ObjectInitIndex {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut ctx = HirLowering::new(source, file, index, type_kinds, types, builtins);

    let mut out: ObjectInitIndex = HashMap::new();
    for item in &file.items {
        match item {
            ast::Item::Object(obj) => {
                collect_object_decl_inits(&mut ctx, &pkg_prefix, &pkg_prefix, obj, &mut out);
            }
            ast::Item::Type(ty) => {
                collect_objects_in_type_decl(&mut ctx, &pkg_prefix, &pkg_prefix, ty, &mut out);
            }
            ast::Item::Fun(_)
            | ast::Item::Val(_)
            | ast::Item::ExtensionProperty(_)
            | ast::Item::TypeAlias(_) => {}
        }
    }

    out
}

fn collect_objects_in_type_decl(
    ctx: &mut HirLowering<'_>,
    pkg_prefix: &str,
    owner_prefix: &str,
    decl: &ast::TypeDecl,
    out: &mut ObjectInitIndex,
) {
    let name = decl.name.text(ctx.source).to_string();
    let type_fqn = join_prefix(owner_prefix, &name);
    let Some(body) = &decl.body else {
        return;
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Object(obj) => {
                collect_object_decl_inits(ctx, pkg_prefix, &type_fqn, obj, out);
            }
            ast::TypeMember::Type(nested) => {
                collect_objects_in_type_decl(ctx, pkg_prefix, &type_fqn, nested, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

fn collect_object_decl_inits(
    ctx: &mut HirLowering<'_>,
    pkg_prefix: &str,
    owner_prefix: &str,
    obj: &ast::ObjectDecl,
    out: &mut ObjectInitIndex,
) {
    let Some(name) = object_decl_name(ctx.source, obj) else {
        return;
    };

    let fqn = join_prefix(owner_prefix, &name);
    let mut init = ObjectInit {
        fqn: fqn.clone(),
        properties: HashMap::new(),
        steps: Vec::new(),
    };

    if let Some(body) = &obj.body {
        for member in &body.members {
            match member {
                ast::TypeMember::Property(p) => {
                    let name = p.name.text(ctx.source).to_string();
                    let mutable = matches!(p.kind, ast::ValKind::Var);
                    let ty =
                        p.ty.as_ref()
                            .map(|t| ctx.lower_type_ref(t))
                            .unwrap_or(ctx.builtins.any);
                    let has_init = p.init.is_some();
                    init.properties.insert(
                        name.clone(),
                        ObjectProperty {
                            name: name.clone(),
                            mutable,
                            ty,
                            has_init,
                        },
                    );

                    if let Some(expr) = p.init.as_ref() {
                        let lowered = ctx.lower_expr(pkg_prefix, expr);
                        init.steps.push(ObjectInitStep::PropertyInit { name, init: lowered });
                    }
                }
                ast::TypeMember::InitBlock(b) => {
                    let block = ctx.lower_block(pkg_prefix, &b.body);
                    init.steps.push(ObjectInitStep::InitBlock { block });
                }
                ast::TypeMember::Object(nested) => {
                    collect_object_decl_inits(ctx, pkg_prefix, &fqn, nested, out);
                }
                ast::TypeMember::Type(_)
                | ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::SecondaryCtor(_)
                | ast::TypeMember::Fun(_) => {}
            }
        }
    }

    out.entry(fqn).or_insert(init);
}

fn object_decl_name(source: &SourceFile, obj: &ast::ObjectDecl) -> Option<String> {
    match obj.name.as_ref() {
        Some(name) => Some(name.text(source).to_string()),
        None => match obj.kind {
            ast::ObjectKind::Companion => Some("Companion".to_string()),
            ast::ObjectKind::Object => None,
        },
    }
}

fn join_prefix(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}

fn collect_extern_funs(source: &SourceFile, file: &ast::File) -> super::ExternFunIndex {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut out: super::ExternFunIndex = HashMap::new();

    for item in &file.items {
        let ast::Item::Fun(fun) = item else {
            continue;
        };

        let Some(extern_fun) = extern_fun_of_decl(source, fun) else {
            continue;
        };

        let name = fun.name.text(source).to_string();
        let fqn = if pkg_prefix.is_empty() {
            name
        } else {
            format!("{pkg_prefix}.{name}")
        };

        out.insert(fqn, extern_fun);
    }

    out
}

fn extern_fun_of_decl(source: &SourceFile, fun: &ast::FunDecl) -> Option<super::ExternFun> {
    // 说明：
    // - `@Extern` 在语义上由 typecheck 校验（参数个数/类型等）；
    // - HIR lowering 只做“提取已校验信息”的 best-effort，避免把错误传播面扩到 HIR/LLVM 层。
    let name = fun.name.text(source);

    for ann in &fun.annotations {
        if !is_builtin_extern_annotation(source, ann) {
            continue;
        }

        let symbol = match ann.args.as_slice() {
            // `@Extern`：缺省用函数名作为链接符号名。
            [] => name.to_string(),
            // `@Extern("foo")`：单一位置参数（字符串字面量）。
            [arg] if arg.name.is_none() => {
                let text = source.slice(arg.value.span);
                parse_string_literal_utf8(text).unwrap_or_else(|_| name.to_string())
            }
            // 其它形态（如命名参数/多参数）在 typecheck 阶段应报错；这里按缺省兜底。
            _ => name.to_string(),
        };

        return Some(super::ExternFun {
            abi: super::ExternAbi::C,
            symbol,
        });
    }

    None
}

fn is_builtin_extern_annotation(source: &SourceFile, ann: &ast::AnnotationUse) -> bool {
    let segs = ann.path.iter().map(|id| id.text(source)).collect::<Vec<_>>();
    matches!(segs.as_slice(), ["Extern"] | ["scoop", "core", "Extern"])
}

/// 收集当前编译单元（sysroot + 当前文件）里出现的 struct 字段布局信息。
///
/// 说明（早期阶段约束）：
/// - 仅收集**顶层 struct**；
/// - 仅使用 struct 的 primary ctor params 作为字段（与 resolver 对齐：`p.x` 来自 ctor param）；
/// - 暂不支持泛型 struct / `eff` 参数化 struct：这类布局需要单态化后再确定（留到后续任务）。
fn collect_struct_layouts(pairs: &[(&SourceFile, &ast::File)], index: &Index) -> StructLayoutIndex {
    let mut out: StructLayoutIndex = HashMap::new();

    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());

        for item in &file.items {
            let ast::Item::Type(ty) = item else {
                continue;
            };
            if !matches!(ty.kind, ast::TypeKind::Struct) {
                continue;
            }

            // 泛型/eff 参数化 struct 的布局需要在 monomorphization 后才能稳定确定：
            // - field 的 type args 可能包含未绑定的 type params；
            // - ABI/layout 可能依赖实例化参数。
            if !ty.type_params.is_empty() || ty.eff_param.is_some() {
                continue;
            }

            let name = ty.name.text(source).to_string();
            let fqn = if pkg_prefix.is_empty() {
                name.clone()
            } else {
                format!("{pkg_prefix}.{name}")
            };

            // 避免重复写入（例如 sysroot 与用户文件存在同名 type 时，resolver 会先报错）。
            if out.contains_key(&fqn) {
                continue;
            }

            let mut fields: Vec<StructFieldLayout> = Vec::new();
            if let Some(primary_ctor) = &ty.primary_ctor {
                for p in &primary_ctor.params {
                    let field_name = p.name.text(source).to_string();
                    let field_fqn = format!("{fqn}.{field_name}");
                    let ty_fqn =
                        p.ty.as_ref()
                            .and_then(|t| index.type_ref_to_fqn_in_file(source, file, t));

                    fields.push(StructFieldLayout {
                        span: p.name.span,
                        name: field_name,
                        fqn: field_fqn,
                        ty_fqn,
                    });
                }
            }

            out.insert(fqn.clone(), StructLayout { fqn, fields });
        }
    }

    out
}

/// 收集当前编译单元（sysroot + 当前文件）里出现的 enum variant 布局信息。
///
/// 说明（早期阶段约束）：
/// - 仅收集**顶层 enum**；
/// - 暂不支持泛型 enum / `eff` 参数化 enum（这类布局需要单态化后再确定，留到后续任务）；
/// - variant tag 按声明顺序分配，从 0 开始（与 typecheck/type env 的最小规则对齐）。
fn collect_enum_layouts(pairs: &[(&SourceFile, &ast::File)], index: &Index) -> EnumLayoutIndex {
    let mut out: EnumLayoutIndex = HashMap::new();

    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());

        for item in &file.items {
            let ast::Item::Type(ty) = item else {
                continue;
            };
            if !matches!(ty.kind, ast::TypeKind::Enum) {
                continue;
            }

            // 泛型/eff 参数化 enum 的布局需要在 monomorphization 后才能稳定确定：
            // - payload 字段类型可能包含未绑定的 type params；
            // - 后端布局/boxing 策略可能依赖实例化参数。
            if !ty.type_params.is_empty() || ty.eff_param.is_some() {
                continue;
            }

            let name = ty.name.text(source).to_string();
            let fqn = if pkg_prefix.is_empty() {
                name.clone()
            } else {
                format!("{pkg_prefix}.{name}")
            };

            if out.contains_key(&fqn) {
                continue;
            }

            let mut variants: Vec<EnumVariantLayout> = Vec::new();
            let Some(body) = &ty.body else {
                out.insert(fqn.clone(), EnumLayout { fqn, variants });
                continue;
            };

            let mut next_tag: u32 = 0;
            for member in &body.members {
                let ast::TypeMember::EnumVariant(v) = member else {
                    continue;
                };

                let variant_name = v.name.text(source).to_string();
                let tag = next_tag;
                next_tag = next_tag.saturating_add(1);

                let mut fields: Vec<EnumVariantFieldLayout> = Vec::new();
                for p in &v.params {
                    let field_name = p.name.text(source).to_string();
                    let ty_fqn =
                        p.ty.as_ref()
                            .and_then(|t| index.type_ref_to_fqn_in_file(source, file, t));
                    fields.push(EnumVariantFieldLayout {
                        span: p.name.span,
                        name: field_name,
                        ty_fqn,
                    });
                }

                variants.push(EnumVariantLayout {
                    span: v.span,
                    name: variant_name,
                    tag,
                    fields,
                });
            }

            out.insert(fqn.clone(), EnumLayout { fqn, variants });
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn lower_minimal_file_smoke() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual("<mem>", "package a\nfun main() { val x: Int = 1; x }");

        let lowered = lower_for_dump(&sess, &src).unwrap();
        assert!(!lowered.file.items.is_empty());
    }

    #[test]
    fn hir_fixture_minimal_golden() {
        let sess = Session::new().unwrap();

        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/minimal.scoop");
        let file = SourceFile::load(&fixture_path).unwrap();

        let lowered = lower_for_dump(&sess, &file).unwrap();
        let actual = format!("{:#?}\n", lowered.file);

        let golden_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/hir/minimal.hir");
        let expected = std::fs::read_to_string(&golden_path).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn hir_fixture_handle_perform_golden() {
        let sess = Session::new().unwrap();

        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/handle_perform.scoop");
        let file = SourceFile::load(&fixture_path).unwrap();

        let lowered = lower_for_dump(&sess, &file).unwrap();
        let actual = format!("{:#?}\n", lowered.file);

        let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/handle_perform.hir");
        let expected = std::fs::read_to_string(&golden_path).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn hir_fixture_control_flow_golden() {
        let sess = Session::new().unwrap();

        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/control_flow.scoop");
        let file = SourceFile::load(&fixture_path).unwrap();

        let lowered = lower_for_dump(&sess, &file).unwrap();
        let actual = format!("{:#?}\n", lowered.file);

        let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/control_flow.hir");
        let expected = std::fs::read_to_string(&golden_path).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn hir_fixture_member_access_golden() {
        let sess = Session::new().unwrap();

        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/member_access.scoop");
        let file = SourceFile::load(&fixture_path).unwrap();

        let lowered = lower_for_dump(&sess, &file).unwrap();
        let actual = format!("{:#?}\n", lowered.file);

        let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/member_access.hir");
        let expected = std::fs::read_to_string(&golden_path).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn hir_fixture_closure_non_capture_golden() {
        let sess = Session::new().unwrap();

        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/closure_non_capture.scoop");
        let file = SourceFile::load(&fixture_path).unwrap();

        let lowered = lower_for_dump(&sess, &file).unwrap();
        let actual = format!("{:#?}\n", lowered.file);

        let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/closure_non_capture.hir");
        let expected = std::fs::read_to_string(&golden_path).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn hir_fixture_closure_capture_val_golden() {
        let sess = Session::new().unwrap();

        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/closure_capture_val.scoop");
        let file = SourceFile::load(&fixture_path).unwrap();

        let lowered = lower_for_dump(&sess, &file).unwrap();
        let actual = format!("{:#?}\n", lowered.file);

        let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/closure_capture_val.hir");
        let expected = std::fs::read_to_string(&golden_path).unwrap();

        assert_eq!(actual, expected);
    }
}
