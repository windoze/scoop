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
//! - 不实现 `super`、capture/闭包等更复杂的语义（T0313 之后再补）

use std::collections::{HashMap, HashSet};

use crate::{ast, source::SourceFile, span::Span};

use super::{
    ConeId, ImportTable, Index, ResolveError, SymbolKind, Visibility, is_symbol_visible_from,
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
    use_cone: ConeId,
    pkg_prefix: String,
    scopes: Vec<HashMap<String, LocalBinding>>,
    /// `this` 允许出现的上下文栈：
    /// - 类型体成员（property init/accessor、member fun）里：`this` 指向当前类型实例
    /// - 扩展函数体里：`this` 指向 extension receiver
    this_context: Vec<ThisContext>,
    /// class 初始化阶段的“可见成员集合”栈（T0316）。
    ///
    /// 用途：在 property initializer / `init { ... }` 中禁止访问“尚未初始化”的后置属性，
    /// 并给出稳定诊断（`scoop::resolve::forward_reference`）。
    init_value_members: Vec<InitValueMembersContext>,
}

#[derive(Clone)]
struct LocalBinding {
    decl_span: Span,
    ty: Option<ast::TypeRef>,
}

#[derive(Debug, Clone)]
struct ThisContext {
    /// 用作 `ResolvedValueRef::Local { decl_span }` 的最小“身份标识”。
    decl_span: Span,
    /// `this` 的类型 FQN（用于 `this.member` 的成员解析）；若无法静态确定则为 None。
    ty_fqn: Option<String>,
}

#[derive(Debug, Clone)]
struct InitValueMembersContext {
    /// `this` 的静态类型 FQN。
    this_ty_fqn: String,
    /// 在当前初始化点之前，已经完成初始化（或至少已声明）的 value members（属性/字段）集合。
    ///
    /// 说明：这里刻意只追踪 value namespace。
    /// - methods（fun namespace）在 Kotlin 语义下不受“前向引用”限制；
    /// - 具体的初始化顺序与更深语义由 typecheck 处理（见 TODO T0316 目标）。
    visible_value_members: HashSet<String>,
}

#[derive(Debug, Clone)]
struct BackingFieldBinding {
    decl_span: Span,
    ty: Option<ast::TypeRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MemberReceiverKind {
    /// receiver 是一个值表达式，且其静态类型（FQN）可确定。
    Value { ty_fqn: String },
    /// receiver 是一个类型名（例如 `C.member` 中的 `C`）。
    Type { ty_fqn: String },
}

impl<'a> BlockScopeChecker<'a> {
    fn new(
        source: &'a SourceFile,
        file: &ast::File,
        index: &'a Index,
        imports: &'a ImportTable,
    ) -> Self {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        let use_cone = index.cone_of_source(source);
        Self {
            source,
            index,
            imports,
            use_cone,
            pkg_prefix,
            scopes: Vec::new(),
            this_context: Vec::new(),
            init_value_members: Vec::new(),
        }
    }

