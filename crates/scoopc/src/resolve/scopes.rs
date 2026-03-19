//! 块级作用域（val/var）与表达式裸标识符解析（T0304/T0305）。
//!
//! 目标：
//! - 在 resolve 阶段为 block 建立 scope 栈；
//! - 记录局部 `val/var` 绑定，并允许嵌套块遮蔽（shadowing）；
//! - 对表达式中的裸标识符（`ExprKind::Ident`）做最小解析并写回 AST：
//!   - 先查局部作用域栈
//!   - 再查同包/导入的顶层 fun/value（避免把明显合法的顶层调用误报为未定义）
//!
//! 非目标（后续任务处理）：
//! - 不解析成员访问（`.`）的目标（T0310）
//! - 不实现 `this`/构造参数/成员初始化的特殊作用域规则（T0313）

use std::collections::HashMap;

use crate::{ast, source::SourceFile, span::Span};

use super::{
    ImportTable, Index, ResolveError, SymbolKind, Visibility, is_symbol_visible_from,
    package_prefix,
};

pub(super) fn check_block_scopes(
    source: &SourceFile,
    file: &mut ast::File,
    index: &Index,
    imports: &ImportTable,
) -> Result<(), ResolveError> {
    let mut checker = BlockScopeChecker::new(source, file, index, imports);
    checker.check_file(file)
}

struct BlockScopeChecker<'a> {
    source: &'a SourceFile,
    index: &'a Index,
    imports: &'a ImportTable,
    pkg_prefix: String,
    scopes: Vec<HashMap<String, LocalBinding>>,
}

#[derive(Clone)]
struct LocalBinding {
    decl_span: Span,
    ty: Option<ast::TypeRef>,
}

impl<'a> BlockScopeChecker<'a> {
    fn new(
        source: &'a SourceFile,
        file: &ast::File,
        index: &'a Index,
        imports: &'a ImportTable,
    ) -> Self {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        Self {
            source,
            index,
            imports,
            pkg_prefix,
            scopes: Vec::new(),
        }
    }

    fn check_file(&mut self, file: &mut ast::File) -> Result<(), ResolveError> {
        for item in &mut file.items {
            match item {
                ast::Item::Fun(fun) => self.check_fun(fun)?,
                ast::Item::Type(ty) => self.check_type_decl(ty)?,
                ast::Item::Val(v) => self.check_top_level_val(v)?,
                ast::Item::TypeAlias(_) => {}
            }
        }
        Ok(())
    }

    fn check_top_level_val(&mut self, v: &mut ast::ValDecl) -> Result<(), ResolveError> {
        // 顶层 `val/var` 的 initializer 不引入额外局部作用域；
        // 但 initializer 内部若包含 block/lambda 等表达式，会在对应分支里自行 push scope。
        if let Some(init) = &mut v.init {
            self.check_expr(init)?;
        }
        Ok(())
    }

