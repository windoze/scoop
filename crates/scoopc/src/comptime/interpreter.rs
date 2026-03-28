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

use super::eval::{ConstEvalHost, eval_const_expr_with_host, value_kind};
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
    /// 该文件中所有顶层类型声明（按名称聚合；用于反射 intrinsics v0）。
    types_by_name: HashMap<String, Vec<&'a ast::TypeDecl>>,
}

impl<'a> ConstInterpreter<'a> {
    fn with_options(ctx: ConstEvalCtx<'a>, options: ConstEvalOptions) -> Self {
        Self {
            ctx,
            options,
            call_depth: 0,
            scopes: vec![HashMap::new()],
            funs_by_name: HashMap::new(),
            types_by_name: HashMap::new(),
        }
    }

    fn register_file(&mut self, file: &'a ast::File) {
        for item in &file.items {
            match item {
                ast::Item::Fun(fun) => {
                    let name = fun.name.text(self.ctx.source).to_string();
                    self.funs_by_name.entry(name).or_default().push(fun);
                }
                ast::Item::Type(ty) => {
                    let name = ty.name.text(self.ctx.source).to_string();
                    self.types_by_name.entry(name).or_default().push(ty);
                }
                ast::Item::TypeAlias(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::Object(_)
                | ast::Item::Val(_) => {}
            }
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

    fn call_fun_or_intrinsic(
        &mut self,
        call_span: Span,
        callee_name: &str,
        type_args: Vec<ast::TypeRef>,
        args: Vec<ConstValue>,
    ) -> Result<ConstValue, ConstEvalError> {
        // T1204：反射 intrinsics（comptime 执行时由解释器内建实现）。
        match callee_name {
            "nameOf" | "sizeOf" | "fieldsOf" => {
                return self.call_reflection_intrinsics(call_span, callee_name, type_args, args);
            }
            _ => {}
        }

        // v0：解释器不支持泛型 const fun；显式 type args 只允许用于 intrinsics。
        if !type_args.is_empty() {
            return Err(ConstEvalError::UnsupportedConstFunSignature {
                reason: "explicit type args",
                span: call_span.into(),
            });
        }

        self.call_const_fun(call_span, callee_name, args)
    }

    fn call_reflection_intrinsics(
        &mut self,
        call_span: Span,
        name: &str,
        type_args: Vec<ast::TypeRef>,
        args: Vec<ConstValue>,
    ) -> Result<ConstValue, ConstEvalError> {
        if type_args.len() != 1 || !args.is_empty() {
            return Err(ConstEvalError::ReflectionBadCall {
                name: name.to_string(),
                reason: "期望形态为 `<T>()`（1 个类型实参 + 0 个值实参）",
                span: call_span.into(),
            });
        }

        let ty_arg = &type_args[0];
        let (full_name, simple_name, ty_span) = self.type_ref_path_name_and_simple(ty_arg)?;

        match name {
            "nameOf" => Ok(ConstValue::String(full_name)),
            "sizeOf" => {
                let Some(size) = size_of_builtin_ty_bytes(&simple_name) else {
                    return Err(ConstEvalError::ReflectionSizeOfUnsupportedType {
                        name: full_name,
                        span: ty_span.into(),
                    });
                };
                Ok(ConstValue::Int(super::ConstInt::new(
                    self.ctx.default_int_ty,
                    size as u128,
                )))
            }
            "fieldsOf" => {
                let decls = self.types_by_name.get(&simple_name).ok_or_else(|| {
                    ConstEvalError::ReflectionUnknownType {
                        name: full_name.clone(),
                        span: ty_span.into(),
                    }
                })?;
                let decl = match decls.as_slice() {
                    [one] => *one,
                    _ => {
                        return Err(ConstEvalError::ReflectionAmbiguousType {
                            name: full_name.clone(),
                            span: ty_span.into(),
                        });
                    }
                };
                if decl.kind != ast::TypeKind::Struct {
                    return Err(ConstEvalError::ReflectionUnsupportedTarget {
                        name: full_name.clone(),
                        span: ty_span.into(),
                    });
                }

                let mut fields: Vec<String> = Vec::new();
                let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

                // 1) 主构造 `val/var` 参数声明的字段
                if let Some(ctor) = decl.primary_ctor.as_ref() {
                    for p in &ctor.params {
                        if p.kind.is_none() {
                            continue;
                        }
                        let fname = p.name.text(self.ctx.source).to_string();
                        if !seen.insert(fname.clone()) {
                            return Err(ConstEvalError::ReflectionDuplicateField {
                                field: fname,
                                span: ty_span.into(),
                            });
                        }
                        fields.push(fname);
                    }
                }

                // 2) type body 里“看起来像 backing field 的属性声明”
                if let Some(body) = decl.body.as_ref() {
                    for m in &body.members {
                        let ast::TypeMember::Property(p) = m else { continue };

                        // v0：只把“无 delegate、无自定义 getter/setter”的属性当作字段。
                        if p.delegate.is_some() || p.getter.is_some() || p.setter.is_some() {
                            continue;
                        }

                        let fname = p.name.text(self.ctx.source).to_string();
                        if !seen.insert(fname.clone()) {
                            return Err(ConstEvalError::ReflectionDuplicateField {
                                field: fname,
                                span: ty_span.into(),
                            });
                        }
                        fields.push(fname);
                    }
                }

                Ok(ConstValue::Tuple(
                    fields.into_iter().map(ConstValue::String).collect(),
                ))
            }
            _ => Err(ConstEvalError::ReflectionBadCall {
                name: name.to_string(),
                reason: "unknown reflection intrinsic",
                span: call_span.into(),
            }),
        }
    }

    fn type_ref_path_name_and_simple(
        &self,
        ty: &ast::TypeRef,
    ) -> Result<(String, String, Span), ConstEvalError> {
        let ast::TypeRef::Path(p) = ty else {
            return Err(ConstEvalError::ReflectionTypeArgNotSupported {
                found: "non-path type",
                span: ty.span().into(),
            });
        };

        let mut full = String::new();
        for (idx, seg) in p.segments.iter().enumerate() {
            if idx > 0 {
                full.push('.');
            }
            full.push_str(seg.text(self.ctx.source));
        }

        let simple = p
            .segments
            .last()
            .map(|s| s.text(self.ctx.source).to_string())
            .unwrap_or_default();

        Ok((full, simple, p.span))
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
            ast::StmtKind::ComptimeBlock { body, .. } => match self.eval_block(body)? {
                ControlFlow::Break(ret) => Ok(ControlFlow::Break(ret)),
                ControlFlow::Continue(v) => Ok(ControlFlow::Continue(Some(v))),
            },
            ast::StmtKind::ComptimeIf(ci) => match self.eval_comptime_if(ci)? {
                ControlFlow::Break(ret) => Ok(ControlFlow::Break(ret)),
                ControlFlow::Continue(v) => Ok(ControlFlow::Continue(Some(v))),
            },
            ast::StmtKind::ComptimeFor(cf) => match self.eval_comptime_for(cf)? {
                ControlFlow::Break(ret) => Ok(ControlFlow::Break(ret)),
                ControlFlow::Continue(v) => Ok(ControlFlow::Continue(Some(v))),
            },
            ast::StmtKind::Missing => Err(ConstEvalError::UnsupportedStmt {
                kind: "missing stmt",
                span: stmt.span.into(),
            }),
        }
    }

    fn eval_comptime_if(
        &mut self,
        ci: &ast::ComptimeIf,
    ) -> Result<ControlFlow<ConstValue, ConstValue>, ConstEvalError> {
        // `comptime if`：在编译期求值条件，仅执行被选中的分支（未选中分支不求值）。
        let cond_v = eval_const_expr_with_host(self.ctx, self, &ci.cond)?;
        let ConstValue::Bool(cond_b) = cond_v else {
            return Err(ConstEvalError::OperandTypeMismatch {
                expected: "Bool",
                found: value_kind(&cond_v),
                span: ci.cond.span.into(),
            });
        };

        if cond_b {
            return self.eval_block(&ci.then_branch);
        }

        match &ci.else_branch {
            None => Ok(ControlFlow::Continue(ConstValue::Unit)),
            Some(else_branch) => match &**else_branch {
                ast::ComptimeIfElse::Block(b) => self.eval_block(b),
                ast::ComptimeIfElse::If(nested) => self.eval_comptime_if(nested),
            },
        }
    }

    fn eval_comptime_for(
        &mut self,
        cf: &ast::ComptimeFor,
    ) -> Result<ControlFlow<ConstValue, ConstValue>, ConstEvalError> {
        // `comptime for (x in xs) { ... }`：
        // - 先在编译期求值 iter；
        // - 对可迭代对象进行“展开执行”，每次迭代把 binder 绑定到当前元素；
        // - v0：仅支持整数范围 `a..b` 与 tuple/array（以 ConstValue::Tuple 承载）。
        let binder_name = cf.binder.text(self.ctx.source).to_string();

        // 1) 整数范围：`a..b`
        if let ast::ExprKind::Binary {
            lhs,
            op: ast::BinaryOp::RangeInclusive,
            rhs,
            ..
        } = &cf.iter.kind
        {
            let lv = eval_const_expr_with_host(self.ctx, self, lhs)?;
            let li = match lv {
                ConstValue::Int(i) => i,
                other => {
                    return Err(ConstEvalError::OperandTypeMismatch {
                        expected: "整数",
                        found: value_kind(&other),
                        span: lhs.span.into(),
                    });
                }
            };

            let rv = eval_const_expr_with_host(self.ctx, self, rhs)?;
            let ri = match rv {
                ConstValue::Int(i) => i,
                other => {
                    return Err(ConstEvalError::OperandTypeMismatch {
                        expected: "整数",
                        found: value_kind(&other),
                        span: rhs.span.into(),
                    });
                }
            };
            if li.ty != ri.ty {
                return Err(ConstEvalError::OperandTypeMismatch {
                    expected: "相同的整数类型",
                    found: "不同位宽/符号位的整数",
                    span: cf.iter.span.into(),
                });
            }

            let mut last_value = ConstValue::Unit;

            if li.ty.signed {
                let mut cur = li.as_i128();
                let end = ri.as_i128();
                while cur <= end {
                    self.push_scope();
                    self.define_local(binder_name.clone(), ConstValue::Int(super::ConstInt::new(li.ty, cur as u128)));
                    match self.eval_block(&cf.body)? {
                        ControlFlow::Break(ret) => {
                            self.pop_scope();
                            return Ok(ControlFlow::Break(ret));
                        }
                        ControlFlow::Continue(v) => {
                            last_value = v;
                        }
                    }
                    self.pop_scope();

                    let Some(next) = cur.checked_add(1) else { break };
                    cur = next;
                }
            } else {
                let mut cur = li.as_u128();
                let end = ri.as_u128();
                while cur <= end {
                    self.push_scope();
                    self.define_local(binder_name.clone(), ConstValue::Int(super::ConstInt::new(li.ty, cur)));
                    match self.eval_block(&cf.body)? {
                        ControlFlow::Break(ret) => {
                            self.pop_scope();
                            return Ok(ControlFlow::Break(ret));
                        }
                        ControlFlow::Continue(v) => {
                            last_value = v;
                        }
                    }
                    self.pop_scope();

                    let Some(next) = cur.checked_add(1) else { break };
                    cur = next;
                }
            }

            return Ok(ControlFlow::Continue(last_value));
        }

        // 2) tuple/array（v0：统一用 Tuple 承载，见 comptime::eval）
        let iter_v = eval_const_expr_with_host(self.ctx, self, &cf.iter)?;
        let ConstValue::Tuple(items) = iter_v else {
            return Err(ConstEvalError::OperandTypeMismatch {
                expected: "Tuple（可迭代）",
                found: value_kind(&iter_v),
                span: cf.iter.span.into(),
            });
        };

        let mut last_value = ConstValue::Unit;
        for item in items {
            self.push_scope();
            self.define_local(binder_name.clone(), item);

            match self.eval_block(&cf.body)? {
                ControlFlow::Break(ret) => {
                    self.pop_scope();
                    return Ok(ControlFlow::Break(ret));
                }
                ControlFlow::Continue(v) => {
                    last_value = v;
                }
            }

            self.pop_scope();
        }

        Ok(ControlFlow::Continue(last_value))
    }
}

fn size_of_builtin_ty_bytes(name: &str) -> Option<usize> {
    match name {
        // scalar/value types
        "Bool" => Some(std::mem::size_of::<bool>()),
        "Unit" => Some(std::mem::size_of::<()>()),
        "Int" => Some(std::mem::size_of::<isize>()),
        "UInt" | "UIntPtr" => Some(std::mem::size_of::<usize>()),
        "Int8" | "UInt8" | "Byte" => Some(1),
        "Int16" | "UInt16" | "Short" | "UShort" => Some(2),
        "Int32" | "UInt32" => Some(4),
        "Int64" | "UInt64" | "Long" | "ULong" => Some(8),

        // 引用类型：v0 先把它们视为“指针大小”。
        "String" => Some(std::mem::size_of::<usize>()),

        _ => None,
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
        type_args: Vec<ast::TypeRef>,
        args: Vec<ConstValue>,
    ) -> Result<ConstValue, ConstEvalError> {
        self.call_fun_or_intrinsic(call_span, callee_name, type_args, args)
    }
}
