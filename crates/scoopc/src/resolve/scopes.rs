//! 块级作用域（val/var）与最小值名字解析（T0304）。
//!
//! 目标：
//! - 在 resolve 阶段为 block 建立 scope 栈；
//! - 记录局部 `val/var` 绑定，并允许嵌套块遮蔽（shadowing）；
//! - 对表达式中的裸标识符（`ExprKind::Ident`）做最小“存在性”解析：
//!   - 先查局部作用域栈
//!   - 再查同包/导入的顶层 fun/value（避免把明显合法的顶层调用误报为未定义）
//!
//! 非目标（后续任务处理）：
//! - 不把解析结果写回 AST/HIR（T0305）
//! - 不解析成员访问（`.`）的目标（T0310）
//! - 不实现 `this`/构造参数/成员初始化的特殊作用域规则（T0313）

use std::collections::HashMap;

use crate::{ast, source::SourceFile, span::Span};

use super::{ImportTable, Index, ResolveError, SymbolKind, package_prefix};

pub(super) fn check_block_scopes(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
) -> Result<(), ResolveError> {
    let mut checker = BlockScopeChecker::new(source, file, index, imports);
    checker.check_file()
}

struct BlockScopeChecker<'a> {
    source: &'a SourceFile,
    file: &'a ast::File,
    index: &'a Index,
    imports: &'a ImportTable,
    pkg_prefix: String,
    scopes: Vec<HashMap<String, Span>>,
}

impl<'a> BlockScopeChecker<'a> {
    fn new(
        source: &'a SourceFile,
        file: &'a ast::File,
        index: &'a Index,
        imports: &'a ImportTable,
    ) -> Self {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        Self {
            source,
            file,
            index,
            imports,
            pkg_prefix,
            scopes: Vec::new(),
        }
    }

    fn check_file(&mut self) -> Result<(), ResolveError> {
        for item in &self.file.items {
            match item {
                ast::Item::Fun(fun) => self.check_fun(fun)?,
                ast::Item::Type(ty) => self.check_type_decl(ty)?,
                ast::Item::Val(_) | ast::Item::TypeAlias(_) => {}
            }
        }
        Ok(())
    }

    fn check_type_decl(&mut self, ty: &ast::TypeDecl) -> Result<(), ResolveError> {
        let Some(body) = &ty.body else {
            return Ok(());
        };

        for member in &body.members {
            match member {
                ast::TypeMember::Property(_) => {
                    // T0313 会引入 `this`/构造参数/成员初始化作用域等规则。
                    // 在那之前，避免对属性 init/accessor body 做值解析，以免产生大量误报。
                }
                ast::TypeMember::Fun(fun) => self.check_fun(fun)?,
                ast::TypeMember::Type(nested) => self.check_type_decl(nested)?,
            }
        }

        Ok(())
    }

    fn check_fun(&mut self, fun: &ast::FunDecl) -> Result<(), ResolveError> {
        // 函数级作用域：先放入参数（允许 block 内部遮蔽参数名）。
        self.push_scope();
        for p in &fun.params {
            self.declare_ident(&p.name)?;
            if let Some(default) = &p.default_value {
                self.check_expr(default)?;
            }
        }

        match &fun.body {
            ast::FunBody::Block(b) => self.check_block(b)?,
            ast::FunBody::Missing => {}
        }

        self.pop_scope();
        Ok(())
    }

    fn check_block(&mut self, block: &ast::Block) -> Result<(), ResolveError> {
        self.push_scope();

        // 语义：局部变量在其声明之后可见；因此先解析 init，再把 bind 注入当前 scope。
        for stmt in &block.stmts {
            self.check_stmt(stmt)?;
        }

        self.pop_scope();
        Ok(())
    }

    fn check_stmt(&mut self, stmt: &ast::Stmt) -> Result<(), ResolveError> {
        match &stmt.kind {
            ast::StmtKind::Empty | ast::StmtKind::Break { .. } | ast::StmtKind::Continue { .. } => {
            }
            ast::StmtKind::Expr(e) => self.check_expr(e)?,
            ast::StmtKind::Val(v) => self.check_val_decl(v)?,
            ast::StmtKind::Return { value, .. } => {
                if let Some(v) = value {
                    self.check_expr(v)?;
                }
            }
            ast::StmtKind::While { cond, body, .. } => {
                self.check_expr(cond)?;
                self.check_block(body)?;
            }
            ast::StmtKind::ComptimeBlock { body, .. } => self.check_block(body)?,
            ast::StmtKind::ComptimeIf(ci) => self.check_comptime_if(ci)?,
            ast::StmtKind::ComptimeFor(cf) => self.check_comptime_for(cf)?,
            ast::StmtKind::Missing => {}
        }

        Ok(())
    }

