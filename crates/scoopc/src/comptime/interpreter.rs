//! const/comptime 解释器（v0）。
//!
//! 目标（TODO T1202c）：
//! - 支持 `const fun` 调用（仅 Pure；由 typecheck headers 做最小门禁）；
//! - 支持函数体内的局部 `val`、`return` 语句、以及 block 的“最后表达式返回”；
//! - 支持 `const val` initializer 的常量折叠（用于 `tests/fixtures/comptime` 回归）。
//!
//! 非目标（后续任务逐步补齐）：
//! - 闭包/lambda、effects、循环/控制流（`if/when/while`）、`perform/handle`；
//! - 泛型实例化与重载决议（当前仅按“函数名 + 参数个数”做最小选择）。

use std::collections::HashMap;
use std::ops::ControlFlow;

use crate::ast;
use crate::source::SourceFile;
use crate::span::Span;

use super::eval::{ConstEvalHost, eval_const_expr_with_host};
use super::{ConstEvalCtx, ConstEvalError, ConstValue};

/// const 解释器配置项（v0）。
#[derive(Debug, Clone, Copy)]
pub struct ConstEvalOptions {
    /// 最大递归深度（避免无限递归导致栈溢出）。
    pub recursion_limit: usize,
}

impl Default for ConstEvalOptions {
    fn default() -> Self {
        Self { recursion_limit: 64 }
    }
}

/// 一个 `const val` 的求值结果（用于 dump/fixtures）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstBinding {
    pub name: String,
    pub value: ConstValue,
}

/// 计算一个文件中所有 `const val` 的 initializer。
///
/// 说明：
/// - 该函数不做 parse；调用方需先通过 parser 拿到 AST；
/// - 当前仅支持同一文件内的 `const fun` 调用（按 name+arity 最小选择）。
pub fn eval_const_bindings_in_file<'a>(
    source: &'a SourceFile,
    file: &'a ast::File,
) -> Result<Vec<ConstBinding>, ConstEvalError> {
    let mut interp = ConstInterpreter::with_options(ConstEvalCtx::new(source), ConstEvalOptions::default());
    interp.register_file(file);
    interp.eval_const_bindings(file)
}

/// `const fun` 调用与局部求值的解释器状态。
struct ConstInterpreter<'a> {
    ctx: ConstEvalCtx<'a>,
    options: ConstEvalOptions,
    call_depth: usize,
    /// 作用域栈（后进先出）：局部 val/参数/顶层 const val 都放在这里。
    scopes: Vec<HashMap<String, ConstValue>>,
    /// 该文件中所有顶层函数（按名称聚合；用于判断“存在但非 const”）。
    funs_by_name: HashMap<String, Vec<&'a ast::FunDecl>>,
}

impl<'a> ConstInterpreter<'a> {
    fn with_options(ctx: ConstEvalCtx<'a>, options: ConstEvalOptions) -> Self {
        Self {
            ctx,
            options,
            call_depth: 0,
            scopes: vec![HashMap::new()],
            funs_by_name: HashMap::new(),
        }
    }

    fn register_file(&mut self, file: &'a ast::File) {
        for item in &file.items {
            let ast::Item::Fun(fun) = item else { continue };
            let name = fun.name.text(self.ctx.source).to_string();
            self.funs_by_name.entry(name).or_default().push(fun);
        }
    }