    fn check_file(&mut self, file: &mut ast::File) -> Result<(), ResolveError> {
        // 这里先拷贝一份，避免在可变借用 `self` 时同时借用 `self.pkg_prefix` 导致借用冲突。
        let pkg_prefix = self.pkg_prefix.clone();

        for item in &mut file.items {
            match item {
                ast::Item::Fun(fun) => self.check_fun(fun)?,
                ast::Item::ExtensionProperty(p) => self.check_extension_property(p)?,
                ast::Item::Type(ty) => self.check_type_decl(ty, &pkg_prefix)?,
                ast::Item::Object(obj) => self.check_object_decl(obj, &pkg_prefix)?,
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

    fn check_type_decl(
        &mut self,
        ty: &mut ast::TypeDecl,
        prefix: &str,
    ) -> Result<(), ResolveError> {
        // T0313：主构造参数默认值属于“初始化语境”，但不引入 `this`；
        // 这里先做最小值名字解析（可引用同一 ctor 中更早声明的参数）。
        if let Some(primary_ctor) = &mut ty.primary_ctor {
            self.check_primary_ctor_defaults(primary_ctor)?;
        }

        let Some(body) = &mut ty.body else {
            return Ok(());
        };

        let type_name = self.source.slice(ty.name.span);
        let type_fqn = if prefix.is_empty() {
            type_name.to_string()
        } else {
            format!("{prefix}.{type_name}")
        };

        let ctor_params: &[ast::Param] = ty
            .primary_ctor
            .as_ref()
            .map(|c| c.params.as_slice())
            .unwrap_or(&[]);

        let this_ctx = ThisContext {
            decl_span: ty.name.span,
            ty_fqn: Some(type_fqn.clone()),
        };

        // T0316：class 初始化阶段的“前向引用”规则需要按成员声明顺序推进：
        // - property initializer / init block 只能访问在其之前已完成初始化的属性；
        // - method（fun namespace）不受该限制（Kotlin-like）。
        let mut init_visible_value_members: HashSet<String> = HashSet::new();

        for member in &mut body.members {
            match member {
                ast::TypeMember::EnumVariant(_v) => {
                    // enum variants 不引入值名字解析语境（无 initializer/body）。
                }
                ast::TypeMember::Property(p) => {
                    let init_scope = matches!(ty.kind, ast::TypeKind::Class)
                        .then_some(&init_visible_value_members);
                    self.check_type_member_property(
                        p,
                        &this_ctx,
                        ctor_params,
                        init_scope,
                        matches!(ty.kind, ast::TypeKind::Class),
                    )?;

                    // 当前属性的 initializer 已解析完成：后续 init block / property initializer 可见。
                    if matches!(ty.kind, ast::TypeKind::Class) {
                        init_visible_value_members
                            .insert(self.source.slice(p.name.span).to_string());
                    }
                }
                ast::TypeMember::InitBlock(b) => {
                    if matches!(ty.kind, ast::TypeKind::Class) {
                        self.check_type_member_init_block(
                            b,
                            &this_ctx,
                            ctor_params,
                            &init_visible_value_members,
                        )?;
                    }
                }
                ast::TypeMember::SecondaryCtor(ctor) => {
                    if matches!(ty.kind, ast::TypeKind::Class) {
                        self.check_type_member_secondary_ctor(ctor, &this_ctx, ctor_params)?;
                    }
                }
                ast::TypeMember::Fun(fun) => {
                    self.check_type_member_fun(fun, &this_ctx, ctor_params)?
                }
                ast::TypeMember::Type(nested) => self.check_type_decl(nested, &type_fqn)?,
                ast::TypeMember::Object(obj) => self.check_object_decl(obj, &type_fqn)?,
            }
        }

        Ok(())
    }

    fn check_object_decl(
        &mut self,
        obj: &mut ast::ObjectDecl,
        prefix: &str,
    ) -> Result<(), ResolveError> {
        let Some(body) = &mut obj.body else {
            return Ok(());
        };

        let obj_fqn = obj.name.as_ref().map(|name| {
            let obj_name = self.source.slice(name.span);
            if prefix.is_empty() {
                obj_name.to_string()
            } else {
                format!("{prefix}.{obj_name}")
            }
        });

        let this_ctx = ThisContext {
            decl_span: obj.name.as_ref().map(|n| n.span).unwrap_or(obj.span),
            ty_fqn: obj_fqn.clone(),
        };

        // object 没有主构造参数；成员解析时 ctor params 为空即可。
        let ctor_params: &[ast::Param] = &[];

        let nested_prefix = obj_fqn.as_deref().unwrap_or(prefix);

        for member in &mut body.members {
            match member {
                ast::TypeMember::EnumVariant(_v) => {}
                ast::TypeMember::Property(p) => {
                    self.check_type_member_property(p, &this_ctx, ctor_params, None, false)?
                }
                ast::TypeMember::InitBlock(b) => {
                    // object/companion object 也允许出现 `init { ... }`：需要在其中解析值名字引用，
                    // 以便后续 typecheck/codegen 能复用写回的绑定结果（T0828）。
                    self.with_this_context(this_ctx.clone(), |this| this.check_block(&mut b.body))?;
                }
                ast::TypeMember::SecondaryCtor(_ctor) => {}
                ast::TypeMember::Fun(fun) => {
                    self.check_type_member_fun(fun, &this_ctx, ctor_params)?
                }
                ast::TypeMember::Type(nested) => self.check_type_decl(nested, nested_prefix)?,
                ast::TypeMember::Object(nested) => self.check_object_decl(nested, nested_prefix)?,
            }
        }

        Ok(())
    }

    fn check_fun(&mut self, fun: &mut ast::FunDecl) -> Result<(), ResolveError> {
        // 扩展函数：`this` 指向 receiver（与类型体 member 的 `this` 语义并行存在）。
        if let Some(receiver) = &fun.receiver {
            let ctx = ThisContext {
                decl_span: receiver.span(),
                ty_fqn: self.type_ref_to_fqn(receiver),
            };
            return self.with_this_context(ctx, |this| this.check_fun_body(fun));
        }

        self.check_fun_body(fun)
    }

    fn check_extension_property(
        &mut self,
        p: &mut ast::ExtensionPropertyDecl,
    ) -> Result<(), ResolveError> {
        // 扩展属性：`this` 指向 receiver（与扩展函数保持一致）。
        let this_ctx = ThisContext {
            decl_span: p.receiver.span(),
            ty_fqn: self.type_ref_to_fqn(&p.receiver),
        };

        // extension property 没有主构造参数。
        let ctor_params: &[ast::Param] = &[];

        self.with_this_context(this_ctx, |this| {
            // initializer 语义上可能不被允许（由 typecheck 处理），但 resolver 仍需要在其中解析值引用，
            // 以便保持错误定位稳定、避免后续阶段重复报错。
            if let Some(init) = &mut p.init {
                this.check_expr(init)?;
            }

            // 与 class 属性一致：为 `field` 注入一个隐式局部绑定，便于后续 typecheck 给出“无 backing field”
            // 的专门诊断（而不是在 resolve 阶段提前报 UnresolvedValue）。
            let backing_field = BackingFieldBinding {
                decl_span: p.name.span,
                ty: p.ty.clone(),
            };

            if let Some(getter) = &mut p.getter {
                this.check_accessor_in_type(getter, ctor_params, Some(&backing_field))?;
            }
            if let Some(setter) = &mut p.setter {
                this.check_accessor_in_type(setter, ctor_params, Some(&backing_field))?;
            }

            Ok(())
        })?;

        Ok(())
    }

    fn check_fun_body(&mut self, fun: &mut ast::FunDecl) -> Result<(), ResolveError> {
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

    fn check_primary_ctor_defaults(
        &mut self,
        ctor: &mut ast::PrimaryCtorDecl,
    ) -> Result<(), ResolveError> {
        // 说明：
        // - 该作用域仅覆盖主构造头参数默认值（非类型体）；不引入 `this`。
        // - 与函数参数默认值一致：更早声明的参数可被后续默认值引用。
        self.push_scope();
        for p in &mut ctor.params {
            self.declare_ident_typed(&p.name, p.ty.clone())?;
            if let Some(default) = &mut p.default_value {
                self.check_expr(default)?;
            }
        }
        self.pop_scope();
        Ok(())
    }

    fn check_type_member_fun(
        &mut self,
        fun: &mut ast::FunDecl,
        this_ctx: &ThisContext,
        ctor_params: &[ast::Param],
    ) -> Result<(), ResolveError> {
        self.with_this_context(this_ctx.clone(), |this| {
            // 类型体成员：主构造参数在 member fun 内可见（T0313）。
            this.with_ctor_params_scope(ctor_params, |this| this.check_fun(fun))
        })?;
        Ok(())
    }

    fn check_type_member_property(
        &mut self,
        p: &mut ast::PropertyDecl,
        this_ctx: &ThisContext,
        ctor_params: &[ast::Param],
        init_visible_value_members: Option<&HashSet<String>>,
        inject_backing_field_ident: bool,
    ) -> Result<(), ResolveError> {
        self.with_this_context(this_ctx.clone(), |this| {
            // 属性初始化表达式：允许引用主构造参数（T0313）。
            if let Some(init) = &mut p.init {
                this.with_ctor_params_scope(ctor_params, |this| {
                    // T0316：property initializer 属于 class 初始化阶段，禁止访问“后置属性”（前向引用）。
                    //
                    // 说明：这里只对 value namespace 做限制；method 仍可通过 `this.m()` 访问。
                    if let Some(visible) = init_visible_value_members {
                        this.with_init_value_members_context(
                            this_ctx.ty_fqn.as_deref(),
                            visible,
                            |this| this.check_expr(init),
                        )?;
                        return Ok(());
                    }

                    this.check_expr(init)
                })?;
            }

            // 委托属性：delegate 表达式属于初始化语境，同样允许引用主构造参数（T0313）
            // 且遵循初始化阶段的前向引用限制（T0316）。
            if let Some(delegate) = &mut p.delegate {
                this.with_ctor_params_scope(ctor_params, |this| {
                    if let Some(visible) = init_visible_value_members {
                        this.with_init_value_members_context(
                            this_ctx.ty_fqn.as_deref(),
                            visible,
                            |this| this.check_expr(delegate),
                        )?;
                        return Ok(());
                    }
                    this.check_expr(delegate)
                })?;
            }

            // delegated property 不生成 backing field：因此不注入 `field`。
            let inject_backing_field_ident = inject_backing_field_ident && p.delegate.is_none();

            let backing_field = inject_backing_field_ident.then_some(BackingFieldBinding {
                decl_span: p.name.span,
                ty: p.ty.clone(),
            });

            if let Some(getter) = &mut p.getter {
                this.check_accessor_in_type(getter, ctor_params, backing_field.as_ref())?;
            }
            if let Some(setter) = &mut p.setter {
                this.check_accessor_in_type(setter, ctor_params, backing_field.as_ref())?;
            }

            Ok(())
        })?;
        Ok(())
    }

    fn check_type_member_init_block(
        &mut self,
        b: &mut ast::InitBlockDecl,
        this_ctx: &ThisContext,
        ctor_params: &[ast::Param],
        init_visible_value_members: &HashSet<String>,
    ) -> Result<(), ResolveError> {
        // `init { ... }` 属于 class 初始化阶段：可见 `this` + 主构造参数，
        // 且禁止访问尚未初始化的后置属性（前向引用）。
        self.with_this_context(this_ctx.clone(), |this| {
            this.with_ctor_params_scope(ctor_params, |this| {
                this.with_init_value_members_context(
                    this_ctx.ty_fqn.as_deref(),
                    init_visible_value_members,
                    |this| this.check_block(&mut b.body),
                )
            })
        })?;
        Ok(())
    }

    fn check_type_member_secondary_ctor(
        &mut self,
        ctor: &mut ast::SecondaryCtorDecl,
        this_ctx: &ThisContext,
        primary_ctor_params: &[ast::Param],
    ) -> Result<(), ResolveError> {
        // 1) 参数默认值：Kotlin-like 语义下不引入 `this`（在调用点求值），但允许引用更早声明的参数。
        self.push_scope();
        for p in &mut ctor.params {
            self.declare_ident_typed(&p.name, p.ty.clone())?;
            if let Some(default) = &mut p.default_value {
                self.check_expr(default)?;
            }
        }
        self.pop_scope();

        // 2) 构造器 body：拥有 `this`，并且主构造参数在该初始化语境内可见（与 member fun 保持一致，T0313）。
        self.with_this_context(this_ctx.clone(), |this| {
            this.with_ctor_params_scope(primary_ctor_params, |this| {
                // 构造器级作用域：参数在 body 内可见，且允许 block 内部遮蔽。
                this.push_scope();
                for p in &ctor.params {
                    this.declare_ident_typed(&p.name, p.ty.clone())?;
                }
                this.check_block(&mut ctor.body)?;
                this.pop_scope();
                Ok(())
            })
        })?;

        Ok(())
    }

    fn check_accessor_in_type(
        &mut self,
        acc: &mut ast::AccessorDecl,
        ctor_params: &[ast::Param],
        backing_field: Option<&BackingFieldBinding>,
    ) -> Result<(), ResolveError> {
        // accessor 是“函数体”语境：可见 `this`（由调用者提供 this context），并引入：
        // - 主构造参数（外层 frame）
        // - setter 形参（内层 frame）
        self.with_ctor_params_scope(ctor_params, |this| {
            this.push_scope();

            // spec §10.1：`field` 仅在属性 accessor 内可用，用作 backing field 的引用。
            //
            // 说明：
            // - 当前阶段我们把 `field` 作为一个“隐式局部绑定”注入 accessor scope，
            //   使 resolver 能为其写回 `ResolvedValueRef::Local`；
            // - 是否允许在该属性中使用 `field`（以及是否真的生成 backing field）由后续 typecheck（T0431）决定。
            if let Some(field) = backing_field {
                this.declare_name("field".to_string(), field.decl_span, field.ty.clone())?;
            }

            if let Some(param) = acc.param {
                this.declare_ident(&param)?;
            }

            match &mut acc.body {
                ast::AccessorBody::Block(b) => this.check_block(b)?,
                ast::AccessorBody::Expr(e) => this.check_expr(e)?,
                ast::AccessorBody::Missing => {}
            }

            this.pop_scope();
            Ok(())
        })?;
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
            ast::ExprKind::Missing
            | ast::ExprKind::IntLit
            | ast::ExprKind::StringLit
            | ast::ExprKind::UnitLit
            | ast::ExprKind::ClassLit { .. } => {}
            ast::ExprKind::TupleLit { elements } => {
                for e in elements {
                    self.check_expr(e)?;
                }
            }
            ast::ExprKind::ArrayLit { elements } => {
                for e in elements {
                    self.check_expr(e)?;
                }
            }
            ast::ExprKind::Ident(id) => self.resolve_value_ident(id)?,
            ast::ExprKind::InterpolatedString { parts, .. } => {
                for p in parts {
                    if let ast::InterpolatedStringPart::Expr { expr } = p {
                        self.check_expr(expr)?;
                    }
                }
            }
            ast::ExprKind::Block(b) => self.check_block(b)?,
            ast::ExprKind::UnsafeBlock { body, .. } => self.check_block(body)?,
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
                    // spec §4：pattern binder 仅在该分支 body 内可见。
                    self.push_scope();
                    self.declare_when_pat_binders(&arm.pat)?;
                    if let Some(guard) = &mut arm.guard {
                        self.check_expr(guard)?;
                    }
                    self.check_expr(&mut arm.body)?;
                    self.pop_scope();
                }
            }
            ast::ExprKind::Handle { body, arms, finally } => {
                // handle body 是一个 block；handler arms 的 binder 仅在各自 arm body 内可见。
                self.check_block(body)?;

                for arm in arms {
                    self.push_scope();
                    for b in &arm.op.binders {
                        self.declare_ident_typed(&b.name, b.ty.clone())?;
                    }
                    // spec §5.4：`-> resume` arm 里 `resume(value)` 是一个隐式注入的局部符号。
                    if let ast::HandleArmKind::ImmediateResume { resume_span } = arm.kind {
                        self.declare_ident(&ast::Ident::new(resume_span))?;
                    }
                    // spec §5.4：`, k ->` arm 里 `k` 是显式 continuation binder（作为局部值）。
                    if let ast::HandleArmKind::EscapeContinuation { k_span } = arm.kind {
                        self.declare_ident(&ast::Ident::new(k_span))?;
                    }
                    self.check_expr(&mut arm.body)?;
                    self.pop_scope();
                }

                if let Some(finally) = finally {
                    self.check_block(finally)?;
                }
            }
            ast::ExprKind::Async { body } => {
                // spec §5.7：`async { ... }` 的 body 同样是一个 block。
                self.check_block(body)?;
            }
            ast::ExprKind::Spawn { body } => {
                // spec §5.7：`spawn { ... }` 的 body 同样是一个 block。
                self.check_block(body)?;
            }
            ast::ExprKind::Await { expr, .. } => {
                // spec §5.7：`await expr` 只是一层前缀语法糖，递归检查其操作数即可。
                self.check_expr(expr.as_mut())?;
            }
            ast::ExprKind::Join { expr, .. } => {
                // T0620：`join expr` 只是一层前缀语法糖，递归检查其操作数即可。
                self.check_expr(expr.as_mut())?;
            }
            ast::ExprKind::TypeApply { callee, .. } => {
                // `callee<T>`：type args 本身在该阶段不参与名字解析（TypeRef 解析由其它入口负责）。
                self.check_expr(callee.as_mut())?;
            }
            ast::ExprKind::MemberAccess { receiver, member } => {
                // `TypeName.member` 允许作为 companion member access：
                // receiver 可能不是一个 value ident（例如 class 名称），因此这里对
                // `UnresolvedValue` 做特判，让后续的 `resolve_member_access` 决定是否能解析。
                self.check_member_receiver_expr(receiver.as_mut())?;
                self.resolve_member_access(receiver.as_ref(), member)?;
            }
            ast::ExprKind::SafeMemberAccess {
                receiver, member, ..
            } => {
                self.check_member_receiver_expr(receiver.as_mut())?;
                self.resolve_member_access(receiver.as_ref(), member)?;
            }
            ast::ExprKind::SpliceField { receiver, field } => {
                self.check_expr(receiver.as_mut())?;
                self.check_expr(field.as_mut())?;
            }
            ast::ExprKind::Call { callee, args } => {
                // 调用 callee 的特殊处理：
                //
                // - 对于裸标识符 `callee(args...)`，若该名字无法在 resolve 阶段解析到
                //   局部/顶层符号，我们**不**在此处报 `unresolved_value`，而是留给 typecheck
                //   给出“不可调用/enum variant ctor”等更贴近语义的诊断（T0311/T0426）。
                // - 对于非裸标识符（member access / lambda / 其它表达式），仍按普通表达式递归检查。
                match &mut callee.kind {
                    ast::ExprKind::Ident(id) => match self.resolve_value_ident(id) {
                        Ok(()) => {}
                        Err(ResolveError::UnresolvedValue { .. }) => {}
                        Err(other) => return Err(other),
                    },
                    ast::ExprKind::TypeApply {
                        callee: inner, ..
                    } => match &mut inner.kind {
                        ast::ExprKind::Ident(id) => match self.resolve_value_ident(id) {
                            Ok(()) => {}
                            Err(ResolveError::UnresolvedValue { .. }) => {}
                            Err(other) => return Err(other),
                        },
                        _ => self.check_expr(inner.as_mut())?,
                    },
                    _ => self.check_expr(callee.as_mut())?,
                }
                for a in args.iter_mut() {
                    self.check_expr(a)?;
                }
                self.resolve_call_site(callee.as_mut(), args)?;
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

    fn check_member_receiver_expr(&mut self, receiver: &mut ast::Expr) -> Result<(), ResolveError> {
        match &mut receiver.kind {
            ast::ExprKind::Ident(id) => match self.resolve_value_ident(id) {
                Ok(()) => Ok(()),
                // 允许把 receiver 当作类型名（例如 `C.member`）。
                Err(ResolveError::UnresolvedValue { .. }) => Ok(()),
                Err(other) => Err(other),
            },
            _ => self.check_expr(receiver),
        }
    }

    fn declare_when_pat_binders(&mut self, pat: &ast::WhenPat) -> Result<(), ResolveError> {
        match pat {
            ast::WhenPat::Else { .. }
            | ast::WhenPat::Or { .. }
            | ast::WhenPat::Is { .. }
            | ast::WhenPat::Wildcard { .. }
            | ast::WhenPat::Rest { .. }
            | ast::WhenPat::IntLit { .. }
            | ast::WhenPat::StringLit { .. }
            | ast::WhenPat::BoolLit { .. } => Ok(()),
            ast::WhenPat::Bind { ident } => self.declare_ident(ident),
            ast::WhenPat::Tuple { elements, .. } => {
                for e in elements {
                    self.declare_when_pat_binders(e)?;
                }
                Ok(())
            }
            ast::WhenPat::Variant { args, .. } => {
                for a in args {
                    self.declare_when_pat_binders(a)?;
                }
                Ok(())
            }
        }
    }

    /// 解析调用点（T0319）：收集候选集合 + 调用形状，并写回到 AST。
    ///
    /// 说明：
    /// - resolve 阶段不做 overload 决议，不在“多候选”时报错；
    /// - 若候选集合为空，则保持现状（让 typecheck 给出“不可调用/enum variant ctor”等更贴近语义的诊断）。
    fn resolve_call_site(
        &self,
        callee: &mut ast::Expr,
        args: &[ast::Expr],
    ) -> Result<(), ResolveError> {
        match &mut callee.kind {
            ast::ExprKind::TypeApply { callee, .. } => self.resolve_call_site(callee.as_mut(), args),
            ast::ExprKind::Ident(id) => self.resolve_call_ident_callee(id, args),
            ast::ExprKind::MemberAccess { receiver, member } => {
                self.resolve_call_member_callee(receiver, member, args)
            }
            ast::ExprKind::SafeMemberAccess { receiver, member, .. } => {
                self.resolve_call_member_callee(receiver, member, args)
            }
            _ => Ok(()),
        }
    }

    fn resolve_call_ident_callee(
        &self,
        id: &mut ast::ValueIdent,
        args: &[ast::Expr],
    ) -> Result<(), ResolveError> {
        if id.call.is_some() {
            return Ok(());
        }

        // `this(...)` 的构造调用/委托构造语义留给 T0313/后续 class 语义处理。
        let name = self.source.slice(id.span);
        if name == "this" {
            return Ok(());
        }

        // 局部绑定：不做顶层候选收集（保守放行；例如函数值/闭包调用）。
        if matches!(id.resolved, Some(ast::ResolvedValueRef::Local { .. })) {
            return Ok(());
        }

        let mut candidates: Vec<ast::CallCandidate> = Vec::new();

        for fqn in self.top_level_fun_candidates(name, id.span)? {
            candidates.push(ast::CallCandidate::Fun { fqn });
        }
        for ty_fqn in self.top_level_constructor_candidates(name, id.span)? {
            candidates.push(ast::CallCandidate::Constructor { ty_fqn });
        }

        if candidates.is_empty() {
            return Ok(());
        }

        candidates.sort_by(|a, b| call_candidate_sort_key(a).cmp(&call_candidate_sort_key(b)));
        candidates.dedup();

        // 保持现有 typecheck（T0407）的最小子集可用：
        // - 若调用点只有一个顶层函数候选，则沿用旧写回：把 callee 的 ident 绑定到该 fqn。
        //
        // 当存在多个候选（或存在构造候选）时，resolve 阶段不做最终决议：留给后续 typecheck。
        let resolved_single_fun_fqn = (candidates.len() == 1)
            .then(|| match &candidates[0] {
                ast::CallCandidate::Fun { fqn } => Some(fqn.clone()),
                ast::CallCandidate::Constructor { .. } => None,
            })
            .flatten();

        id.call = Some(ast::ResolvedCall {
            candidates,
            shape: self.call_shape(args),
        });

        if let Some(fqn) = resolved_single_fun_fqn {
            id.resolved = Some(ast::ResolvedValueRef::TopLevel { fqn });
        }

        Ok(())
    }

    fn resolve_call_member_callee(
        &self,
        receiver: &ast::Expr,
        member: &mut ast::MemberIdent,
        args: &[ast::Expr],
    ) -> Result<(), ResolveError> {
        if member.call.is_some() {
            return Ok(());
        }

        let Some(resolved) = &member.resolved else {
            return Ok(());
        };

        let candidates: Vec<ast::CallCandidate> = match resolved {
            ast::ResolvedMemberRef::Fun { fqn } => vec![ast::CallCandidate::Fun { fqn: fqn.clone() }],
            ast::ResolvedMemberRef::ExtensionFun { fqn } => {
                // T0322：跨包 extension 导入与候选收集。
                //
                // 说明：
                // - member access 解析阶段会把 `receiver.member` 绑定为某个 `ExtensionFun { fqn }`，
                //   以保持旧的 typecheck 增量（依赖单一 fqn）可用；
                // - 但在调用点层面，我们需要把“当前作用域内可见的 extension 候选集合”写回，
                //   以便后续 typecheck/infer 做 most-specific/歧义诊断。
                let fqns: Vec<String> = match self.infer_member_receiver_kind(receiver) {
                    Some(MemberReceiverKind::Value { ty_fqn }) => {
                        let name = self.source.slice(member.span);
                        let mut fqns = self.extension_fun_candidates(&ty_fqn, name, member.span)?;
                        if fqns.is_empty() {
                            fqns.push(fqn.clone());
                        }
                        fqns
                    }
                    _ => vec![fqn.clone()],
                };

                fqns.into_iter()
                    .map(|fqn| ast::CallCandidate::Fun { fqn })
                    .collect()
            }
            ast::ResolvedMemberRef::Value { .. } | ast::ResolvedMemberRef::ExtensionValue { .. } => {
                // `p.x()`：此时 callee 是一个 value member，是否可调用取决于其类型（留给 typecheck）。
                return Ok(());
            }
        };

        if candidates.is_empty() {
            return Ok(());
        }

        let mut candidates = candidates;
        candidates.sort_by(|a, b| call_candidate_sort_key(a).cmp(&call_candidate_sort_key(b)));
        candidates.dedup();

        member.call = Some(ast::ResolvedCall {
            candidates,
            shape: self.call_shape(args),
        });

        Ok(())
    }

    fn call_shape(&self, args: &[ast::Expr]) -> ast::CallShape {
        let mut out: Vec<ast::CallArgShape> = Vec::with_capacity(args.len());
        for a in args {
            match &a.kind {
                ast::ExprKind::NamedArg { name, value, .. } => out.push(ast::CallArgShape::Named {
                    name: self.source.slice(name.span).to_string(),
                    name_span: name.span,
                    value_span: value.span,
                }),
                _ => out.push(ast::CallArgShape::Positional { span: a.span }),
            }
        }
        ast::CallShape { args: out }
    }

    fn resolve_value_ident(&mut self, id: &mut ast::ValueIdent) -> Result<(), ResolveError> {
        let name = self.source.slice(id.span);

        // `true/false` 当前阶段仍以 ident token 形式存在，但语义上属于字面量。
        // 因此它们不应参与名字解析，也不应在 resolve 阶段报“未定义符号”。
        //
        // 说明：
        // - 未来 lexer/parser 很可能会把它们升级为专门的 BoolLit token/ExprKind；
        // - 在那之前，这个 special-case 用于保持 fixtures 与 typecheck 增量推进的稳定性。
        if name == "true" || name == "false" {
            return Ok(());
        }

        if name == "this" {
            if let Some(ctx) = self.this_context.last() {
                id.resolved = Some(ast::ResolvedValueRef::Local {
                    name: name.to_string(),
                    decl_span: ctx.decl_span,
                });
                return Ok(());
            }

            return Err(ResolveError::UnresolvedValue {
                name: name.to_string(),
                span: id.span.into(),
            });
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
        self.scopes.iter().rev().find_map(|frame| frame.get(name))
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
            let sym_val = syms.get(SymbolKind::Value);
            if !syms.has_fun() && sym_val.is_none() {
                continue;
            }

            if let Some(_fun) = syms.any_visible_fun(self.use_cone, self.source) {
                // fun overload set：任一 overload 可见即可匹配该 FQN。
                return Ok(Some(fqn));
            } else if syms.has_fun() {
                // 记录“只有不可见候选”的情况，用于在最终无匹配时给出 NotVisible（优先于 UnresolvedValue）。
                if let Some(first) = syms.first_fun() {
                    if not_visible.is_none() {
                        not_visible =
                            Some((fqn.clone(), first.symbol.visibility, first.symbol.span));
                    }
                }
            }

            if let Some(sym) = sym_val {
                if is_symbol_visible_from(self.use_cone, self.source, sym) {
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

    fn top_level_fun_candidates(
        &self,
        name: &str,
        use_span: Span,
    ) -> Result<Vec<String>, ResolveError> {
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

            if let Some(_fun) = syms.any_visible_fun(self.use_cone, self.source) {
                matches.push(fqn);
            } else if syms.has_fun() && not_visible.is_none() {
                if let Some(first) = syms.first_fun() {
                    not_visible = Some((fqn.clone(), first.symbol.visibility, first.symbol.span));
                }
            }
        }

        if matches.is_empty() {
            if let Some((name, visibility, def_span)) = not_visible {
                return Err(ResolveError::NotVisible {
                    name,
                    visibility,
                    use_span: use_span.into(),
                    def_span: def_span.into(),
                });
            }
            return Ok(Vec::new());
        }

        matches.sort();
        matches.dedup();
        Ok(matches)
    }

    fn top_level_constructor_candidates(
        &self,
        name: &str,
        use_span: Span,
    ) -> Result<Vec<String>, ResolveError> {
        // 1) 先按 type 命名空间解析“同名类型”候选（同包 / import / star import），保留全部可见项。
        let mut candidates: Vec<String> = Vec::new();

        if !self.pkg_prefix.is_empty() {
            candidates.push(format!("{}.{}", self.pkg_prefix, name));
        }
        candidates.push(name.to_string());

        if let Some(fqns) = self.imports.ty.explicit.get(name) {
            candidates.extend(fqns.iter().cloned());
        }
        for prefix in &self.imports.star {
            candidates.push(format!("{prefix}.{name}"));
        }

        candidates.sort();
        candidates.dedup();

        let mut matches: Vec<String> = Vec::new();
        let mut not_visible: Option<(String, Visibility, Span)> = None;

        for ty_fqn in candidates {
            let Some(syms) = self.index.by_fqn.get(&ty_fqn) else {
                continue;
            };
            let Some(sym) = syms.get(SymbolKind::Type) else {
                continue;
            };
            if !is_symbol_visible_from(self.use_cone, self.source, sym) {
                if not_visible.is_none() {
                    not_visible = Some((ty_fqn.clone(), sym.visibility, sym.span));
                }
                continue;
            }

            let Some(ctors) = self.index.constructors.get(&ty_fqn) else {
                // 当前阶段（T0319）只把“显式收集到 constructors overload set” 的类型纳入构造候选；
                // 隐式默认构造器规则留给后续 typecheck/语义任务补齐。
                continue;
            };

            if ctors
                .iter()
                .any(|c| is_ctor_visible_from(self.use_cone, self.source, c))
            {
                matches.push(ty_fqn);
            } else if not_visible.is_none() {
                if let Some(first) = ctors.first() {
                    not_visible = Some((ty_fqn.clone(), first.visibility, first.span));
                }
            }
        }

        if matches.is_empty() {
            if let Some((name, visibility, def_span)) = not_visible {
                return Err(ResolveError::NotVisible {
                    name,
                    visibility,
                    use_span: use_span.into(),
                    def_span: def_span.into(),
                });
            }
            return Ok(Vec::new());
        }

        matches.sort();
        matches.dedup();
        Ok(matches)
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

        let Some(receiver_kind) = self.infer_member_receiver_kind(receiver) else {
            // 当前阶段（T0310/T0317）只处理“静态可确定”的 receiver：
            // - value receiver：可确定类型（局部类型注解 / this / struct literal / object 单例）
            // - type receiver：可解析为一个类型 FQN（用于 companion member access）
            //
            // 无法确定时保持保守：不做解析也不报错（避免对泛型/推断场景产生误报）。
            return Ok(());
        };

        match receiver_kind {
            MemberReceiverKind::Value {
                ty_fqn: receiver_ty_fqn,
            } => {
                return self.resolve_member_access_on_value_receiver(
                    &receiver_ty_fqn,
                    receiver,
                    member,
                );
            }
            MemberReceiverKind::Type {
                ty_fqn: receiver_ty_fqn,
            } => {
                return self.resolve_member_access_on_type_receiver(
                    &receiver_ty_fqn,
                    receiver,
                    member,
                );
            }
        }
    }

    fn resolve_member_access_on_value_receiver(
        &self,
        receiver_ty_fqn: &str,
        receiver: &ast::Expr,
        member: &mut ast::MemberIdent,
    ) -> Result<(), ResolveError> {
        let member_name = self.source.slice(member.span);
        let member_fqn = format!("{receiver_ty_fqn}.{member_name}");

        if let Some(syms) = self.index.by_fqn.get(&member_fqn) {
            // 与调用解析（T0311）解耦：这里只做存在性与最小“绑定写回”。
            // 目前策略：优先解析为方法（fun namespace），否则退化到字段/属性（value namespace）。
            let mut not_visible: Option<(String, Visibility, Span)> = None;

            if let Some(_fun) = syms.any_visible_fun(self.use_cone, self.source) {
                member.resolved = Some(ast::ResolvedMemberRef::Fun { fqn: member_fqn });
                return Ok(());
            } else if syms.has_fun() {
                if let Some(first) = syms.first_fun() {
                    not_visible = Some((
                        member_fqn.clone(),
                        first.symbol.visibility,
                        first.symbol.span,
                    ));
                }
            }

            if let Some(sym) = syms.get(SymbolKind::Value) {
                if is_symbol_visible_from(self.use_cone, self.source, sym) {
                    // T0316：class 初始化阶段禁止访问“后置属性”（前向引用）。
                    if let Some(init_ctx) = self.init_value_members.last() {
                        if init_ctx.this_ty_fqn == receiver_ty_fqn
                            && self.is_receiver_this(receiver)
                            && !init_ctx.visible_value_members.contains(member_name)
                        {
                            return Err(ResolveError::ForwardReference {
                                name: member_fqn.clone(),
                                use_span: member.span.into(),
                                def_span: sym.span.into(),
                            });
                        }
                    }
                    member.resolved = Some(ast::ResolvedMemberRef::Value { fqn: member_fqn });
                    return Ok(());
                }
                if not_visible.is_none() {
                    not_visible = Some((member_fqn.clone(), sym.visibility, sym.span));
                }
            }

            // 同名 member 与 extension 并存时：member 优先（即便 member 不可见，也不回退到 extension）。
            if let Some((name, visibility, def_span)) = not_visible {
                return Err(ResolveError::NotVisible {
                    name,
                    visibility,
                    use_span: member.span.into(),
                    def_span: def_span.into(),
                });
            }
        }

        // T0312：若不存在同名 member，则尝试在同包可见的 extension fun 中按 receiver 类型匹配。
        let ext_candidates =
            self.extension_fun_candidates(receiver_ty_fqn, member_name, member.span)?;
        if let Some(fqn) = ext_candidates.first().cloned() {
            member.resolved = Some(ast::ResolvedMemberRef::ExtensionFun { fqn });
            return Ok(());
        }

        // spec §8.4：`String.trimIndent()` 是内建的字符串 API（const fun）。
        //
        // 说明：
        // - 早期阶段我们尚未把 `String` 的成员函数完整建模为可解析的符号（TODO T1216/T0827）；
        // - resolver 在这里对“已知 receiver 类型为 `scoop.core.String`”的 case 做保守放行，
        //   让后续 typecheck/codegen 走 intrinsic 路径（并在运行期 fallback）。
        if receiver_ty_fqn == "scoop.core.String" && member_name == "trimIndent" {
            return Ok(());
        }

        // spec §5.5：`Continuation.resume` 是内建操作。
        //
        // 说明：
        // - 现阶段 resolver 主要负责“把可静态确定的成员名解析到一个 FQN”，用于后续 typecheck；
        // - 但 `Continuation.resume` 在语义上并不是普通成员/扩展函数（更像语言关键操作），并且我们
        //   还未在 sysroot 中暴露其完整声明；
        // - 为了避免把该语义强行建模为一个普通符号，这里对该内建操作做保守放行：不报 unresolved，
        //   留给 typecheck 决定其类型规则与 required effects 传播（TODO T0611）。
        if receiver_ty_fqn == "scoop.core.Continuation" && member_name == "resume" {
            return Ok(());
        }

        Err(ResolveError::UnresolvedMember {
            name: member_fqn,
            span: member.span.into(),
        })
    }

    /// 查找“在当前文件作用域内可见”的 extension fun 候选集合（T0322）。
    ///
    /// 规则（当前阶段最小落地）：
    /// - 同包（同 cone）声明的 extension 无需 import 可见；
    /// - 显式 import（含 alias）与 star import（`import pkg.*`）可引入跨包/跨 cone 的 extension；
    /// - 可见性过滤沿用统一规则：public 跨 cone 可见，internal 仅 cone 内可见，private 仅文件内可见；
    /// - receiver 类型匹配规则保持与 T0312 一致：
    ///   - receiver 完全相等，或
    ///   - extension receiver 为 `scoop.core.Any`（通配）。
    fn extension_fun_candidates(
        &self,
        receiver_ty_fqn: &str,
        member_name: &str,
        use_span: Span,
    ) -> Result<Vec<String>, ResolveError> {
        let mut candidates: Vec<String> = Vec::new();
        let mut not_visible: Option<(String, Visibility, Span)> = None;

        // 1) 同包（同 cone）隐式可见。
        for ext in &self.index.extension_funs {
            if ext.decl_cone != self.use_cone {
                continue;
            }
            if ext.pkg_prefix != self.pkg_prefix {
                continue;
            }
            if ext.name != member_name {
                continue;
            }

            let receiver_matches = match ext.receiver_ty_fqn.as_deref() {
                Some(ext_receiver) => {
                    ext_receiver == receiver_ty_fqn || ext_receiver == "scoop.core.Any"
                }
                // `fun <T> T.ext()`：receiver 是 type param（通配）。
                None => ext.receiver_is_type_param,
            };
            if !receiver_matches {
                continue;
            }

            let Some(syms) = self.index.by_fqn.get(&ext.fqn) else {
                continue;
            };
            if let Some(_fun) = syms.any_visible_fun(self.use_cone, self.source) {
                candidates.push(ext.fqn.clone());
                continue;
            }

            if not_visible.is_none() {
                if let Some(first) = syms.first_fun() {
                    not_visible = Some((ext.fqn.clone(), first.symbol.visibility, first.symbol.span));
                }
            }
        }

        // 2) star import：`import pkg.*` 引入该 package 下的 extension。
        for prefix in &self.imports.star {
            for ext in &self.index.extension_funs {
                if ext.pkg_prefix != *prefix {
                    continue;
                }
                if ext.name != member_name {
                    continue;
                }

                let receiver_matches = match ext.receiver_ty_fqn.as_deref() {
                    Some(ext_receiver) => {
                        ext_receiver == receiver_ty_fqn || ext_receiver == "scoop.core.Any"
                    }
                    // `fun <T> T.ext()`：receiver 是 type param（通配）。
                    None => ext.receiver_is_type_param,
                };
                if !receiver_matches {
                    continue;
                }

                let Some(syms) = self.index.by_fqn.get(&ext.fqn) else {
                    continue;
                };
                if let Some(_fun) = syms.any_visible_fun(self.use_cone, self.source) {
                    candidates.push(ext.fqn.clone());
                    continue;
                }

                if not_visible.is_none() {
                    if let Some(first) = syms.first_fun() {
                        not_visible =
                            Some((ext.fqn.clone(), first.symbol.visibility, first.symbol.span));
                    }
                }
            }
        }

        // 3) 显式 import（含 alias）：通过 local 名字 → fqn 查找 extension。
        if let Some(imported) = self.imports.value.explicit.get(member_name) {
            for imported_fqn in imported {
                for ext in self
                    .index
                    .extension_funs
                    .iter()
                    .filter(|e| e.fqn == *imported_fqn)
                {
                    let receiver_matches = match ext.receiver_ty_fqn.as_deref() {
                        Some(ext_receiver) => {
                            ext_receiver == receiver_ty_fqn || ext_receiver == "scoop.core.Any"
                        }
                        // `fun <T> T.ext()`：receiver 是 type param（通配）。
                        None => ext.receiver_is_type_param,
                    };
                    if !receiver_matches {
                        continue;
                    }

                    let Some(syms) = self.index.by_fqn.get(&ext.fqn) else {
                        continue;
                    };
                    if let Some(_fun) = syms.any_visible_fun(self.use_cone, self.source) {
                        candidates.push(ext.fqn.clone());
                        continue;
                    }

                    if not_visible.is_none() {
                        if let Some(first) = syms.first_fun() {
                            not_visible = Some((
                                ext.fqn.clone(),
                                first.symbol.visibility,
                                first.symbol.span,
                            ));
                        }
                    }
                }
            }
        }

        candidates.sort();
        candidates.dedup();

        if candidates.is_empty() {
            if let Some((name, visibility, def_span)) = not_visible {
                return Err(ResolveError::NotVisible {
                    name,
                    visibility,
                    use_span: use_span.into(),
                    def_span: def_span.into(),
                });
            }
        }

        Ok(candidates)
    }

    fn resolve_member_access_on_type_receiver(
        &self,
        receiver_ty_fqn: &str,
        receiver: &ast::Expr,
        member: &mut ast::MemberIdent,
    ) -> Result<(), ResolveError> {
        let member_name = self.source.slice(member.span);
        let direct_fqn = format!("{receiver_ty_fqn}.{member_name}");

        // 1) `TypeName.NestedObject`：若该成员本身是一个 object（同时存在 type+value 符号），直接解析到该 object 值。
        if let Some(syms) = self.index.by_fqn.get(&direct_fqn) {
            if syms.get(SymbolKind::Type).is_some() {
                let mut not_visible: Option<(String, Visibility, Span)> = None;

                if let Some(sym) = syms.get(SymbolKind::Value) {
                    if is_symbol_visible_from(self.use_cone, self.source, sym) {
                        member.resolved = Some(ast::ResolvedMemberRef::Value { fqn: direct_fqn });
                        return Ok(());
                    }
                    not_visible = Some((direct_fqn.clone(), sym.visibility, sym.span));
                }

                if let Some((name, visibility, def_span)) = not_visible {
                    return Err(ResolveError::NotVisible {
                        name,
                        visibility,
                        use_span: member.span.into(),
                        def_span: def_span.into(),
                    });
                }
            }
        }

        // 1.5) `EffectName.op`：effect operation 的限定引用（spec §5.2）。
        //
        // 说明：
        // - effect operation 语义上不经 companion object 分发；
        // - parser 会把 effect body 内的 `fun` 标记为 `FunDeclKind::EffectOp`（T0601），
        //   resolver 在此处通过索引侧记录的 `FunSig.kind` 做最小区分。
        if let Some(syms) = self.index.by_fqn.get(&direct_fqn) {
            let mut not_visible: Option<(String, Visibility, Span)> = None;

            for o in &syms.fun {
                if o.sig.kind != ast::FunDeclKind::EffectOp {
                    continue;
                }
                if is_symbol_visible_from(self.use_cone, self.source, &o.symbol) {
                    member.resolved = Some(ast::ResolvedMemberRef::Fun { fqn: direct_fqn });
                    return Ok(());
                }
                if not_visible.is_none() {
                    not_visible = Some((
                        direct_fqn.clone(),
                        o.symbol.visibility,
                        o.symbol.span,
                    ));
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
        }

        // 2) `TypeName.member`：尝试经 companion object 解析（spec Appendix B.9 / TODO T0317）。
        let Some(companions) = self.index.companion_objects.get(receiver_ty_fqn) else {
            return Err(ResolveError::MissingCompanionObject {
                ty: receiver_ty_fqn.to_string(),
                span: receiver.span.into(),
            });
        };

        // 规则（最小落地）：
        // - 优先在 companion 的 fun namespace 查找；若无任何 fun 候选，再查 value namespace。
        // - 若多个 companion 同时定义同名成员，则报错（避免解析结果依赖声明顺序）。
        let mut fun_matches: Vec<(String, Visibility, Span)> = Vec::new();
        let mut value_matches: Vec<(String, Visibility, Span)> = Vec::new();
        let mut not_visible: Option<(String, Visibility, Span)> = None;

        for c in companions {
            let fqn = format!("{c}.{member_name}");
            let Some(syms) = self.index.by_fqn.get(&fqn) else {
                continue;
            };

            if let Some(fun) = syms.any_visible_fun(self.use_cone, self.source) {
                fun_matches.push((fqn.clone(), fun.symbol.visibility, fun.symbol.span));
            } else if syms.has_fun() && not_visible.is_none() {
                if let Some(first) = syms.first_fun() {
                    not_visible = Some((fqn.clone(), first.symbol.visibility, first.symbol.span));
                }
            }

            if let Some(sym) = syms.get(SymbolKind::Value) {
                if is_symbol_visible_from(self.use_cone, self.source, sym) {
                    value_matches.push((fqn.clone(), sym.visibility, sym.span));
                } else if not_visible.is_none() {
                    not_visible = Some((fqn.clone(), sym.visibility, sym.span));
                }
            }
        }

        match fun_matches.len() {
            1 => {
                member.resolved = Some(ast::ResolvedMemberRef::Fun {
                    fqn: fun_matches[0].0.clone(),
                });
                return Ok(());
            }
            n if n > 1 => {
                // 复用 `unresolved_member` 以保持错误码数量最小；
                // 更细粒度的歧义诊断留给后续（如 T04 typecheck）。
                return Err(ResolveError::UnresolvedMember {
                    name: format!("{receiver_ty_fqn}.{member_name}"),
                    span: member.span.into(),
                });
            }
            _ => {}
        }

        match value_matches.len() {
            1 => {
                member.resolved = Some(ast::ResolvedMemberRef::Value {
                    fqn: value_matches[0].0.clone(),
                });
                return Ok(());
            }
            n if n > 1 => {
                return Err(ResolveError::UnresolvedMember {
                    name: format!("{receiver_ty_fqn}.{member_name}"),
                    span: member.span.into(),
                });
            }
            _ => {}
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
            name: format!("{receiver_ty_fqn}.{member_name}"),
            span: member.span.into(),
        })
    }

    fn is_receiver_this(&self, receiver: &ast::Expr) -> bool {
        matches!(
            &receiver.kind,
            ast::ExprKind::Ident(id) if self.source.slice(id.span) == "this"
        )
    }

    fn infer_member_receiver_kind(&self, expr: &ast::Expr) -> Option<MemberReceiverKind> {
        match &expr.kind {
            ast::ExprKind::Ident(id) => {
                let name = self.source.slice(id.span);

                if name == "this" {
                    return self
                        .this_context
                        .last()
                        .and_then(|ctx| ctx.ty_fqn.clone())
                        .map(|ty_fqn| MemberReceiverKind::Value { ty_fqn });
                }

                if let Some(binding) = self.local_binding(name) {
                    return self
                        .type_ref_to_fqn(binding.ty.as_ref()?)
                        .map(|ty_fqn| MemberReceiverKind::Value { ty_fqn });
                }

                // 顶层 object / enum（值名与类型名同名）：当 receiver 是该顶层值时，可把其类型视为同名类型。
                if let Some(ast::ResolvedValueRef::TopLevel { fqn }) = &id.resolved {
                    if self.index.object_types.contains(fqn) {
                        return Some(MemberReceiverKind::Value {
                            ty_fqn: fqn.clone(),
                        });
                    }
                    // enum 也会在 value namespace 中注入一个同名值（见 `Index::add_type_decl`），
                    // 以支持 `EnumName.Variant` 的限定名引用；此时同样可把其“接收者类型”视为同名类型。
                    if self
                        .index
                        .by_fqn
                        .get(fqn)
                        .and_then(|syms| syms.get(SymbolKind::Type))
                        .is_some()
                    {
                        return Some(MemberReceiverKind::Value {
                            ty_fqn: fqn.clone(),
                        });
                    }
                    return None;
                }

                // `TypeName.member`：若 receiver 解析不到 value，但能解析到 type，则按“类型接收者”处理。
                //
                // 注意：这里刻意不把 type receiver 写回到 `ValueIdent.resolved`，避免把类型名误当成值名
                //（例如 `val x = C`）在其它语境里被放行。
                if id.resolved.is_none() {
                    return self
                        .type_name_to_fqn(name)
                        .map(|ty_fqn| MemberReceiverKind::Type { ty_fqn });
                }

                None
            }
            ast::ExprKind::StructLit { ty, .. } => self
                .type_path_to_fqn(ty)
                .map(|ty_fqn| MemberReceiverKind::Value { ty_fqn }),
            ast::ExprKind::MemberAccess { member, .. }
            | ast::ExprKind::SafeMemberAccess { member, .. } => {
                // 链式成员访问：若 `a.b` 解析到一个 object 值（同时存在 type symbol），则它也可作为后续 receiver。
                let Some(ast::ResolvedMemberRef::Value { fqn }) = &member.resolved else {
                    return None;
                };
                if self.index.object_types.contains(fqn) {
                    return Some(MemberReceiverKind::Value {
                        ty_fqn: fqn.clone(),
                    });
                }
                None
            }
            _ => None,
        }
    }

    fn type_name_to_fqn(&self, name: &str) -> Option<String> {
        let mut candidates: Vec<String> = Vec::new();

        if !self.pkg_prefix.is_empty() {
            candidates.push(format!("{}.{}", self.pkg_prefix, name));
        }
        // 允许用户引用 root package 下的符号（无 package 声明时也可匹配）。
        candidates.push(name.to_string());

        if let Some(fqns) = self.imports.ty.explicit.get(name) {
            candidates.extend(fqns.iter().cloned());
        }
        for prefix in &self.imports.star {
            candidates.push(format!("{prefix}.{name}"));
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
            if is_symbol_visible_from(self.use_cone, self.source, sym) {
                return Some(fqn);
            }
        }

        None
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
            if is_symbol_visible_from(self.use_cone, self.source, sym) {
                return Some(fqn);
            }
        }

        None
    }

    fn with_scope<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, ResolveError>,
    ) -> Result<T, ResolveError> {
        self.push_scope();
        let result = f(self);
        self.pop_scope();
        result
    }

    fn with_this_context<T>(
        &mut self,
        ctx: ThisContext,
        f: impl FnOnce(&mut Self) -> Result<T, ResolveError>,
    ) -> Result<T, ResolveError> {
        self.this_context.push(ctx);
        let result = f(self);
        let _ = self.this_context.pop();
        result
    }

    fn with_init_value_members_context<T>(
        &mut self,
        this_ty_fqn: Option<&str>,
        visible_value_members: &HashSet<String>,
        f: impl FnOnce(&mut Self) -> Result<T, ResolveError>,
    ) -> Result<T, ResolveError> {
        let Some(this_ty_fqn) = this_ty_fqn else {
            return f(self);
        };

        self.init_value_members.push(InitValueMembersContext {
            this_ty_fqn: this_ty_fqn.to_string(),
            visible_value_members: visible_value_members.clone(),
        });
        let result = f(self);
        let _ = self.init_value_members.pop();
        result
    }

    fn with_ctor_params_scope<T>(
        &mut self,
        ctor_params: &[ast::Param],
        f: impl FnOnce(&mut Self) -> Result<T, ResolveError>,
    ) -> Result<T, ResolveError> {
        self.with_scope(|this| {
            for p in ctor_params {
                this.declare_ident_typed(&p.name, p.ty.clone())?;
            }
            f(this)
        })
    }
}

fn call_candidate_sort_key(c: &ast::CallCandidate) -> (u8, &str) {
    match c {
        ast::CallCandidate::Fun { fqn } => (0, fqn.as_str()),
        ast::CallCandidate::Constructor { ty_fqn } => (1, ty_fqn.as_str()),
    }
}

fn is_ctor_visible_from(
    use_cone: ConeId,
    source: &SourceFile,
    ctor: &super::ConstructorOverload,
) -> bool {
    match ctor.visibility {
        Visibility::Public => true,
        Visibility::Internal => ctor.decl_cone == use_cone,
        Visibility::Private => ctor.decl_file == source.path(),
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
        ast::PatternKind::Wildcard | ast::PatternKind::Rest | ast::PatternKind::Missing => {}
        ast::PatternKind::Bind(id) => out.push(*id),
        ast::PatternKind::Variant { args, .. } => {
            for arg in args {
                collect_bound_idents_in_pattern(arg, out);
            }
        }
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
        assert_eq!(
            member.call,
            Some(ast::ResolvedCall {
                candidates: vec![ast::CallCandidate::Fun {
                    fqn: "a.Point.m".to_string()
                }],
                shape: ast::CallShape { args: Vec::new() },
            })
        );
    }

    #[test]
    fn extension_fun_is_resolved_when_no_member() {
        let src = SourceFile::new_virtual(
            "<mem>",
            "package a\nstruct Point {}\nfun Point.ext() {}\nfun use(p: Point) { p.ext() }",
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

        let Some(ast::Stmt {
            kind: ast::StmtKind::Expr(call),
            ..
        }) = block.stmts.first()
        else {
            panic!("expected first stmt to be an expr");
        };

        let ast::ExprKind::Call { callee, .. } = &call.kind else {
            panic!("expected call expr");
        };

        let ast::ExprKind::MemberAccess { member, .. } = &callee.kind else {
            panic!("expected member access callee");
        };

        assert!(matches!(
            member.resolved,
            Some(ast::ResolvedMemberRef::ExtensionFun { ref fqn }) if fqn == "a.ext"
        ));
        assert_eq!(
            member.call,
            Some(ast::ResolvedCall {
                candidates: vec![ast::CallCandidate::Fun {
                    fqn: "a.ext".to_string()
                }],
                shape: ast::CallShape { args: Vec::new() },
            })
        );
    }

    #[test]
    fn member_still_wins_over_extension_with_same_name() {
        let src = SourceFile::new_virtual(
            "<mem>",
            "package a\nstruct Point { fun ext() {} }\nfun Point.ext() {}\nfun use(p: Point) { p.ext() }",
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

        let Some(ast::Stmt {
            kind: ast::StmtKind::Expr(call),
            ..
        }) = block.stmts.first()
        else {
            panic!("expected first stmt to be an expr");
        };

        let ast::ExprKind::Call { callee, .. } = &call.kind else {
            panic!("expected call expr");
        };

        let ast::ExprKind::MemberAccess { member, .. } = &callee.kind else {
            panic!("expected member access callee");
        };

        assert!(matches!(
            member.resolved,
            Some(ast::ResolvedMemberRef::Fun { ref fqn }) if fqn == "a.Point.ext"
        ));
        assert_eq!(
            member.call,
            Some(ast::ResolvedCall {
                candidates: vec![ast::CallCandidate::Fun {
                    fqn: "a.Point.ext".to_string()
                }],
                shape: ast::CallShape { args: Vec::new() },
            })
        );
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

        let Some(ast::Stmt {
            kind: ast::StmtKind::Expr(call),
            ..
        }) = block.stmts.first()
        else {
            panic!("expected first stmt to be an expr");
        };

        let ast::ExprKind::Call { callee, .. } = &call.kind else {
            panic!("expected call expr");
        };

        let ast::ExprKind::Ident(id) = &callee.kind else {
            panic!("expected ident callee");
        };

        assert_eq!(
            id.call,
            Some(ast::ResolvedCall {
                candidates: vec![ast::CallCandidate::Fun {
                    fqn: "a.top".to_string()
                }],
                shape: ast::CallShape { args: Vec::new() },
            })
        );
        assert_eq!(
            id.resolved,
            Some(ast::ResolvedValueRef::TopLevel {
                fqn: "a.top".to_string()
            })
        );
    }

    #[test]
    fn ambiguous_call_is_kept_as_candidates() {
        let s1 = SourceFile::new_virtual("<p1>", "package p1\nfun foo() {}");
        let s2 = SourceFile::new_virtual("<p2>", "package p2\nfun foo() {}");
        let s3 = SourceFile::new_virtual(
            "<use>",
            "package use\nimport p1.foo\nimport p2.foo\nfun use() { foo() }",
        );

        let f1 = parse_file(&s1).unwrap();
        let f2 = parse_file(&s2).unwrap();
        let mut f3 = parse_file(&s3).unwrap();

        let index = Index::build(&[(&s1, &f1), (&s2, &f2), (&s3, &f3)]).unwrap();
        super::super::check_file_bindings(&s3, &mut f3, &index).unwrap();

        let fun_use = f3
            .items
            .iter()
            .find_map(|it| match it {
                ast::Item::Fun(f) if s3.slice(f.name.span) == "use" => Some(f),
                _ => None,
            })
            .expect("missing fun use");

        let ast::FunBody::Block(block) = &fun_use.body else {
            panic!("expected block body");
        };

        let Some(ast::Stmt {
            kind: ast::StmtKind::Expr(call),
            ..
        }) = block.stmts.first()
        else {
            panic!("expected first stmt to be an expr");
        };

        let ast::ExprKind::Call { callee, .. } = &call.kind else {
            panic!("expected call expr");
        };

        let ast::ExprKind::Ident(id) = &callee.kind else {
            panic!("expected ident callee");
        };

        assert_eq!(
            id.call,
            Some(ast::ResolvedCall {
                candidates: vec![
                    ast::CallCandidate::Fun {
                        fqn: "p1.foo".to_string()
                    },
                    ast::CallCandidate::Fun {
                        fqn: "p2.foo".to_string()
                    },
                ],
                shape: ast::CallShape { args: Vec::new() },
            })
        );
    }

    #[test]
    fn constructor_call_is_collected_as_candidates() {
        let s1 = SourceFile::new_virtual("<p1>", "package p1\nstruct Any {}\nclass C(x: Any) {}");
        let s2 = SourceFile::new_virtual("<p2>", "package p2\nstruct Any {}\nclass C(x: Any) {}");
        let s3 = SourceFile::new_virtual(
            "<use>",
            "package use\nimport p1.C\nimport p2.C\nfun use() { C(1) }",
        );

        let f1 = parse_file(&s1).unwrap();
        let f2 = parse_file(&s2).unwrap();
        let mut f3 = parse_file(&s3).unwrap();

        let index = Index::build(&[(&s1, &f1), (&s2, &f2), (&s3, &f3)]).unwrap();
        super::super::check_file_bindings(&s3, &mut f3, &index).unwrap();

        let fun_use = f3
            .items
            .iter()
            .find_map(|it| match it {
                ast::Item::Fun(f) if s3.slice(f.name.span) == "use" => Some(f),
                _ => None,
            })
            .expect("missing fun use");

        let ast::FunBody::Block(block) = &fun_use.body else {
            panic!("expected block body");
        };

        let Some(ast::Stmt {
            kind: ast::StmtKind::Expr(call),
            ..
        }) = block.stmts.first()
        else {
            panic!("expected first stmt to be an expr");
        };

        let ast::ExprKind::Call { callee, args } = &call.kind else {
            panic!("expected call expr");
        };

        let ast::ExprKind::Ident(id) = &callee.kind else {
            panic!("expected ident callee");
        };

        assert_eq!(args.len(), 1);
        assert_eq!(
            id.call,
            Some(ast::ResolvedCall {
                candidates: vec![
                    ast::CallCandidate::Constructor {
                        ty_fqn: "p1.C".to_string()
                    },
                    ast::CallCandidate::Constructor {
                        ty_fqn: "p2.C".to_string()
                    },
                ],
                shape: ast::CallShape {
                    args: vec![ast::CallArgShape::Positional { span: args[0].span }],
                },
            })
        );
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

    #[test]
    fn forward_reference_in_init_block_is_error() {
        let src = SourceFile::new_virtual(
            "<mem>",
            "package a\nstruct Any {}\nclass Box(x: Any) {\ninit { this.y }\nval y: Any = x\n}",
        );
        let mut file = parse_file(&src).unwrap();
        let index = build_index(&[(&src, &file)]);

        let err = super::super::check_file_bindings(&src, &mut file, &index).unwrap_err();
        assert!(matches!(err, ResolveError::ForwardReference { .. }));
    }

    #[test]
    fn secondary_ctor_body_sees_this_and_params() {
        let src = SourceFile::new_virtual(
            "<mem>",
            "package a\nstruct Any {}\nclass Pair(a: Any) { constructor(b: Any) { this\na\nb } }",
        );
        let mut file = parse_file(&src).unwrap();
        let index = build_index(&[(&src, &file)]);

        super::super::check_file_bindings(&src, &mut file, &index).unwrap();
    }
}