    fn check_comptime_if(&mut self, ci: &ast::ComptimeIf) -> Result<(), ResolveError> {
        self.check_expr(&ci.cond)?;
        self.check_block(&ci.then_branch)?;

        let Some(else_branch) = &ci.else_branch else {
            return Ok(());
        };

        match &**else_branch {
            ast::ComptimeIfElse::Block(b) => self.check_block(b)?,
            ast::ComptimeIfElse::If(next) => self.check_comptime_if(next)?,
        }

        Ok(())
    }

    fn check_comptime_for(&mut self, cf: &ast::ComptimeFor) -> Result<(), ResolveError> {
        self.check_expr(&cf.iter)?;

        // `comptime for (x in xs) { ... }` 的 binder 在 body 作用域内可见。
        self.push_scope();
        self.declare_ident(&cf.binder)?;
        self.check_block(&cf.body)?;
        self.pop_scope();

        Ok(())
    }

    fn check_val_decl(&mut self, v: &ast::ValDecl) -> Result<(), ResolveError> {
        if let Some(init) = &v.init {
            self.check_expr(init)?;
        }

        for b in bound_idents(&v.binding) {
            self.declare_ident(&b)?;
        }

        Ok(())
    }

    fn check_expr(&mut self, expr: &ast::Expr) -> Result<(), ResolveError> {
        match &expr.kind {
            ast::ExprKind::Missing | ast::ExprKind::IntLit | ast::ExprKind::StringLit => {}
            ast::ExprKind::Ident(id) => self.check_value_ident(id)?,
            ast::ExprKind::InterpolatedString { parts, .. } => {
                for p in parts {
                    if let ast::InterpolatedStringPart::Expr { expr } = p {
                        self.check_expr(expr)?;
                    }
                }
            }
            ast::ExprKind::Block(b) => self.check_block(b)?,
            ast::ExprKind::Lambda(lam) => {
                // lambda 的参数在其 body 内可见；暂不记录 capture 信息（非目标）。
                self.push_scope();
                for p in &lam.params {
                    self.declare_ident(&p.name)?;
                    if let Some(default) = &p.default_value {
                        self.check_expr(default)?;
                    }
                }
                self.check_expr(&lam.body)?;
                self.pop_scope();
            }
            ast::ExprKind::StructLit { fields, .. } => {
                for f in fields {
                    self.check_expr(&f.value)?;
                }
            }
            ast::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.check_expr(cond)?;
                self.check_expr(then_branch)?;
                if let Some(e) = else_branch {
                    self.check_expr(e)?;
                }
            }
            ast::ExprKind::When { subject, arms } => {
                self.check_expr(subject)?;
                for arm in arms {
                    self.check_expr(&arm.body)?;
                }
            }
            ast::ExprKind::MemberAccess { receiver, .. }
            | ast::ExprKind::SafeMemberAccess { receiver, .. } => {
                // 仅解析 receiver；member 本身的解析留到 T0310。
                self.check_expr(receiver)?;
            }
            ast::ExprKind::SpliceField { receiver, field } => {
                self.check_expr(receiver)?;
                self.check_expr(field)?;
            }
            ast::ExprKind::Call { callee, args } => {
                self.check_expr(callee)?;
                for a in args {
                    self.check_expr(a)?;
                }
            }
            ast::ExprKind::NamedArg { value, .. } => self.check_expr(value)?,
            ast::ExprKind::NotNullAssert { expr, .. } => self.check_expr(expr)?,
            ast::ExprKind::Unary { expr, .. } => self.check_expr(expr)?,
            ast::ExprKind::Binary { lhs, rhs, .. } => {
                self.check_expr(lhs)?;
                self.check_expr(rhs)?;
            }
            ast::ExprKind::Assign { lhs, rhs, .. } => {
                self.check_expr(lhs)?;
                self.check_expr(rhs)?;
            }
            ast::ExprKind::TypeCheck { expr, .. } => self.check_expr(expr)?,
            ast::ExprKind::Cast { expr, .. } => self.check_expr(expr)?,
            ast::ExprKind::WithUpdate { base, updates, .. } => {
                self.check_expr(base)?;
                for u in updates {
                    self.check_expr(&u.value)?;
                }
            }
        }