    fn eval_const_bindings(&mut self, file: &'a ast::File) -> Result<Vec<ConstBinding>, ConstEvalError> {
        let mut out = Vec::new();

        for item in &file.items {
            let ast::Item::Val(v) = item else { continue };
            if !v.modifiers.contains(&ast::Modifier::Const) {
                continue;
            }

            // `const val` 目前只支持名字绑定。
            let Some(name_ident) = v.name() else {
                return Err(ConstEvalError::UnsupportedStmt {
                    kind: "const val pattern binding",
                    span: v.span.into(),
                });
            };
            if v.kind != ast::ValKind::Val {
                return Err(ConstEvalError::UnsupportedStmt {
                    kind: "const var",
                    span: v.span.into(),
                });
            }
            let Some(init) = v.init.as_ref() else {
                return Err(ConstEvalError::MissingInitializer {
                    kind: "const val",
                    span: v.span.into(),
                });
            };

            let name = name_ident.text(self.ctx.source).to_string();
            let value = eval_const_expr_with_host(self.ctx, self, init)?;

            // 顶层 const val 也进入环境：后续 const val/const fun 可引用它。
            self.define_local(name.clone(), value.clone());
            out.push(ConstBinding { name, value });
        }

        Ok(out)
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn define_local(&mut self, name: String, value: ConstValue) {
        let scope = self.scopes.last_mut().expect("at least one scope");
        scope.insert(name, value);
    }

    fn lookup(&self, name: &str) -> Option<ConstValue> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v.clone());
            }
        }
        None
    }

    fn call_const_fun(
        &mut self,
        call_span: Span,
        callee_name: &str,
        args: Vec<ConstValue>,
    ) -> Result<ConstValue, ConstEvalError> {
        let Some(candidates) = self.funs_by_name.get(callee_name) else {
            return Err(ConstEvalError::UnknownConstFun {
                name: callee_name.to_string(),
                span: call_span.into(),
            });
        };

        // 仅允许调用 `const fun`。
        let const_candidates = candidates
            .iter()
            .copied()
            .filter(|f| f.modifiers.contains(&ast::Modifier::Const))
            .collect::<Vec<_>>();
        if const_candidates.is_empty() {
            return Err(ConstEvalError::CalleeNotConstFun {
                name: callee_name.to_string(),
                span: call_span.into(),
            });
        }

        let arity = args.len();
        let arity_matches = const_candidates
            .into_iter()
            .filter(|f| f.params.len() == arity)
            .collect::<Vec<_>>();

        let fun = match arity_matches.as_slice() {
            [] => {
                // 早期阶段只按 arity 匹配；默认参数/命名参数/重载决议留给后续阶段。
                let expected = candidates
                    .iter()
                    .filter(|f| f.modifiers.contains(&ast::Modifier::Const))
                    .map(|f| f.params.len())
                    .min()
                    .unwrap_or(0);
                return Err(ConstEvalError::ConstFunArityMismatch {
                    name: callee_name.to_string(),
                    expected,
                    found: arity,
                    span: call_span.into(),
                });
            }
            [one] => *one,
            _ => {
                return Err(ConstEvalError::ConstFunAmbiguous {
                    name: callee_name.to_string(),
                    arity,
                    span: call_span.into(),
                });
            }
        };

        self.eval_fun_call(call_span, fun, args)
    }

    fn eval_fun_call(
        &mut self,
        call_span: Span,
        fun: &'a ast::FunDecl,
        args: Vec<ConstValue>,
    ) -> Result<ConstValue, ConstEvalError> {
        if self.call_depth >= self.options.recursion_limit {
            return Err(ConstEvalError::RecursionLimitExceeded {
                name: fun.name.text(self.ctx.source).to_string(),
                limit: self.options.recursion_limit,
                span: call_span.into(),
            });
        }

        // 解释器入口做一次“最小签名门禁”，避免把复杂语义带入 v0。
        if fun.receiver.is_some() {
            return Err(ConstEvalError::UnsupportedConstFunSignature {
                reason: "extension receiver",
                span: fun.span.into(),
            });
        }
        if !fun.type_params.is_empty() {
            return Err(ConstEvalError::UnsupportedConstFunSignature {
                reason: "generic type params",
                span: fun.span.into(),
            });
        }
        if fun.eff_param.is_some() {
            return Err(ConstEvalError::UnsupportedConstFunSignature {
                reason: "effect row param",
                span: fun.span.into(),
            });
        }
        if fun.params.len() != args.len() {
            return Err(ConstEvalError::ConstFunArityMismatch {
                name: fun.name.text(self.ctx.source).to_string(),
                expected: fun.params.len(),
                found: args.len(),
                span: call_span.into(),
            });
        }

        self.call_depth += 1;
        self.push_scope();

        // 参数绑定写入当前 frame scope。
        for (param, arg) in fun.params.iter().zip(args) {
            let name = param.name.text(self.ctx.source).to_string();
            self.define_local(name, arg);
        }

        let ret = match &fun.body {
            ast::FunBody::Block(b) => match self.eval_block(b)? {
                ControlFlow::Break(v) | ControlFlow::Continue(v) => v,
            },
            ast::FunBody::Missing => {
                return Err(ConstEvalError::UnsupportedConstFunSignature {
                    reason: "missing body",
                    span: fun.span.into(),
                });
            }
        };

        self.pop_scope();
        self.call_depth -= 1;
        Ok(ret)
    }

    fn eval_block(&mut self, block: &ast::Block) -> Result<ControlFlow<ConstValue, ConstValue>, ConstEvalError> {
        // block 自带一个子作用域（与 resolver/typecheck 的“block 内声明仅在该 block 内可见”一致）。
        self.push_scope();

        let mut last_value = ConstValue::Unit;
        for stmt in &block.stmts {
            match self.eval_stmt(stmt)? {
                ControlFlow::Break(ret) => {
                    self.pop_scope();
                    return Ok(ControlFlow::Break(ret));
                }
                ControlFlow::Continue(Some(v)) => last_value = v,
                ControlFlow::Continue(None) => {}
            }
        }

        self.pop_scope();
        Ok(ControlFlow::Continue(last_value))
    }

    fn eval_stmt(
        &mut self,
        stmt: &ast::Stmt,
    ) -> Result<ControlFlow<ConstValue, Option<ConstValue>>, ConstEvalError> {
        match &stmt.kind {
            ast::StmtKind::Empty => Ok(ControlFlow::Continue(None)),
            ast::StmtKind::Expr(e) => {
                let v = eval_const_expr_with_host(self.ctx, self, e)?;
                Ok(ControlFlow::Continue(Some(v)))
            }
            ast::StmtKind::Val(v) => {
                if v.kind != ast::ValKind::Val {
                    return Err(ConstEvalError::UnsupportedStmt {
                        kind: "local var",
                        span: v.span.into(),
                    });
                }
                let Some(name) = v.name() else {
                    return Err(ConstEvalError::UnsupportedStmt {
                        kind: "local val pattern binding",
                        span: v.span.into(),
                    });
                };
                let Some(init) = v.init.as_ref() else {
                    return Err(ConstEvalError::MissingInitializer {
                        kind: "local val",
                        span: v.span.into(),
                    });
                };

                let value = eval_const_expr_with_host(self.ctx, self, init)?;
                self.define_local(name.text(self.ctx.source).to_string(), value);
                Ok(ControlFlow::Continue(None))
            }
            ast::StmtKind::Return { value, .. } => {
                let v = match value {
                    Some(expr) => eval_const_expr_with_host(self.ctx, self, expr)?,
                    None => ConstValue::Unit,
                };
                Ok(ControlFlow::Break(v))
            }
            ast::StmtKind::While { .. } => Err(ConstEvalError::UnsupportedStmt {
                kind: "while",
                span: stmt.span.into(),
            }),
            ast::StmtKind::Break { .. } => Err(ConstEvalError::UnsupportedStmt {
                kind: "break",
                span: stmt.span.into(),
            }),
            ast::StmtKind::Continue { .. } => Err(ConstEvalError::UnsupportedStmt {
                kind: "continue",
                span: stmt.span.into(),
            }),
            ast::StmtKind::ComptimeBlock { .. } => Err(ConstEvalError::UnsupportedStmt {
                kind: "comptime block",
                span: stmt.span.into(),
            }),
            ast::StmtKind::ComptimeIf(_) => Err(ConstEvalError::UnsupportedStmt {
                kind: "comptime if",
                span: stmt.span.into(),
            }),
            ast::StmtKind::ComptimeFor(_) => Err(ConstEvalError::UnsupportedStmt {
                kind: "comptime for",
                span: stmt.span.into(),
            }),
            ast::StmtKind::Missing => Err(ConstEvalError::UnsupportedStmt {
                kind: "missing stmt",
                span: stmt.span.into(),
            }),
        }
    }
}

impl ConstEvalHost for ConstInterpreter<'_> {
    fn resolve_ident(&mut self, name: &str) -> Option<ConstValue> {
        self.lookup(name)
    }

    fn call_fun(
        &mut self,
        call_span: Span,
        callee_name: &str,
        args: Vec<ConstValue>,
    ) -> Result<ConstValue, ConstEvalError> {
        self.call_const_fun(call_span, callee_name, args)
    }
}