    fn check_type_decl(&mut self, ty: &mut ast::TypeDecl) -> Result<(), ResolveError> {
        let Some(body) = &mut ty.body else {
            return Ok(());
        };

        for member in &mut body.members {
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

    fn check_fun(&mut self, fun: &mut ast::FunDecl) -> Result<(), ResolveError> {
        // 函数级作用域：先放入参数（允许 block 内部遮蔽参数名）。
        self.push_scope();
        for p in &mut fun.params {
            self.declare_ident_typed(&p.name, p.ty.clone())?;
            if let Some(default) = &mut p.default_value {
                self.check_expr(default)?;
            }
        }

        match &mut fun.body {
            ast::FunBody::Block(b) => self.check_block(b)?,
            ast::FunBody::Missing => {}
        }

        self.pop_scope();
        Ok(())
    }

    fn check_block(&mut self, block: &mut ast::Block) -> Result<(), ResolveError> {
        self.push_scope();

        // 语义：局部变量在其声明之后可见；因此先解析 init，再把 bind 注入当前 scope。
        for stmt in &mut block.stmts {
            self.check_stmt(stmt)?;
        }

        self.pop_scope();
        Ok(())
    }

    fn check_stmt(&mut self, stmt: &mut ast::Stmt) -> Result<(), ResolveError> {
        match &mut stmt.kind {
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

    fn check_comptime_if(&mut self, ci: &mut ast::ComptimeIf) -> Result<(), ResolveError> {
        self.check_expr(&mut ci.cond)?;
        self.check_block(&mut ci.then_branch)?;

        let Some(else_branch) = &mut ci.else_branch else {
            return Ok(());
        };

        match &mut **else_branch {
            ast::ComptimeIfElse::Block(b) => self.check_block(b)?,
            ast::ComptimeIfElse::If(next) => self.check_comptime_if(next)?,
        }

        Ok(())
    }

    fn check_comptime_for(&mut self, cf: &mut ast::ComptimeFor) -> Result<(), ResolveError> {
        self.check_expr(&mut cf.iter)?;

        // `comptime for (x in xs) { ... }` 的 binder 在 body 作用域内可见。
        self.push_scope();
        self.declare_ident(&cf.binder)?;
        self.check_block(&mut cf.body)?;
        self.pop_scope();

        Ok(())
    }

    fn check_val_decl(&mut self, v: &mut ast::ValDecl) -> Result<(), ResolveError> {
        if let Some(init) = &mut v.init {
            self.check_expr(init)?;
        }

        match &v.binding {
            ast::ValBinding::Name(id) => {
                self.declare_ident_typed(id, v.ty.clone())?;
            }
            ast::ValBinding::Pattern(_) => {
                for b in bound_idents(&v.binding) {
                    self.declare_ident(&b)?;
                }
            }
        }

        Ok(())
    }

    fn check_expr(&mut self, expr: &mut ast::Expr) -> Result<(), ResolveError> {
        match &mut expr.kind {
            ast::ExprKind::Missing | ast::ExprKind::IntLit | ast::ExprKind::StringLit => {}
            ast::ExprKind::Ident(id) => self.resolve_value_ident(id)?,
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
                for p in &mut lam.params {
                    self.declare_ident(&p.name)?;
                    if let Some(default) = &mut p.default_value {
                        self.check_expr(default)?;
                    }
                }
                self.check_expr(lam.body.as_mut())?;
                self.pop_scope();
            }
            ast::ExprKind::StructLit { fields, .. } => {
                for f in fields {
                    self.check_expr(&mut f.value)?;
                }
            }
            ast::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.check_expr(cond.as_mut())?;
                self.check_expr(then_branch.as_mut())?;
                if let Some(e) = else_branch {
                    self.check_expr(e.as_mut())?;
                }
            }
            ast::ExprKind::When { subject, arms } => {
                self.check_expr(subject.as_mut())?;
                for arm in arms {
                    self.check_expr(&mut arm.body)?;
                }
            }
            ast::ExprKind::MemberAccess { receiver, member } => {
                self.check_expr(receiver.as_mut())?;
                self.resolve_member_access(receiver.as_ref(), member)?;
            }
            ast::ExprKind::SafeMemberAccess { receiver, member, .. } => {
                self.check_expr(receiver.as_mut())?;
                self.resolve_member_access(receiver.as_ref(), member)?;
            }
            ast::ExprKind::SpliceField { receiver, field } => {
                self.check_expr(receiver.as_mut())?;
                self.check_expr(field.as_mut())?;
            }
            ast::ExprKind::Call { callee, args } => {
                self.check_expr(callee.as_mut())?;
                for a in args {
                    self.check_expr(a)?;
                }
                self.resolve_call_callee(callee.as_mut())?;
            }
            ast::ExprKind::NamedArg { value, .. } => self.check_expr(value.as_mut())?,
            ast::ExprKind::NotNullAssert { expr, .. } => self.check_expr(expr.as_mut())?,
            ast::ExprKind::Unary { expr, .. } => self.check_expr(expr.as_mut())?,
            ast::ExprKind::Binary { lhs, rhs, .. } => {
                self.check_expr(lhs.as_mut())?;
                self.check_expr(rhs.as_mut())?;
            }
            ast::ExprKind::Assign { lhs, rhs, .. } => {
                self.check_expr(lhs.as_mut())?;
                self.check_expr(rhs.as_mut())?;
            }
            ast::ExprKind::TypeCheck { expr, .. } => self.check_expr(expr.as_mut())?,
            ast::ExprKind::Cast { expr, .. } => self.check_expr(expr.as_mut())?,
            ast::ExprKind::WithUpdate { base, updates, .. } => {
                self.check_expr(base.as_mut())?;
                for u in updates {
                    self.check_expr(&mut u.value)?;
                }
            }
        }

        Ok(())
    }

    /// 解析调用表达式的 callee（T0311：`Call(Ident)` → 顶层 fun symbol）。
    ///
    /// 当前策略（最小子集）：
    /// - 仅处理 `callee` 是裸标识符（`ExprKind::Ident`）的情况；
    /// - 若该标识符解析为局部绑定（参数/局部 val），则认为是“通过值调用”（例如函数值），此处不介入；
    /// - 否则尝试按 fun 命名空间解析到顶层函数：
    ///   - 唯一匹配：写回到 `ValueIdent.resolved = TopLevel { fqn }`
    ///   - 多个匹配：报 `scoop::resolve::ambiguous_call`
    /// - 若没有任何函数候选（例如只有同名顶层 value），则保持现状交给后续 typecheck 诊断“不可调用”。
    fn resolve_call_callee(&self, callee: &mut ast::Expr) -> Result<(), ResolveError> {
        let ast::ExprKind::Ident(id) = &mut callee.kind else {
            return Ok(());
        };

        // `this(...)` 的构造调用/委托构造语义留给 T0313/后续 class 语义处理。
        if self.source.slice(id.span) == "this" {
            return Ok(());
        }

        // 局部绑定：不做顶层函数解析（保守放行）。
        if matches!(id.resolved, Some(ast::ResolvedValueRef::Local { .. })) {
            return Ok(());
        }

        let name = self.source.slice(id.span);
        if let Some(fqn) = self.top_level_fun_fqn(name, id.span)? {
            id.resolved = Some(ast::ResolvedValueRef::TopLevel { fqn });
        }

        Ok(())
    }

    fn resolve_value_ident(&mut self, id: &mut ast::ValueIdent) -> Result<(), ResolveError> {
        let name = self.source.slice(id.span);

        // `this` 的精确作用域规则在 T0313；此处先作为保守放行，避免误报。
        if name == "this" {
            return Ok(());
        }

        if let Some(binding) = self.local_binding(name) {
            id.resolved = Some(ast::ResolvedValueRef::Local {
                name: name.to_string(),
                decl_span: binding.decl_span,
            });
            return Ok(());
        }

        match self.top_level_value_fqn(name, id.span)? {
            Some(fqn) => {
                id.resolved = Some(ast::ResolvedValueRef::TopLevel { fqn });
                return Ok(());
            }
            None => {}
        }

        Err(ResolveError::UnresolvedValue {
            name: name.to_string(),
            span: id.span.into(),
        })
    }

    fn local_binding(&self, name: &str) -> Option<&LocalBinding> {
        self.scopes
            .iter()
            .rev()
            .find_map(|frame| frame.get(name))
    }

    fn top_level_value_fqn(
        &self,
        name: &str,
        use_span: Span,
    ) -> Result<Option<String>, ResolveError> {
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

        let mut not_visible: Option<(String, Visibility, Span)> = None;
        for fqn in candidates {
            let Some(syms) = self.index.by_fqn.get(&fqn) else {
                continue;
            };

            // value namespace：fun/value 任一存在即匹配（与 T0304/T0305 旧逻辑保持一致）。
            let sym_fun = syms.get(SymbolKind::Fun);
            let sym_val = syms.get(SymbolKind::Value);
            if sym_fun.is_none() && sym_val.is_none() {
                continue;
            }

            if let Some(sym) = sym_fun {
                if is_symbol_visible_from(self.source, sym) {
                    return Ok(Some(fqn));
                }
                if not_visible.is_none() {
                    not_visible = Some((fqn.clone(), sym.visibility, sym.span));
                }
            }

            if let Some(sym) = sym_val {
                if is_symbol_visible_from(self.source, sym) {
                    return Ok(Some(fqn));
                }
                if not_visible.is_none() {
                    not_visible = Some((fqn.clone(), sym.visibility, sym.span));
                }
            }
        }

        if let Some((name, visibility, def_span)) = not_visible {
            return Err(ResolveError::NotVisible {
                name,
                visibility,
                use_span: use_span.into(),
                def_span: def_span.into(),
            });
        }

        Ok(None)
    }

    fn top_level_fun_fqn(
        &self,
        name: &str,
        use_span: Span,
    ) -> Result<Option<String>, ResolveError> {
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

        let mut matches: Vec<String> = Vec::new();
        let mut not_visible: Option<(String, Visibility, Span)> = None;

        for fqn in candidates {
            let Some(syms) = self.index.by_fqn.get(&fqn) else {
                continue;
            };
            let Some(sym) = syms.get(SymbolKind::Fun) else {
                continue;
            };

            if is_symbol_visible_from(self.source, sym) {
                matches.push(fqn);
            } else if not_visible.is_none() {
                not_visible = Some((fqn.clone(), sym.visibility, sym.span));
            }
        }

        match matches.len() {
            0 => {
                if let Some((name, visibility, def_span)) = not_visible {
                    return Err(ResolveError::NotVisible {
                        name,
                        visibility,
                        use_span: use_span.into(),
                        def_span: def_span.into(),
                    });
                }
                Ok(None)
            }
            1 => Ok(matches.into_iter().next()),
            _ => Err(ResolveError::AmbiguousCall {
                name: name.to_string(),
                span: use_span.into(),
            }),
        }
    }

    fn declare_ident(&mut self, id: &ast::Ident) -> Result<(), ResolveError> {
        let name = self.source.slice(id.span).to_string();
        self.declare_name(name, id.span, None)
    }

    fn declare_ident_typed(
        &mut self,
        id: &ast::Ident,
        ty: Option<ast::TypeRef>,
    ) -> Result<(), ResolveError> {
        let name = self.source.slice(id.span).to_string();
        self.declare_name(name, id.span, ty)
    }

    fn declare_name(
        &mut self,
        name: String,
        span: Span,
        ty: Option<ast::TypeRef>,
    ) -> Result<(), ResolveError> {
        let Some(cur) = self.scopes.last_mut() else {
            // 作为防御性兜底：理论上任何 declare 都应该发生在 push_scope 之后。
            self.scopes.push(HashMap::new());
            return self.declare_name(name, span, ty);
        };

        if let Some(prev) = cur.get(&name) {
            return Err(ResolveError::DuplicateDefinition {
                name,
                first: prev.decl_span.into(),
                second: span.into(),
            });
        }

        cur.insert(
            name,
            LocalBinding {
                decl_span: span,
                ty,
            },
        );
        Ok(())
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        let _ = self.scopes.pop();
    }

    fn resolve_member_access(
        &self,
        receiver: &ast::Expr,
        member: &mut ast::MemberIdent,
    ) -> Result<(), ResolveError> {
        if member.resolved.is_some() {
            return Ok(());
        }

        let Some(receiver_ty_fqn) = self.infer_expr_type_fqn(receiver) else {
            // 当前阶段（T0310）只处理“静态可确定”的 receiver 类型；
            // 无法确定时保持保守：不做解析也不报错（避免对泛型/推断场景产生误报）。
            return Ok(());
        };

        let member_name = self.source.slice(member.span);
        let member_fqn = format!("{receiver_ty_fqn}.{member_name}");

        let Some(syms) = self.index.by_fqn.get(&member_fqn) else {
            return Err(ResolveError::UnresolvedMember {
                name: member_fqn,
                span: member.span.into(),
            });
        };

        // 与调用解析（T0311）解耦：这里只做存在性与最小“绑定写回”。
        // 目前策略：优先解析为方法（fun namespace），否则退化到字段/属性（value namespace）。
        let mut not_visible: Option<(String, Visibility, Span)> = None;

        if let Some(sym) = syms.get(SymbolKind::Fun) {
            if is_symbol_visible_from(self.source, sym) {
                member.resolved = Some(ast::ResolvedMemberRef::Fun { fqn: member_fqn });
                return Ok(());
            }
            not_visible = Some((member_fqn.clone(), sym.visibility, sym.span));
        }

        if let Some(sym) = syms.get(SymbolKind::Value) {
            if is_symbol_visible_from(self.source, sym) {
                member.resolved = Some(ast::ResolvedMemberRef::Value { fqn: member_fqn });
                return Ok(());
            }
            if not_visible.is_none() {
                not_visible = Some((member_fqn.clone(), sym.visibility, sym.span));
            }
        }

        if let Some((name, visibility, def_span)) = not_visible {
            return Err(ResolveError::NotVisible {
                name,
                visibility,
                use_span: member.span.into(),
                def_span: def_span.into(),
            });
        }

        Err(ResolveError::UnresolvedMember {
            name: member_fqn,
            span: member.span.into(),
        })
    }

    fn infer_expr_type_fqn(&self, expr: &ast::Expr) -> Option<String> {
        match &expr.kind {
            ast::ExprKind::Ident(id) => {
                let name = self.source.slice(id.span);
                if name == "this" {
                    return None;
                }

                let binding = self.local_binding(name)?;
                self.type_ref_to_fqn(binding.ty.as_ref()?)
            }
            ast::ExprKind::StructLit { ty, .. } => self.type_path_to_fqn(ty),
            _ => None,
        }
    }

    fn type_ref_to_fqn(&self, ty: &ast::TypeRef) -> Option<String> {
        match ty {
            ast::TypeRef::Path(p) => self.type_path_to_fqn(p),
            _ => None,
        }
    }

    fn type_path_to_fqn(&self, path: &ast::TypePath) -> Option<String> {
        let segments = path
            .segments
            .iter()
            .map(|id| self.source.slice(id.span))
            .collect::<Vec<_>>();
        let local = segments.join(".");

        let mut candidates: Vec<String> = Vec::new();

        // 1) 同包优先：pkg + local
        if !self.pkg_prefix.is_empty() {
            candidates.push(format!("{}.{}", self.pkg_prefix, local));
        }

        // 2) 直接使用 local（允许显式写 FQN：`scoop.core.Any`）
        candidates.push(local.clone());

        // 3) 对单段名字，应用 import 规则（显式 import / star import）
        if segments.len() == 1 {
            let name = segments[0];

            if let Some(fqns) = self.imports.ty.explicit.get(name) {
                candidates.extend(fqns.iter().cloned());
            }

            for prefix in &self.imports.star {
                candidates.push(format!("{prefix}.{name}"));
            }
        }

        candidates.sort();
        candidates.dedup();

        for fqn in candidates {
            let Some(syms) = self.index.by_fqn.get(&fqn) else {
                continue;
            };
            let Some(sym) = syms.get(SymbolKind::Type) else {
                continue;
            };
            if is_symbol_visible_from(self.source, sym) {
                return Some(fqn);
            }
        }

        None
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
        let mut file = parse_file(&src).unwrap();
        let index = build_index(&[(&src, &file)]);

        let err = super::super::check_file_bindings(&src, &mut file, &index).unwrap_err();
        assert!(matches!(err, ResolveError::UnresolvedValue { .. }));
    }

    #[test]
    fn local_shadowing_in_nested_block_is_ok() {
        let src = SourceFile::new_virtual(
            "<mem>",
            "package a\nfun f() { val x = 1\n{ val x = 2\nx }\nx }",
        );
        let mut file = parse_file(&src).unwrap();
        let index = build_index(&[(&src, &file)]);

        super::super::check_file_bindings(&src, &mut file, &index).unwrap();
    }

    #[test]
    fn value_ident_resolution_is_written_back() {
        let src = SourceFile::new_virtual("<mem>", "package a\nfun top() {}\nfun f(x) { x\ntop }");
        let mut file = parse_file(&src).unwrap();
        let index = build_index(&[(&src, &file)]);

        super::super::check_file_bindings(&src, &mut file, &index).unwrap();

        // 找到 `fun f` 的 body 里两个 Expr 语句：`x` 与 `top`。
        let fun_f = file
            .items
            .iter()
            .find_map(|it| match it {
                ast::Item::Fun(f) if src.slice(f.name.span) == "f" => Some(f),
                _ => None,
            })
            .expect("missing fun f");

        let ast::FunBody::Block(block) = &fun_f.body else {
            panic!("expected block body");
        };

        let mut idents = block
            .stmts
            .iter()
            .filter_map(|s| match &s.kind {
                ast::StmtKind::Expr(e) => match &e.kind {
                    ast::ExprKind::Ident(id) => Some(id),
                    _ => None,
                },
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(idents.len(), 2);

        // `x` 解析为局部（参数）。
        let x = idents.remove(0);
        assert!(matches!(
            x.resolved,
            Some(ast::ResolvedValueRef::Local { ref name, .. }) if name == "x"
        ));

        // `top` 解析为顶层（同包）。
        let top = idents.remove(0);
        assert_eq!(
            top.resolved,
            Some(ast::ResolvedValueRef::TopLevel {
                fqn: "a.top".to_string()
            })
        );
    }

    #[test]
    fn member_access_resolution_is_written_back() {
        let src = SourceFile::new_virtual(
            "<mem>",
            "package a\nstruct Int {}\nstruct Point { val x: Int\nfun m() {} }\nfun use(p: Point) { p.x\np.m() }",
        );
        let mut file = parse_file(&src).unwrap();
        let index = build_index(&[(&src, &file)]);

        super::super::check_file_bindings(&src, &mut file, &index).unwrap();

        let fun_use = file
            .items
            .iter()
            .find_map(|it| match it {
                ast::Item::Fun(f) if src.slice(f.name.span) == "use" => Some(f),
                _ => None,
            })
            .expect("missing fun use");

        let ast::FunBody::Block(block) = &fun_use.body else {
            panic!("expected block body");
        };

        let stmts = block
            .stmts
            .iter()
            .filter_map(|s| match &s.kind {
                ast::StmtKind::Expr(e) => Some(e),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(stmts.len(), 2);

        // `p.x` 解析到 value member。
        let ast::ExprKind::MemberAccess { member, .. } = &stmts[0].kind else {
            panic!("expected member access");
        };
        assert!(matches!(
            member.resolved,
            Some(ast::ResolvedMemberRef::Value { ref fqn }) if fqn == "a.Point.x"
        ));

        // `p.m()` 的 callee（`p.m`）解析到 fun member。
        let ast::ExprKind::Call { callee, .. } = &stmts[1].kind else {
            panic!("expected call");
        };
        let ast::ExprKind::MemberAccess { member, .. } = &callee.kind else {
            panic!("expected member access callee");
        };
        assert!(matches!(
            member.resolved,
            Some(ast::ResolvedMemberRef::Fun { ref fqn }) if fqn == "a.Point.m"
        ));
    }

    #[test]
    fn call_callee_ident_is_resolved_to_top_level_fun() {
        let src = SourceFile::new_virtual("<mem>", "package a\nfun top() {}\nfun f() { top() }");
        let mut file = parse_file(&src).unwrap();
        let index = build_index(&[(&src, &file)]);

        super::super::check_file_bindings(&src, &mut file, &index).unwrap();

        let fun_f = file
            .items
            .iter()
            .find_map(|it| match it {
                ast::Item::Fun(f) if src.slice(f.name.span) == "f" => Some(f),
                _ => None,
            })
            .expect("missing fun f");

        let ast::FunBody::Block(block) = &fun_f.body else {
            panic!("expected block body");
        };

        let Some(ast::Stmt { kind: ast::StmtKind::Expr(call), .. }) = block.stmts.first() else {
            panic!("expected first stmt to be an expr");
        };

        let ast::ExprKind::Call { callee, .. } = &call.kind else {
            panic!("expected call expr");
        };

        let ast::ExprKind::Ident(id) = &callee.kind else {
            panic!("expected ident callee");
        };

        assert_eq!(
            id.resolved,
            Some(ast::ResolvedValueRef::TopLevel {
                fqn: "a.top".to_string()
            })
        );
    }

    #[test]
    fn ambiguous_call_is_error() {
        let s1 = SourceFile::new_virtual("<p1>", "package p1\nfun foo() {}");
        let s2 = SourceFile::new_virtual("<p2>", "package p2\nfun foo() {}");
        let s3 =
            SourceFile::new_virtual("<use>", "package use\nimport p1.foo\nimport p2.foo\nfun use() { foo() }");

        let f1 = parse_file(&s1).unwrap();
        let f2 = parse_file(&s2).unwrap();
        let mut f3 = parse_file(&s3).unwrap();

        let index = Index::build(&[(&s1, &f1), (&s2, &f2), (&s3, &f3)]).unwrap();
        let err = super::super::check_file_bindings(&s3, &mut f3, &index).unwrap_err();
        assert!(matches!(err, ResolveError::AmbiguousCall { .. }));
    }

    #[test]
    fn private_top_level_is_not_visible_from_other_file() {
        let s1 = SourceFile::new_virtual("<a1>", "package a\nprivate fun hidden() {}");
        let s2 = SourceFile::new_virtual("<a2>", "package a\nfun use() { hidden() }");

        let file1 = parse_file(&s1).unwrap();
        let mut file2 = parse_file(&s2).unwrap();

        let index = Index::build(&[(&s1, &file1), (&s2, &file2)]).unwrap();
        let err = super::super::check_file_bindings(&s2, &mut file2, &index).unwrap_err();
        assert!(matches!(err, ResolveError::NotVisible { .. }));
    }
}