        Ok(())
    }

    fn check_value_ident(&self, id: &ast::Ident) -> Result<(), ResolveError> {
        let name = self.source.slice(id.span);

        // `this` 的精确作用域规则在 T0313；此处先作为保守放行，避免误报。
        if name == "this" {
            return Ok(());
        }

        if self.is_value_name_resolved(name) {
            return Ok(());
        }

        Err(ResolveError::UnresolvedValue {
            name: name.to_string(),
            span: id.span.into(),
        })
    }

    fn is_value_name_resolved(&self, name: &str) -> bool {
        self.is_in_local_scope(name) || self.is_in_top_level(name)
    }

    fn is_in_local_scope(&self, name: &str) -> bool {
        self.scopes
            .iter()
            .rev()
            .any(|frame| frame.contains_key(name))
    }

    fn is_in_top_level(&self, name: &str) -> bool {
        let mut candidates: Vec<String> = Vec::new();

        if !self.pkg_prefix.is_empty() {
            candidates.push(format!("{}.{}", self.pkg_prefix, name));
        }
        // 允许用户引用 root package 下的符号（无 package 声明时也可匹配）。
        candidates.push(name.to_string());

        if let Some(fqns) = self.imports.value.explicit.get(name) {
            candidates.extend(fqns.iter().cloned());
        }
        for prefix in &self.imports.star {
            candidates.push(format!("{prefix}.{name}"));
        }

        candidates.sort();
        candidates.dedup();

        for fqn in candidates {
            if self.index.by_fqn.get(&fqn).is_some_and(|syms| {
                syms.get(SymbolKind::Fun).is_some() || syms.get(SymbolKind::Value).is_some()
            }) {
                return true;
            }
        }

        false
    }

    fn declare_ident(&mut self, id: &ast::Ident) -> Result<(), ResolveError> {
        let name = self.source.slice(id.span).to_string();
        self.declare_name(name, id.span)
    }

    fn declare_name(&mut self, name: String, span: Span) -> Result<(), ResolveError> {
        let Some(cur) = self.scopes.last_mut() else {
            // 作为防御性兜底：理论上任何 declare 都应该发生在 push_scope 之后。
            self.scopes.push(HashMap::new());
            return self.declare_name(name, span);
        };

        if let Some(prev) = cur.get(&name) {
            return Err(ResolveError::DuplicateDefinition {
                name,
                first: (*prev).into(),
                second: span.into(),
            });
        }

        cur.insert(name, span);
        Ok(())
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        let _ = self.scopes.pop();
    }
}

fn bound_idents(binding: &ast::ValBinding) -> Vec<ast::Ident> {
    match binding {
        ast::ValBinding::Name(id) => vec![*id],
        ast::ValBinding::Pattern(p) => bound_idents_in_pattern(p),
    }
}

fn bound_idents_in_pattern(p: &ast::Pattern) -> Vec<ast::Ident> {
    let mut out = Vec::new();
    collect_bound_idents_in_pattern(p, &mut out);
    out
}

fn collect_bound_idents_in_pattern(p: &ast::Pattern, out: &mut Vec<ast::Ident>) {
    match &p.kind {
        ast::PatternKind::Wildcard | ast::PatternKind::Missing => {}
        ast::PatternKind::Bind(id) => out.push(*id),
        ast::PatternKind::Tuple(parts) => {
            for part in parts {
                collect_bound_idents_in_pattern(part, out);
            }
        }
        ast::PatternKind::Struct { fields, .. } => {
            for f in fields {
                match &f.value {
                    Some(v) => collect_bound_idents_in_pattern(v, out),
                    None => {
                        // shorthand：`Point { x }` 会把 `x` 作为一个绑定引入作用域。
                        out.push(f.name);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_file;

    fn build_index<'a>(pairs: &[(&'a SourceFile, &'a ast::File)]) -> Index {
        Index::build(pairs).unwrap()
    }

    #[test]
    fn unresolved_value_in_fun_body_is_error() {
        let src = SourceFile::new_virtual("<mem>", "package a\nfun f() { x }");
        let file = parse_file(&src).unwrap();
        let index = build_index(&[(&src, &file)]);

        let err = super::super::check_file_bindings(&src, &file, &index).unwrap_err();
        assert!(matches!(err, ResolveError::UnresolvedValue { .. }));
    }

    #[test]
    fn local_shadowing_in_nested_block_is_ok() {
        let src = SourceFile::new_virtual(
            "<mem>",
            "package a\nfun f() { val x = 1\n{ val x = 2\nx }\nx }",
        );
        let file = parse_file(&src).unwrap();
        let index = build_index(&[(&src, &file)]);

        super::super::check_file_bindings(&src, &file, &index).unwrap();
    }
}
