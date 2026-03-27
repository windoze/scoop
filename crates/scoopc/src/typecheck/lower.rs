//! `TypeRef` → `Type` lowering（T0403）。
//!
//! 当前阶段的目标：
//! - 把 parser 产出的 AST `TypeRef` 转换为编译器内部类型表示（`ty::TypeId`）
//! - 在 lowering 过程中做最小语义校验：类型存在性（应由 resolve 保证）与泛型 arity 检查
//! - 覆盖 `Path` / `Tuple` / `Nullable` / `Function`，其它类型语法在后续任务逐步补齐

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::path::PathBuf;

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::monomorph::{MonomorphKey, MonomorphSymbol};
use crate::resolve::{ImportTable, Index, Visibility};
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{
    BuiltinTypes, EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeParamType, TypeStore,
    ValueTypeKind,
};

use super::assignable::is_type_assignable;
use super::{TypeEnv, TypeSymbol, TypeSymbolKind};

#[derive(Debug, Error, Diagnostic)]
pub enum TypeLowerError {
    #[error("未解析的类型：{name}")]
    #[diagnostic(code(scoop::typecheck::unresolved_type))]
    UnresolvedType {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("类型参数数量不匹配：{name} 期望 {expected} 个，但提供了 {found} 个")]
    #[diagnostic(code(scoop::typecheck::type_arity_mismatch))]
    TypeArityMismatch {
        name: String,
        expected: usize,
        found: usize,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("类型 {name} 不支持 use-site effect row 实参（`eff ...`）")]
    #[diagnostic(code(scoop::typecheck::use_site_eff_arg_not_allowed))]
    UseSiteEffectRowArgNotAllowed {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("暂不支持的类型语法：{kind}")]
    #[diagnostic(code(scoop::typecheck::unsupported_type_ref))]
    UnsupportedTypeRef {
        kind: &'static str,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("类型环境缺少符号：{fqn}")]
    #[diagnostic(code(scoop::typecheck::missing_type_symbol_in_env))]
    MissingTypeSymbolInEnv {
        fqn: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("effect row 项必须是 effect 类型：{item}（得到 {found}）")]
    #[diagnostic(code(scoop::typecheck::effect_row_item_not_effect))]
    EffectRowItemNotEffect {
        item: String,
        found: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    /// 闭合 effect row（`E!`）不允许包含 row 变量（例如函数级 `<eff E>` 的 `E`）。
    ///
    /// 说明：当前阶段闭合 row 的主要用途是 program boundary（例如 `Pure!`）。
    /// 把 row 变量放进闭合 row 会让“闭合”的语义在调用点被重新打开，导致边界规则难以保持直观。
    #[error("闭合 effect row 不允许引用 row 变量：{name}")]
    #[diagnostic(code(scoop::typecheck::closed_effect_row_contains_row_var))]
    ClosedEffectRowContainsRowVar {
        name: String,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    /// 循环 typealias（直接或间接）：例如 `typealias A = B; typealias B = A`。
    #[error("循环的类型别名：{cycle}")]
    #[diagnostic(code(scoop::typecheck::cyclic_type_alias))]
    CyclicTypeAlias {
        cycle: String,
        #[label("别名声明在这里")]
        first: miette::SourceSpan,
        #[label("并且通过其它别名又回到这里")]
        second: miette::SourceSpan,
    },

    /// 声明处变型（`in`/`out`）位置规则违规（spec §3.2 / Appendix B.4）。
    #[error("变型位置不合法：类型参数 {param} 声明为 {declared}，但在 {position} 位置使用")]
    #[diagnostic(code(scoop::typecheck::variance_position_violation))]
    VariancePositionViolation {
        param: String,
        declared: &'static str,
        position: &'static str,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    /// `where` 子句约束不满足（T0458）。
    #[error("泛型约束不满足：{type_fqn} 的类型实参 {arg} 不满足 {param} : {bound}")]
    #[diagnostic(code(scoop::typecheck::where_constraint_not_satisfied))]
    WhereConstraintNotSatisfied {
        type_fqn: String,
        param: String,
        arg: String,
        bound: String,
        #[label("这里的类型实参不满足 where 约束")]
        span: miette::SourceSpan,
    },
}

/// 对一个文件内出现的所有 “type position” 的 `TypeRef` 执行 lowering 并做最小校验。
///
/// 说明：
/// - 该函数是早期 typecheck phase 的一块可独立回归的最小能力（fixtures 会直接调用）；
/// - 当前只走声明头（fun/val/type/typealias）的类型引用，不进入函数体的表达式类型检查。
pub fn check_file_type_refs(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Result<(), TypeLowerError> {
    let mut ctx = TypeLowering::new(source, file, index, imports, env, types, builtins);

    for item in &file.items {
        match item {
            ast::Item::TypeAlias(ta) => {
                let _ = ctx.lower_type_ref(&ta.ty)?;
            }
            ast::Item::Fun(fun) => {
                ctx.push_type_params(&fun.type_params);
                let eff_binding = if let Some(eff_param) = &fun.eff_param {
                    let name = source.slice(eff_param.name.span).to_string();
                    let default = match eff_param.default.as_ref() {
                        Some(expr) => ctx.lower_effect_row_expr(Some(expr))?,
                        None => EffectRow::pure(),
                    };
                    ctx.push_effect_row_param_binding(name.clone(), default);
                    Some(name)
                } else {
                    None
                };
                if let Some(receiver) = &fun.receiver {
                    let _ = ctx.lower_type_ref(receiver)?;
                }
                for p in &fun.params {
                    if let Some(ty) = &p.ty {
                        let _ = ctx.lower_type_ref(ty)?;
                    }
                }
                if let Some(ret) = &fun.return_ty {
                    let _ = ctx.lower_type_ref(ret)?;
                }
                // T0458：`where` 子句中的 bound 同样属于 type position，需要参与 lowering。
                if let Some(w) = &fun.where_clause {
                    for c in &w.constraints {
                        let _ = ctx.lower_type_ref(&c.bound)?;
                    }
                }
                ctx.pop_type_params(&fun.type_params);
                if eff_binding.is_some() {
                    ctx.pop_effect_row_param_binding();
                }
            }
            ast::Item::ExtensionProperty(p) => {
                ctx.push_type_params(&p.type_params);
                let _ = ctx.lower_type_ref(&p.receiver)?;
                if let Some(ty) = &p.ty {
                    let _ = ctx.lower_type_ref(ty)?;
                }
                ctx.pop_type_params(&p.type_params);
            }
            ast::Item::Val(v) => {
                if let Some(ty) = &v.ty {
                    let _ = ctx.lower_type_ref(ty)?;
                }
            }
            ast::Item::Type(ty) => {
                ctx.check_type_decl_headers(ty)?;
            }
            ast::Item::Object(obj) => {
                ctx.check_object_decl_headers(obj)?;
            }
        }
    }

    Ok(())
}

pub(super) struct TypeLowering<'a> {
    source: &'a SourceFile,
    index: &'a Index,
    imports: ImportTable,
    env: &'a TypeEnv,
    types: &'a mut TypeStore,
    builtins: BuiltinTypes,
    pkg_prefix: String,
    /// type parameter 作用域栈：用于 lowering `T` 这类抽象类型引用。
    type_param_scopes: Vec<HashMap<String, TypeId>>,
    /// effect row parameter 作用域栈：用于 lowering `/ E` 这类 row 变量引用（T0509）。
    effect_row_param_scopes: Vec<HashMap<String, EffectRow>>,
    /// typealias 展开栈（用于循环检测；存储 alias FQN）。
    type_alias_stack: Vec<String>,
    /// 已展开 typealias 的缓存（alias FQN → lowered TypeId）。
    type_alias_cache: HashMap<String, TypeId>,
    /// 当前是否处于“required effects 收集”模式（T0604）。
    ///
    /// 说明：只在检查函数体时启用；其它 typecheck phase 默认关闭，避免改变现有行为。
    effect_collection_enabled: bool,
    /// effect 收集的“抑制深度”：
    /// - 进入 lambda body 时会暂时抑制（lambda 的 effect 属于函数值本身，而非外层函数立即执行的效果）。
    /// - 未来若引入更多“非立即执行”的语境（例如 `const`/comptime），同样可复用该机制。
    effect_collection_suspend_depth: usize,
    /// 记录（effect TypeId, perform span）。
    performed_effects: Vec<(TypeId, Span)>,
    /// 单态化（monomorphization）请求收集器（T0712）。
    ///
    /// 说明：
    /// - 该字段仅在“需要收集 monomorph 实例”的入口中启用；
    /// - typecheck 阶段遇到“泛型函数调用”时会把 (callee, type args) 记录下来，
    ///   供后续 monomorph pass 生成专用实例并做去重缓存。
    monomorph_requests: Option<MonomorphRequests>,

    /// unsafe 上下文深度（T1003/T1004）。
    ///
    /// 说明：
    /// - 在 `@Unsafe` 函数体内，调用 `@Extern/@Unsafe` 函数是允许的；
    /// - 在非 unsafe context 中，这类调用会在表达式 typecheck 阶段报错（T1003）；
    /// - 未来 `@Unsafe { ... }` block（T1004）会复用同一机制做局部 push/pop。
    unsafe_context_depth: usize,

    /// `@NoGC` 上下文深度（TODO T1005）。
    ///
    /// 说明：
    /// - 在 `@NoGC` 函数体内，编译器需要保守拒绝“可能分配”的行为；
    /// - 目前我们只实现最小静态门禁（调用点/已知装箱点），更完整分析留给后续任务；
    /// - 使用 depth 而不是 bool，便于未来扩展局部 `@NoGC { ... }` 或其它可嵌套语境。
    nogc_context_depth: usize,
}

impl<'a> TypeLowering<'a> {
    pub(super) fn new(
        source: &'a SourceFile,
        file: &'a ast::File,
        index: &'a Index,
        imports: &'a ImportTable,
        env: &'a TypeEnv,
        types: &'a mut TypeStore,
        builtins: BuiltinTypes,
    ) -> Self {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        Self::new_with_ctx(
            source,
            index,
            env,
            types,
            builtins,
            pkg_prefix,
            imports.clone(),
        )
    }

    /// 在指定的 package/import 上下文中创建 `TypeLowering`。
    ///
    /// 用途：
    /// - typealias 展开时切换到“别名声明处文件”的上下文（T0446）
    /// - enum layout/metadata 计算时按“enum 声明处文件”的上下文降低 variant 字段类型（T0449）
    pub(super) fn new_with_ctx(
        source: &'a SourceFile,
        index: &'a Index,
        env: &'a TypeEnv,
        types: &'a mut TypeStore,
        builtins: BuiltinTypes,
        pkg_prefix: String,
        imports: ImportTable,
    ) -> Self {
        Self {
            source,
            index,
            imports,
            env,
            types,
            builtins,
            pkg_prefix,
            type_param_scopes: Vec::new(),
            effect_row_param_scopes: Vec::new(),
            type_alias_stack: Vec::new(),
            type_alias_cache: HashMap::new(),
            effect_collection_enabled: false,
            effect_collection_suspend_depth: 0,
            performed_effects: Vec::new(),
            monomorph_requests: None,
            unsafe_context_depth: 0,
            nogc_context_depth: 0,
        }
    }

    pub(super) fn pkg_prefix(&self) -> &str {
        &self.pkg_prefix
    }

    pub(super) fn env(&self) -> &TypeEnv {
        self.env
    }

    pub(super) fn push_unsafe_context(&mut self) {
        self.unsafe_context_depth += 1;
    }

    pub(super) fn pop_unsafe_context(&mut self) {
        debug_assert!(self.unsafe_context_depth > 0);
        self.unsafe_context_depth = self.unsafe_context_depth.saturating_sub(1);
    }

    pub(super) fn in_unsafe_context(&self) -> bool {
        self.unsafe_context_depth > 0
    }

    pub(super) fn push_nogc_context(&mut self) {
        self.nogc_context_depth += 1;
    }

    pub(super) fn pop_nogc_context(&mut self) {
        debug_assert!(self.nogc_context_depth > 0);
        self.nogc_context_depth = self.nogc_context_depth.saturating_sub(1);
    }

    pub(super) fn in_nogc_context(&self) -> bool {
        self.nogc_context_depth > 0
    }

    /// 在一个临时区域中“抑制 `@NoGC` 上下文”。
    ///
    /// 用途：
    /// - lambda body 的代码并不在外层函数执行时立即运行；
    ///   为避免把 `@NoGC` 的限制错误地施加到 lambda body 上（产生大量假阳性），
    ///   这里提供一个最小的“暂停”机制。
    pub(super) fn with_nogc_context_suspended<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let saved = self.nogc_context_depth;
        self.nogc_context_depth = 0;
        let out = f(self);
        self.nogc_context_depth = saved;
        out
    }

    /// 开启 monomorph 请求收集（T0712）。
    ///
    /// 说明：默认情况下 typecheck 不收集这些信息，以避免在仅做语义检查时产生额外分配。
    pub(super) fn enable_monomorph_collection(&mut self) {
        self.monomorph_requests = Some(MonomorphRequests::default());
    }

    /// 取出并清空当前收集到的 monomorph keys。
    pub(super) fn take_monomorph_keys(&mut self) -> Vec<MonomorphKey> {
        self.monomorph_requests
            .take()
            .map(|r| r.into_vec())
            .unwrap_or_default()
    }

    /// 记录一次“泛型函数实例化调用”的 monomorph key（T0712）。
    ///
    /// 当前阶段约束：
    /// - 只记录“当前文件内”的顶层函数（decl_file 取 `self.source.path()`）；
    /// - effect row args（`<eff E>`）的实例化在后续任务接入（此处先留空）。
    pub(super) fn record_monomorph_call(
        &mut self,
        callee_fqn: String,
        callee_decl_span: Span,
        type_args: &[TypeId],
    ) {
        let Some(req) = self.monomorph_requests.as_mut() else {
            return;
        };
        if type_args.is_empty() {
            return;
        }

        let symbol = MonomorphSymbol {
            fqn: callee_fqn,
            decl_file: self.source.path().to_path_buf(),
            decl_span: callee_decl_span,
        };
        req.record(MonomorphKey {
            symbol,
            type_args: type_args.to_vec(),
            eff_args: Vec::new(),
        });
    }

    pub(super) fn push_effect_row_param_binding(&mut self, name: String, row: EffectRow) {
        let mut scope = HashMap::new();
        scope.insert(name, row);
        self.effect_row_param_scopes.push(scope);
    }

    pub(super) fn pop_effect_row_param_binding(&mut self) {
        let _ = self.effect_row_param_scopes.pop();
    }

    pub(super) fn index(&self) -> &Index {
        self.index
    }

    pub(super) fn is_object_type(&self, fqn: &str) -> bool {
        self.index.object_types.contains(fqn)
    }

    /// 在“声明处文件”的 package/import 上下文中 lower 一个 `TypeRef`。
    ///
    /// 用途：
    /// - 从 `Index` 侧的签名信息（如 ctor params）进行 typecheck 时，
    ///   需要按声明处的解析规则（import/star import）来降低类型引用；
    /// - 该方法会在缺少 env 上下文时回退到当前 `TypeLowering` 的上下文（保持健壮性）。
    pub(super) fn lower_type_ref_in_decl_file(
        &mut self,
        decl_file: &Path,
        ty: &ast::TypeRef,
    ) -> Result<TypeId, TypeLowerError> {
        let decl_source = self.env.source(decl_file).unwrap_or(self.source);
        let (pkg_prefix, imports) = match self.env.file_type_context(decl_file) {
            Some(ctx) => (ctx.pkg_prefix.clone(), ctx.imports.clone()),
            None => (self.pkg_prefix.clone(), self.imports.clone()),
        };

        let mut ctx = TypeLowering::new_with_ctx(
            decl_source,
            self.index,
            self.env,
            self.types,
            self.builtins,
            pkg_prefix,
            imports,
        );
        ctx.lower_type_ref(ty)
    }

    /// 在“声明处文件”的 package/import 上下文中 lower 一个 `TypeRef`，并注入 use-site type args。
    ///
    /// 用途（T0458）：
    /// - `where` 子句满足性检查需要把 bound 中出现的 `T` 等 type param 引用替换为具体实参类型；
    /// - 这里用 `push_type_param_bindings` 直接把 name → TypeId 映射写入 lowering 作用域。
    pub(super) fn lower_type_ref_in_decl_file_with_bindings(
        &mut self,
        decl_file: &Path,
        bindings: impl IntoIterator<Item = (String, TypeId)>,
        ty: &ast::TypeRef,
    ) -> Result<TypeId, TypeLowerError> {
        let decl_source = self.env.source(decl_file).unwrap_or(self.source);
        let (pkg_prefix, imports) = match self.env.file_type_context(decl_file) {
            Some(ctx) => (ctx.pkg_prefix.clone(), ctx.imports.clone()),
            None => (self.pkg_prefix.clone(), self.imports.clone()),
        };

        let mut ctx = TypeLowering::new_with_ctx(
            decl_source,
            self.index,
            self.env,
            self.types,
            self.builtins,
            pkg_prefix,
            imports,
        );
        ctx.push_type_param_bindings(bindings);
        let out = ctx.lower_type_ref(ty);
        ctx.pop_type_param_bindings();
        out
    }

    /// 在“声明处文件”的 package/import 上下文中 lower 一个 effect row expression，并注入 use-site type args。
    ///
    /// 用途（T0511）：
    /// - nominal type 的 `eff` row 参数默认值写在声明处；
    /// - use-site 省略 `Type<eff ...>` 时，需要在声明文件的解析规则下计算默认 row，
    ///   并把其中出现的 type params（如 `Raise<T>`）替换为使用点的具体类型实参。
    pub(super) fn lower_effect_row_expr_in_decl_file_with_bindings(
        &mut self,
        decl_file: &Path,
        bindings: impl IntoIterator<Item = (String, TypeId)>,
        expr: Option<&ast::EffectRowExpr>,
    ) -> Result<EffectRow, TypeLowerError> {
        let decl_source = self.env.source(decl_file).unwrap_or(self.source);
        let (pkg_prefix, imports) = match self.env.file_type_context(decl_file) {
            Some(ctx) => (ctx.pkg_prefix.clone(), ctx.imports.clone()),
            None => (self.pkg_prefix.clone(), self.imports.clone()),
        };

        let mut ctx = TypeLowering::new_with_ctx(
            decl_source,
            self.index,
            self.env,
            self.types,
            self.builtins,
            pkg_prefix,
            imports,
        );
        ctx.push_type_param_bindings(bindings);
        let out = ctx.lower_effect_row_expr(expr);
        ctx.pop_type_param_bindings();
        out
    }

    /// 在“声明处文件”的 package/import 上下文中 lower 一个 effect row expression，并同时注入：
    /// - use-site type param 绑定（`T` → 具体 TypeId）
    /// - effect row param 绑定（`E` → 具体 EffectRow）
    ///
    /// 用途（T0609）：
    /// - override/interface impl 的 effect row 检查需要先对 receiver 的 use-site type args 与
    ///   `<eff E>` row 参数做 substitution，再比较 `R_over ⊆ R_base`。
    pub(super) fn lower_effect_row_expr_in_decl_file_with_scopes(
        &mut self,
        decl_file: &Path,
        type_bindings: impl IntoIterator<Item = (String, TypeId)>,
        eff_bindings: impl IntoIterator<Item = (String, EffectRow)>,
        expr: Option<&ast::EffectRowExpr>,
    ) -> Result<EffectRow, TypeLowerError> {
        let decl_source = self.env.source(decl_file).unwrap_or(self.source);
        let (pkg_prefix, imports) = match self.env.file_type_context(decl_file) {
            Some(ctx) => (ctx.pkg_prefix.clone(), ctx.imports.clone()),
            None => (self.pkg_prefix.clone(), self.imports.clone()),
        };

        let mut ctx = TypeLowering::new_with_ctx(
            decl_source,
            self.index,
            self.env,
            self.types,
            self.builtins,
            pkg_prefix,
            imports,
        );

        ctx.push_type_param_bindings(type_bindings);
        let mut pushed_eff = 0usize;
        for (name, row) in eff_bindings {
            ctx.push_effect_row_param_binding(name, row);
            pushed_eff += 1;
        }

        let out = ctx.lower_effect_row_expr(expr);

        for _ in 0..pushed_eff {
            ctx.pop_effect_row_param_binding();
        }
        ctx.pop_type_param_bindings();
        out
    }

    /// 直接注入一组“使用点 type param 绑定”（name → TypeId）。
    ///
    /// 说明：
    /// - 与 `push_type_params` 相比，这里不分配新的 `TypeParamType`，而是把 `T` 直接映射到
    ///   “已实例化的实参类型”；
    /// - 该能力主要用于“在非 AST 语境内重用 TypeLowering”（例如 layout/metadata 计算）。
    pub(super) fn push_type_param_bindings(
        &mut self,
        bindings: impl IntoIterator<Item = (String, TypeId)>,
    ) {
        let mut scope = HashMap::new();
        for (name, id) in bindings {
            scope.insert(name, id);
        }
        self.type_param_scopes.push(scope);
    }

    pub(super) fn pop_type_param_bindings(&mut self) {
        let _ = self.type_param_scopes.pop();
    }

    pub(super) fn lower_type_ref(&mut self, ty: &ast::TypeRef) -> Result<TypeId, TypeLowerError> {
        match ty {
            ast::TypeRef::Path(p) => self.lower_type_path(p),
            ast::TypeRef::Tuple(t) => {
                if t.elements.is_empty() {
                    return Ok(self.builtins.unit);
                }
                let mut elements = Vec::with_capacity(t.elements.len());
                for e in &t.elements {
                    elements.push(self.lower_type_ref(e)?);
                }
                Ok(self.types.ty_tuple(elements))
            }
            ast::TypeRef::Nullable { inner, .. } => {
                let inner = self.lower_type_ref(inner)?;
                Ok(self.types.ty_option(inner))
            }
            ast::TypeRef::Star { .. } => {
                // spec §3.3：`*` 表示“未知类型实参（boxed ref view）”。
                //
                // 当前阶段（T0437）先把它 lowering 为 `Any`：
                // - 让 `List<*>` 这类类型在签名里可通过 lowering；
                // - 更精确的 read-only/装箱语义留给后续任务（与 codegen/boxing 联动）。
                Ok(self.builtins.any)
            }
            ast::TypeRef::EffectRowArg { span, .. } => Err(TypeLowerError::UnsupportedTypeRef {
                kind: "use-site effect row arg (`eff ...`)",
                span: (*span).into(),
            }),
            ast::TypeRef::Function(f) => {
                let receiver = match &f.receiver {
                    Some(r) => Some(self.lower_type_ref(r)?),
                    None => None,
                };
                let mut params = Vec::with_capacity(f.params.len());
                for p in &f.params {
                    params.push(self.lower_type_ref(p)?);
                }
                let return_ty = self.lower_type_ref(&f.return_ty)?;
                let effects = self.lower_effect_row_expr(f.effects.as_ref())?;
                Ok(self.types.ty_function(receiver, params, return_ty, effects))
            }
        }
    }

    pub(super) fn lower_effect_row_expr(
        &mut self,
        expr: Option<&ast::EffectRowExpr>,
    ) -> Result<EffectRow, TypeLowerError> {
        let Some(expr) = expr else {
            // spec §5.8.2：缺省效果为 Pure。
            return Ok(EffectRow::pure());
        };
        if expr.terms.is_empty() {
            return Ok(EffectRow::pure());
        }

        let mut terms: Vec<TypeId> = Vec::with_capacity(expr.terms.len());
        for term in &expr.terms {
            // T0509：effect row variable（`E`）在 lowering 阶段展开为其绑定的 row。
            if term.segments.len() == 1 && term.args.is_empty() {
                let name = self.source.slice(term.segments[0].span);
                // spec §5.8.4：闭合 row 的语义要求它是“不可逃逸”的边界；
                // 因此这里禁止在闭合 row 内直接引用 row 变量（例如 `E!`）。
                if expr.closed
                    && self
                        .effect_row_param_scopes
                        .iter()
                        .rev()
                        .any(|s| s.contains_key(name))
                {
                    return Err(TypeLowerError::ClosedEffectRowContainsRowVar {
                        name: name.to_string(),
                        span: term.span.into(),
                    });
                }
                if let Some(bound) = self
                    .effect_row_param_scopes
                    .iter()
                    .rev()
                    .find_map(|s| s.get(name))
                {
                    terms.extend(bound.terms.iter().copied());
                    continue;
                }
            }

            // effect item 的语法复用 `TypePath`；这里复用 TypeRef lowering，再做 kind 检查。
            let ty = self.lower_type_ref(&ast::TypeRef::Path(term.clone()))?;
            let ok = match self.type_kind(ty) {
                TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
                    matches!(
                        self.nominal_decl_kind(&nominal.fqn),
                        Some(ast::TypeKind::Effect)
                    )
                }
                _ => false,
            };

            if !ok {
                return Err(TypeLowerError::EffectRowItemNotEffect {
                    item: self.source.slice(term.span).to_string(),
                    found: self.fmt_type(ty),
                    span: term.span.into(),
                });
            }

            terms.push(ty);
        }

        Ok(EffectRow::new(terms))
    }

    pub(super) fn begin_effect_collection(&mut self) {
        self.effect_collection_enabled = true;
        self.effect_collection_suspend_depth = 0;
        self.performed_effects.clear();
    }

    pub(super) fn finish_effect_collection(&mut self) -> Vec<(TypeId, Span)> {
        self.effect_collection_enabled = false;
        self.effect_collection_suspend_depth = 0;
        std::mem::take(&mut self.performed_effects)
    }

    pub(super) fn with_effect_collection_suspended<R>(
        &mut self,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.effect_collection_suspend_depth += 1;
        let out = f(self);
        self.effect_collection_suspend_depth =
            self.effect_collection_suspend_depth.saturating_sub(1);
        out
    }

    pub(super) fn with_nested_effect_collection<R, E>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<R, E>,
    ) -> Result<(R, Vec<(TypeId, Span)>), E> {
        let saved_enabled = self.effect_collection_enabled;
        let saved_suspend = self.effect_collection_suspend_depth;
        let saved_effects = std::mem::take(&mut self.performed_effects);

        self.effect_collection_enabled = true;
        self.effect_collection_suspend_depth = 0;

        let result = f(self);

        let collected = std::mem::take(&mut self.performed_effects);
        self.effect_collection_enabled = saved_enabled;
        self.effect_collection_suspend_depth = saved_suspend;
        self.performed_effects = saved_effects;

        result.map(|value| (value, collected))
    }

    pub(super) fn record_performed_effect(&mut self, effect: TypeId, span: Span) {
        if !self.effect_collection_enabled {
            return;
        }
        if self.effect_collection_suspend_depth > 0 {
            return;
        }
        self.performed_effects.push((effect, span));
    }

    pub(super) fn fmt_type(&self, id: TypeId) -> String {
        self.types.display(id).to_string()
    }

    /// 返回给定 `TypeId` 在 `TypeStore` 中的具体 kind（clone）。
    ///
    /// 说明：typecheck 的某些表达式语义（例如 `with` 更新）需要区分：
    /// - 是否为值类型/引用类型
    /// - 是否为名义值类型（struct/enum）
    pub(super) fn type_kind(&self, id: TypeId) -> TypeKind {
        self.types.kind(id).clone()
    }

    /// 若给定 FQN 对应 nominal type，返回其声明的 `TypeKind`（struct/enum/class/interface/effect）。
    ///
    /// 用途：对“语义上只对某类 nominal type 生效”的规则做最小判定，例如：
    /// - `with` 更新当前阶段仅支持 `struct`
    pub(super) fn nominal_decl_kind(&self, fqn: &str) -> Option<ast::TypeKind> {
        let sym = self.env.type_symbol(fqn)?;
        match sym.kind {
            TypeSymbolKind::Nominal(kind) => Some(kind),
            TypeSymbolKind::TypeAlias => None,
        }
    }

    pub(super) fn is_ref(&self, id: TypeId) -> bool {
        self.types.is_ref(id)
    }

    pub(super) fn ty_option(&mut self, inner: TypeId) -> TypeId {
        self.types.ty_option(inner)
    }

    pub(super) fn ty_tuple(&mut self, elements: Vec<TypeId>) -> TypeId {
        self.types.ty_tuple(elements)
    }

    pub(super) fn ty_union(&mut self, variants: Vec<TypeId>) -> TypeId {
        self.types.ty_union(variants)
    }

    pub(super) fn ty_function(
        &mut self,
        receiver: Option<TypeId>,
        params: Vec<TypeId>,
        return_ty: TypeId,
        effects: EffectRow,
    ) -> TypeId {
        self.types.ty_function(receiver, params, return_ty, effects)
    }

    pub(super) fn intern_type_kind(&mut self, kind: TypeKind) -> TypeId {
        self.types.intern(kind)
    }

    /// 将一个声明处的 `TypeParam` 构造成 `TypeId`（`TypeKind::Param`）。
    ///
    /// 用途：
    /// - 在 typecheck 阶段某些场景（例如 class member body）需要构造 `Box<T>` 这类 “仍未实例化的泛型类型”，
    ///   此时 `T` 应当是 `TypeKind::Param` 而不是 `Any` 等占位。
    pub(super) fn ty_param_from_decl(&mut self, p: &ast::TypeParam) -> TypeId {
        let name = self.source.slice(p.name.span).to_string();
        self.types.ty_param(TypeParamType {
            name,
            decl_file: self.source.path().to_path_buf(),
            decl_span: p.name.span,
        })
    }

    pub(super) fn ty_param_named(&mut self, name: String, decl_file: PathBuf, decl_span: Span) -> TypeId {
        self.types.ty_param(TypeParamType {
            name,
            decl_file,
            decl_span,
        })
    }

    pub(super) fn push_type_params(&mut self, params: &[ast::TypeParam]) {
        if params.is_empty() {
            return;
        }

        let mut scope: HashMap<String, TypeId> = HashMap::new();
        for p in params {
            let name = self.source.slice(p.name.span).to_string();
            let id = self.types.ty_param(TypeParamType {
                name: name.clone(),
                decl_file: self.source.path().to_path_buf(),
                decl_span: p.name.span,
            });
            scope.insert(name, id);
        }
        self.type_param_scopes.push(scope);
    }

    pub(super) fn pop_type_params(&mut self, params: &[ast::TypeParam]) {
        if params.is_empty() {
            return;
        }
        let _ = self.type_param_scopes.pop();
    }

    fn lookup_type_param(&self, name: &str) -> Option<TypeId> {
        for scope in self.type_param_scopes.iter().rev() {
            if let Some(id) = scope.get(name).copied() {
                return Some(id);
            }
        }
        None
    }

    /// 将“已解析出的类型 FQN + 已 lowering 的 type args”构造成 `TypeId`。
    ///
    /// 说明：
    /// - `lower_type_ref`/`lower_type_path` 以 AST 为入口，会递归 lowering type args；
    /// - enum variant 构造等场景（T0426）需要先对 type args 做 substitution/推断，
    ///   再把结果组装回一个 `TypeId`，因此提供该辅助方法。
    pub(super) fn lower_type_fqn_with_args(
        &mut self,
        fqn: String,
        args: Vec<TypeId>,
        span: Span,
    ) -> Result<TypeId, TypeLowerError> {
        // 先对少数 builtin/special-case 做 lowering（不依赖 sysroot 声明/TypeEnv）。
        match fqn.as_str() {
            "scoop.core.Any" => {
                check_arity(&fqn, 0, args.len(), span)?;
                return Ok(self.builtins.any);
            }
            "scoop.core.String" => {
                check_arity(&fqn, 0, args.len(), span)?;
                return Ok(self.builtins.string);
            }
            "scoop.core.Unit" => {
                check_arity(&fqn, 0, args.len(), span)?;
                return Ok(self.builtins.unit);
            }
            "scoop.core.Nothing" => {
                check_arity(&fqn, 0, args.len(), span)?;
                return Ok(self.builtins.nothing);
            }
            "scoop.core.Bool" => {
                check_arity(&fqn, 0, args.len(), span)?;
                return Ok(self.builtins.bool_);
            }
            "scoop.core.Int" => {
                check_arity(&fqn, 0, args.len(), span)?;
                return Ok(self.builtins.int);
            }
            "scoop.core.UInt" => {
                check_arity(&fqn, 0, args.len(), span)?;
                return Ok(self.builtins.uint);
            }
            "scoop.core.Option" => {
                check_arity(&fqn, 1, args.len(), span)?;
                return Ok(self.types.ty_option(args[0]));
            }
            _ => {}
        }

        let expected = self.env.type_param_count(&fqn).ok_or_else(|| {
            TypeLowerError::MissingTypeSymbolInEnv {
                fqn: fqn.clone(),
                span: span.into(),
            }
        })?;
        let found = args.len();
        if expected != found {
            return Err(TypeLowerError::TypeArityMismatch {
                name: fqn,
                expected,
                found,
                span: span.into(),
            });
        }

        // 一般名义类型：保留为 nominal type（早期阶段不展开/不做布局分析）。
        let Some(sym) = self.env.type_symbol(&fqn) else {
            return Err(TypeLowerError::MissingTypeSymbolInEnv {
                fqn,
                span: span.into(),
            });
        };

        match sym.kind {
            TypeSymbolKind::TypeAlias => self.lower_type_alias_fqn(&fqn, span),
            TypeSymbolKind::Nominal(kind) => {
                self.check_where_constraints_on_instantiation(&fqn, sym, &args, span)?;
                let eff = match &sym.eff_param {
                    None => None,
                    Some(eff_param) => {
                        let bindings = sym
                            .type_param_names
                            .iter()
                            .cloned()
                            .zip(args.iter().copied())
                            .collect::<Vec<_>>();
                        Some(self.lower_effect_row_expr_in_decl_file_with_bindings(
                            &sym.decl_file,
                            bindings,
                            eff_param.default.as_ref(),
                        )?)
                    }
                };
                let nominal = NominalType { fqn, args, eff };
                let id = match kind {
                    ast::TypeKind::Struct | ast::TypeKind::Enum => self
                        .types
                        .intern(TypeKind::Value(ValueTypeKind::Nominal(nominal))),
                    ast::TypeKind::Class | ast::TypeKind::Interface | ast::TypeKind::Effect => self
                        .types
                        .intern(TypeKind::Ref(RefTypeKind::Nominal(nominal))),
                };
                Ok(id)
            }
        }
    }

    fn check_where_constraints_on_instantiation(
        &mut self,
        type_fqn: &str,
        sym: &TypeSymbol,
        args: &[TypeId],
        use_span: Span,
    ) -> Result<(), TypeLowerError> {
        if sym.where_constraints.is_empty() {
            return Ok(());
        }

        // name -> concrete type arg
        let bindings = sym
            .type_param_names
            .iter()
            .cloned()
            .zip(args.iter().copied())
            .collect::<Vec<_>>();

        for c in &sym.where_constraints {
            let Some(arg_ty) = args.get(c.param_index).copied() else {
                continue;
            };

            // 当实参本身仍是“未知 kind 的 type param”（例如在泛型声明内部出现 `Box<T>`）时，
            // 我们把 where 约束视为 **假设** 而不是 **需要此刻验证的条件**。
            //
            // 更完整的“约束传播/求解”（例如要求 `T` 也声明 `where T: Bound`）留给后续推断阶段（T05）。
            if matches!(self.type_kind(arg_ty), TypeKind::Param(_)) {
                continue;
            }

            // 在声明处文件上下文中 lowering bound，并用 use-site type args 对其中出现的 `T` 做 substitution。
            let bound_ty = self.lower_type_ref_in_decl_file_with_bindings(
                &sym.decl_file,
                bindings.iter().cloned(),
                &c.bound,
            )?;

            if is_type_assignable(arg_ty, bound_ty, self, self.builtins) {
                continue;
            }

            let param = sym
                .type_param_names
                .get(c.param_index)
                .cloned()
                .unwrap_or_else(|| format!("#{}", c.param_index + 1));

            return Err(TypeLowerError::WhereConstraintNotSatisfied {
                type_fqn: type_fqn.to_string(),
                param,
                arg: self.fmt_type(arg_ty),
                bound: self.fmt_type(bound_ty),
                span: use_span.into(),
            });
        }

        Ok(())
    }

    fn lower_type_path(&mut self, path: &ast::TypePath) -> Result<TypeId, TypeLowerError> {
        // 单段名且无实参：优先解析为当前作用域的 type parameter（它会 shadow 顶层同名 type）。
        if path.segments.len() == 1 && path.args.is_empty() {
            let name = self.source.slice(path.segments[0].span);
            if let Some(id) = self.lookup_type_param(name) {
                return Ok(id);
            }
        }

        let fqn = match self.resolve_type_path_fqn(path) {
            Ok(fqn) => fqn,
            Err(TypeLowerError::UnresolvedType { name, span }) => {
                let Some(builtin_fqn) = implicit_builtin_type_fqn(&name) else {
                    return Err(TypeLowerError::UnresolvedType { name, span });
                };
                builtin_fqn.to_string()
            }
            Err(other) => return Err(other),
        };

        // T0253/T0511：
        // - use-site effect row 实参（`eff ...`）在 AST 中被建模为 `TypeRef::EffectRowArg`；
        // - 它不属于“类型参数”（arity），但会影响 nominal type identity 与后续调用检查。
        let mut eff_arg: Option<&ast::EffectRowExpr> = None;
        for a in &path.args {
            if let ast::TypeRef::EffectRowArg { row, .. } = a {
                // parser 已保证最多一个且位于末尾；这里保持健壮性取第一个。
                eff_arg.get_or_insert(row);
            }
        }
        let type_args = path
            .args
            .iter()
            .filter(|a| !matches!(a, ast::TypeRef::EffectRowArg { .. }))
            .collect::<Vec<_>>();

        // 先对少数 builtin/special-case 做 lowering（不依赖 sysroot 声明/TypeEnv）。
        match fqn.as_str() {
            // `Any`：引用类型的顶层 supertype。
            "scoop.core.Any" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                return Ok(self.builtins.any);
            }
            // `String`：内建字符串类型。
            "scoop.core.String" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                return Ok(self.builtins.string);
            }
            // `Unit`：0 元 tuple。
            "scoop.core.Unit" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                return Ok(self.builtins.unit);
            }
            // `Nothing`：bottom type。
            "scoop.core.Nothing" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                return Ok(self.builtins.nothing);
            }
            // `Bool`：内建布尔类型。
            "scoop.core.Bool" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                return Ok(self.builtins.bool_);
            }
            // `Int/UInt`：word-sized 整数。
            "scoop.core.Int" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                return Ok(self.builtins.int);
            }
            "scoop.core.UInt" => {
                check_arity(&fqn, 0, type_args.len(), path.span)?;
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                return Ok(self.builtins.uint);
            }
            // `Option<T>`：值类型；同时也是 `T?` 的 desugar 目标。
            "scoop.core.Option" => {
                check_arity(&fqn, 1, type_args.len(), path.span)?;
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                let inner = self.lower_type_ref(type_args[0])?;
                return Ok(self.types.ty_option(inner));
            }
            _ => {}
        }

        let expected = self.env.type_param_count(&fqn).ok_or_else(|| {
            TypeLowerError::MissingTypeSymbolInEnv {
                fqn: fqn.clone(),
                span: path.span.into(),
            }
        })?;
        let found = type_args.len();
        if expected != found {
            return Err(TypeLowerError::TypeArityMismatch {
                name: fqn,
                expected,
                found,
                span: path.span.into(),
            });
        }

        // 一般名义类型：保留为 nominal type（早期阶段不展开/不做布局分析）。
        let Some(sym) = self.env.type_symbol(&fqn) else {
            return Err(TypeLowerError::MissingTypeSymbolInEnv {
                fqn,
                span: path.span.into(),
            });
        };

        let args = type_args
            .iter()
            .map(|a| self.lower_type_ref(a))
            .collect::<Result<Vec<_>, _>>()?;

        match sym.kind {
            TypeSymbolKind::TypeAlias => {
                if eff_arg.is_some() {
                    return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                        name: fqn,
                        span: path.span.into(),
                    });
                }
                self.lower_type_alias_fqn(&fqn, path.span)
            }
            TypeSymbolKind::Nominal(kind) => {
                self.check_where_constraints_on_instantiation(&fqn, sym, &args, path.span)?;
                let eff = match (&sym.eff_param, eff_arg) {
                    (None, None) => None,
                    (None, Some(_)) => {
                        return Err(TypeLowerError::UseSiteEffectRowArgNotAllowed {
                            name: fqn,
                            span: path.span.into(),
                        });
                    }
                    (Some(_), Some(expr)) => Some(self.lower_effect_row_expr(Some(expr))?),
                    (Some(eff_param), None) => {
                        let bindings = sym
                            .type_param_names
                            .iter()
                            .cloned()
                            .zip(args.iter().copied())
                            .collect::<Vec<_>>();
                        Some(self.lower_effect_row_expr_in_decl_file_with_bindings(
                            &sym.decl_file,
                            bindings,
                            eff_param.default.as_ref(),
                        )?)
                    }
                };
                let nominal = NominalType { fqn, args, eff };
                let id = match kind {
                    ast::TypeKind::Struct | ast::TypeKind::Enum => self
                        .types
                        .intern(TypeKind::Value(ValueTypeKind::Nominal(nominal))),
                    ast::TypeKind::Class | ast::TypeKind::Interface | ast::TypeKind::Effect => self
                        .types
                        .intern(TypeKind::Ref(RefTypeKind::Nominal(nominal))),
                };
                Ok(id)
            }
        }
    }

    fn lower_type_alias_fqn(
        &mut self,
        fqn: &str,
        use_span: Span,
    ) -> Result<TypeId, TypeLowerError> {
        if let Some(id) = self.type_alias_cache.get(fqn).copied() {
            return Ok(id);
        }

        if let Some(pos) = self.type_alias_stack.iter().position(|x| x == fqn) {
            // 构造 cycle chain：A -> B -> ... -> A
            let mut chain = self.type_alias_stack[pos..]
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            chain.push(fqn.to_string());
            let cycle = chain.join(" -> ");

            // 至少指出两个声明点：cycle 起点 + 当前栈顶（即“把我们带回去”的别名）。
            let first_fqn = chain.first().map(|s| s.as_str()).unwrap_or(fqn);
            let second_fqn = self
                .type_alias_stack
                .last()
                .map(|s| s.as_str())
                .unwrap_or(fqn);

            let first = self
                .env
                .type_alias(first_fqn)
                .map(|i| i.name_span)
                .unwrap_or(use_span);
            let second = self
                .env
                .type_alias(second_fqn)
                .map(|i| i.name_span)
                .unwrap_or(use_span);

            return Err(TypeLowerError::CyclicTypeAlias {
                cycle,
                first: first.into(),
                second: second.into(),
            });
        }

        let Some(info) = self.env.type_alias(fqn) else {
            return Err(TypeLowerError::MissingTypeSymbolInEnv {
                fqn: fqn.to_string(),
                span: use_span.into(),
            });
        };

        let Some(decl_source) = self.env.source(&info.decl_file) else {
            return Err(TypeLowerError::MissingTypeSymbolInEnv {
                fqn: fqn.to_string(),
                span: use_span.into(),
            });
        };

        let Some(decl_ctx) = self.env.file_type_context(&info.decl_file) else {
            return Err(TypeLowerError::MissingTypeSymbolInEnv {
                fqn: fqn.to_string(),
                span: use_span.into(),
            });
        };

        self.type_alias_stack.push(fqn.to_string());

        // 在“别名声明处文件”的 package/import 规则下展开 RHS。
        //
        // 注意：typealias 当前阶段（T0251）是顶层且非泛型，因此不应捕获使用点的 type param scope。
        let saved_source = self.source;
        let saved_pkg_prefix = std::mem::take(&mut self.pkg_prefix);
        let saved_imports = std::mem::take(&mut self.imports);
        let saved_scopes = std::mem::take(&mut self.type_param_scopes);

        self.source = decl_source;
        self.pkg_prefix = decl_ctx.pkg_prefix.clone();
        self.imports = decl_ctx.imports.clone();

        let lowered_rhs = self.lower_type_ref(&info.ty);

        // 恢复 use-site 上下文（无论 RHS lowering 成功与否）。
        self.source = saved_source;
        self.pkg_prefix = saved_pkg_prefix;
        self.imports = saved_imports;
        self.type_param_scopes = saved_scopes;

        let _ = self.type_alias_stack.pop();

        let rhs_id = lowered_rhs?;
        self.type_alias_cache.insert(fqn.to_string(), rhs_id);
        Ok(rhs_id)
    }

    fn check_type_decl_headers(&mut self, ty: &ast::TypeDecl) -> Result<(), TypeLowerError> {
        // `TypeDecl` 的 type params 在其 header/body 的所有 type position 内可见。
        self.push_type_params(&ty.type_params);
        let ty_eff_binding = if let Some(eff_param) = &ty.eff_param {
            let name = self.source.slice(eff_param.name.span).to_string();
            let default = match eff_param.default.as_ref() {
                Some(expr) => self.lower_effect_row_expr(Some(expr))?,
                None => EffectRow::pure(),
            };
            self.push_effect_row_param_binding(name, default);
            true
        } else {
            false
        };

        // 变型位置规则（Appendix B.4）：在 lowering 过程中做最小静态校验。
        self.check_type_decl_variance_rules(ty)?;

        // T0458：`where` 子句中的 bound 也需要参与 lowering（arity/存在性等由 lowering 负责）。
        if let Some(w) = &ty.where_clause {
            for c in &w.constraints {
                let _ = self.lower_type_ref(&c.bound)?;
            }
        }

        // 主构造头参数类型
        if let Some(primary_ctor) = &ty.primary_ctor {
            for p in &primary_ctor.params {
                if let Some(ty) = &p.ty {
                    let _ = self.lower_type_ref(ty)?;
                }
            }
        }

        // 继承/实现列表类型
        for st in &ty.supertypes {
            let _ = self.lower_type_ref(&st.ty)?;
        }

        // 成员签名类型（property/fun/nested type）
        let Some(body) = &ty.body else {
            self.pop_type_params(&ty.type_params);
            return Ok(());
        };

        for member in &body.members {
            match member {
                ast::TypeMember::EnumVariant(v) => {
                    for p in &v.params {
                        if let Some(ty) = &p.ty {
                            let _ = self.lower_type_ref(ty)?;
                        }
                    }
                }
                ast::TypeMember::Property(p) => {
                    if let Some(ty) = &p.ty {
                        let _ = self.lower_type_ref(ty)?;
                    }
                }
                ast::TypeMember::InitBlock(_b) => {
                    // init block 属于初始化执行体；当前阶段 type lowering 仅处理声明头与成员签名。
                }
                ast::TypeMember::SecondaryCtor(ctor) => {
                    // 次构造器参数类型同样属于成员签名类型（T0257）。
                    for p in &ctor.params {
                        if let Some(ty) = &p.ty {
                            let _ = self.lower_type_ref(ty)?;
                        }
                    }
                }
                ast::TypeMember::Fun(f) => {
                    self.push_type_params(&f.type_params);
                    let fun_eff_binding = if let Some(eff_param) = &f.eff_param {
                        let name = self.source.slice(eff_param.name.span).to_string();
                        let default = match eff_param.default.as_ref() {
                            Some(expr) => self.lower_effect_row_expr(Some(expr))?,
                            None => EffectRow::pure(),
                        };
                        self.push_effect_row_param_binding(name, default);
                        true
                    } else {
                        false
                    };
                    if let Some(receiver) = &f.receiver {
                        let _ = self.lower_type_ref(receiver)?;
                    }
                    for p in &f.params {
                        if let Some(ty) = &p.ty {
                            let _ = self.lower_type_ref(ty)?;
                        }
                    }
                    if let Some(ret) = &f.return_ty {
                        let _ = self.lower_type_ref(ret)?;
                    }
                    if let Some(w) = &f.where_clause {
                        for c in &w.constraints {
                            let _ = self.lower_type_ref(&c.bound)?;
                        }
                    }
                    if fun_eff_binding {
                        self.pop_effect_row_param_binding();
                    }
                    self.pop_type_params(&f.type_params);
                }
                ast::TypeMember::Type(nested) => {
                    self.check_type_decl_headers(nested)?;
                }
                ast::TypeMember::Object(obj) => {
                    self.check_object_decl_headers(obj)?;
                }
            }
        }

        if ty_eff_binding {
            self.pop_effect_row_param_binding();
        }
        self.pop_type_params(&ty.type_params);
        Ok(())
    }

    fn check_object_decl_headers(&mut self, obj: &ast::ObjectDecl) -> Result<(), TypeLowerError> {
        // 继承/实现列表类型
        for st in &obj.supertypes {
            let _ = self.lower_type_ref(&st.ty)?;
        }

        let Some(body) = &obj.body else {
            return Ok(());
        };

        for member in &body.members {
            match member {
                ast::TypeMember::EnumVariant(v) => {
                    for p in &v.params {
                        if let Some(ty) = &p.ty {
                            let _ = self.lower_type_ref(ty)?;
                        }
                    }
                }
                ast::TypeMember::Property(p) => {
                    if let Some(ty) = &p.ty {
                        let _ = self.lower_type_ref(ty)?;
                    }
                }
                ast::TypeMember::InitBlock(_b) => {}
                ast::TypeMember::SecondaryCtor(ctor) => {
                    for p in &ctor.params {
                        if let Some(ty) = &p.ty {
                            let _ = self.lower_type_ref(ty)?;
                        }
                    }
                }
                ast::TypeMember::Fun(f) => {
                    self.push_type_params(&f.type_params);
                    let fun_eff_binding = if let Some(eff_param) = &f.eff_param {
                        let name = self.source.slice(eff_param.name.span).to_string();
                        let default = match eff_param.default.as_ref() {
                            Some(expr) => self.lower_effect_row_expr(Some(expr))?,
                            None => EffectRow::pure(),
                        };
                        self.push_effect_row_param_binding(name, default);
                        true
                    } else {
                        false
                    };
                    if let Some(receiver) = &f.receiver {
                        let _ = self.lower_type_ref(receiver)?;
                    }
                    for p in &f.params {
                        if let Some(ty) = &p.ty {
                            let _ = self.lower_type_ref(ty)?;
                        }
                    }
                    if let Some(ret) = &f.return_ty {
                        let _ = self.lower_type_ref(ret)?;
                    }
                    if fun_eff_binding {
                        self.pop_effect_row_param_binding();
                    }
                    self.pop_type_params(&f.type_params);
                }
                ast::TypeMember::Type(nested) => {
                    self.check_type_decl_headers(nested)?;
                }
                ast::TypeMember::Object(nested) => {
                    self.check_object_decl_headers(nested)?;
                }
            }
        }

        Ok(())
    }

    /// 检查声明处变型（`in`/`out`）的最小位置规则（Appendix B.4）。
    ///
    /// 当前阶段只覆盖 type body 的“公开签名”层：
    /// - member fun：参数为 in-position，返回值为 out-position（receiver 视作第一个参数）
    /// - member property：`val` 为 out-position，`var` 视为 in+out（invariant）
    fn check_type_decl_variance_rules(&mut self, ty: &ast::TypeDecl) -> Result<(), TypeLowerError> {
        // 无需为“全 invariant”声明做额外遍历。
        if ty.type_params.iter().all(|p| p.variance.is_none()) {
            return Ok(());
        }

        // 只关心显式标注了 `in/out` 的参数；invariant 参数不参与位置限制。
        let mut variance_params: HashMap<String, ast::TypeParamVariance> = HashMap::new();
        for p in &ty.type_params {
            let Some(v) = p.variance else {
                continue;
            };
            let name = self.source.slice(p.name.span).to_string();
            variance_params.insert(name, v);
        }
        if variance_params.is_empty() {
            return Ok(());
        }

        let Some(body) = &ty.body else {
            return Ok(());
        };

        for member in &body.members {
            match member {
                ast::TypeMember::EnumVariant(v) => {
                    // enum variant 的 payload 字段类型会出现在构造器参数位置（in-position）。
                    for p in &v.params {
                        let Some(ty) = &p.ty else {
                            continue;
                        };
                        self.check_type_ref_variance(ty, VariancePos::In, &variance_params)?;
                    }
                }
                ast::TypeMember::Property(p) => {
                    let Some(ty) = &p.ty else {
                        continue;
                    };
                    let pos = match p.kind {
                        ast::ValKind::Val => VariancePos::Out,
                        ast::ValKind::Var => VariancePos::Invariant,
                    };
                    self.check_type_ref_variance(ty, pos, &variance_params)?;
                }
                ast::TypeMember::InitBlock(_b) => {}
                ast::TypeMember::SecondaryCtor(ctor) => {
                    for p in &ctor.params {
                        let Some(ty) = &p.ty else {
                            continue;
                        };
                        self.check_type_ref_variance(ty, VariancePos::In, &variance_params)?;
                    }
                }
                ast::TypeMember::Fun(f) => {
                    // receiver 视作第一个参数：in-position。
                    if let Some(receiver) = &f.receiver {
                        self.check_type_ref_variance(receiver, VariancePos::In, &variance_params)?;
                    }
                    for p in &f.params {
                        let Some(ty) = &p.ty else {
                            continue;
                        };
                        self.check_type_ref_variance(ty, VariancePos::In, &variance_params)?;
                    }
                    if let Some(ret) = &f.return_ty {
                        self.check_type_ref_variance(ret, VariancePos::Out, &variance_params)?;
                    }
                }
                ast::TypeMember::Type(_) | ast::TypeMember::Object(_) => {
                    // nested 声明会在其自身的 check 中处理；
                    // object 声明不引入新的声明处变型信息，且其成员是否属于“public API”有待
                    // 更完整的可见性/inner 规则确定，因此当前阶段不做递归检查。
                }
            }
        }

        Ok(())
    }

    fn check_type_ref_variance(
        &self,
        ty: &ast::TypeRef,
        pos: VariancePos,
        variance_params: &HashMap<String, ast::TypeParamVariance>,
    ) -> Result<(), TypeLowerError> {
        match ty {
            ast::TypeRef::Path(p) => {
                // 1) 直接引用当前声明的 type param（`T`）。
                if p.segments.len() == 1 && p.args.is_empty() {
                    let name = self.source.slice(p.segments[0].span);
                    if let Some(declared) = variance_params.get(name).copied() {
                        let ok = match (declared, pos) {
                            (ast::TypeParamVariance::Out, VariancePos::Out) => true,
                            (ast::TypeParamVariance::In, VariancePos::In) => true,
                            // invariant position 同时要求 in/out 都可用，因此对 in/out 均不合法
                            // （例如 `var x: T` 会同时把 T 用于 getter/setter）。
                            _ => false,
                        };
                        if !ok {
                            let declared = match declared {
                                ast::TypeParamVariance::Out => "out",
                                ast::TypeParamVariance::In => "in",
                            };
                            return Err(TypeLowerError::VariancePositionViolation {
                                param: name.to_string(),
                                declared,
                                position: pos.as_str(),
                                span: p.span.into(),
                            });
                        }
                    }
                    return Ok(());
                }

                // 2) 递归检查 type args：按被引用类型的声明处 variance 组合位置（Kotlin-like）。
                //
                // 说明：
                // - use-site effect row args 不属于 type args（arity），因此在这里忽略；
                // - `*` 不引入 type param 引用（但作为 `Any` view 会在 lowering 时处理）。
                let type_args = p
                    .args
                    .iter()
                    .filter(|a| !matches!(a, ast::TypeRef::EffectRowArg { .. }))
                    .collect::<Vec<_>>();

                if type_args.is_empty() {
                    return Ok(());
                }

                let fqn = match self.resolve_type_path_fqn(p) {
                    Ok(fqn) => fqn,
                    Err(TypeLowerError::UnresolvedType { name, .. }) => {
                        implicit_builtin_type_fqn(&name)
                            .unwrap_or(name.as_str())
                            .to_string()
                    }
                    Err(other) => return Err(other),
                };

                let declared_variances = self.env.type_param_variances(&fqn);

                for (idx, arg) in type_args.iter().copied().enumerate() {
                    let param_variance = declared_variances
                        .and_then(|v| v.get(idx).copied())
                        .unwrap_or(None);
                    let composed = pos.compose(param_variance);
                    self.check_type_ref_variance(arg, composed, variance_params)?;
                }

                Ok(())
            }
            ast::TypeRef::Tuple(t) => {
                for e in &t.elements {
                    self.check_type_ref_variance(e, pos, variance_params)?;
                }
                Ok(())
            }
            ast::TypeRef::Star { .. } => Ok(()),
            ast::TypeRef::EffectRowArg { row, .. } => {
                // effect row expr 的项复用 type path 语法；它们不是 type param，因此忽略。
                // 未来若引入 `eff` 的 row 变量/约束系统，再在对应任务中处理。
                let _ = row;
                Ok(())
            }
            ast::TypeRef::Function(f) => {
                // 函数类型：参数（含 receiver）逆变，返回值协变。
                let param_pos = pos.flip();
                if let Some(r) = &f.receiver {
                    self.check_type_ref_variance(r, param_pos, variance_params)?;
                }
                for p in &f.params {
                    self.check_type_ref_variance(p, param_pos, variance_params)?;
                }
                self.check_type_ref_variance(&f.return_ty, pos, variance_params)?;
                Ok(())
            }
            ast::TypeRef::Nullable { inner, .. } => {
                self.check_type_ref_variance(inner, pos, variance_params)
            }
        }
    }

    pub(super) fn resolve_type_path_fqn(
        &self,
        path: &ast::TypePath,
    ) -> Result<String, TypeLowerError> {
        let segments = path
            .segments
            .iter()
            .map(|id| id.text(self.source))
            .collect::<Vec<_>>();
        let local = segments.join(".");

        let mut candidates = Vec::new();
        if !self.pkg_prefix.is_empty() {
            candidates.push(format!("{}.{}", self.pkg_prefix, local));
        }
        // 允许显式写 FQN：`scoop.core.Any`
        candidates.push(local.clone());

        // 单段名字才走 import 规则（与 resolve 阶段保持一致）。
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
            let Some(sym) = syms.ty.as_ref() else {
                continue;
            };
            if is_symbol_visible_from(self.source, sym) {
                return Ok(fqn);
            }
        }

        Err(TypeLowerError::UnresolvedType {
            name: local,
            span: path.span.into(),
        })
    }

    /// 在 typecheck 阶段按“路径段名”解析 type path 对应的 FQN。
    ///
    /// 说明：
    /// - `resolve_type_path_fqn` 依赖 `TypePath` 里的 `Ident.span` 从 `self.source` 切片；
    /// - 但某些场景（例如 sysroot enum variant 的字段类型，T0426）持有的是“来自其它源文件”的
    ///   `TypeRef`/`TypePath`，其 span 不能再用于当前文件切片；
    /// - 因此提供该按字符串段名解析的辅助入口（仍复用当前使用点的 package/import/可见性规则）。
    pub(super) fn resolve_type_path_fqn_by_name(
        &self,
        segments: &[String],
        use_span: Span,
    ) -> Result<String, TypeLowerError> {
        let local = segments.join(".");

        let mut candidates = Vec::new();
        if !self.pkg_prefix.is_empty() {
            candidates.push(format!("{}.{}", self.pkg_prefix, local));
        }
        // 允许显式写 FQN：`scoop.core.Any`
        candidates.push(local.clone());

        // 单段名字才走 import 规则（与 resolve 阶段保持一致）。
        if segments.len() == 1 {
            let name = segments[0].as_str();

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
            let Some(sym) = syms.ty.as_ref() else {
                continue;
            };
            if is_symbol_visible_from(self.source, sym) {
                return Ok(fqn);
            }
        }

        Err(TypeLowerError::UnresolvedType {
            name: local,
            span: use_span.into(),
        })
    }
}

fn is_symbol_visible_from(source: &SourceFile, symbol: &crate::resolve::Symbol) -> bool {
    match symbol.visibility {
        Visibility::Public | Visibility::Internal => true,
        Visibility::Private => symbol.decl_file == source.path(),
    }
}

fn package_prefix(source: &SourceFile, pkg: Option<&ast::PackageDecl>) -> String {
    let Some(pkg) = pkg else {
        return String::new();
    };
    pkg.path
        .iter()
        .map(|id| source.slice(id.span))
        .collect::<Vec<_>>()
        .join(".")
}

/// 单态化（monomorphization）请求集合：去重 + 保留稳定顺序。
#[derive(Debug, Default)]
struct MonomorphRequests {
    seen: HashSet<MonomorphKey>,
    ordered: Vec<MonomorphKey>,
}

impl MonomorphRequests {
    fn record(&mut self, key: MonomorphKey) {
        if self.seen.insert(key.clone()) {
            self.ordered.push(key);
        }
    }

    fn into_vec(self) -> Vec<MonomorphKey> {
        self.ordered
    }
}

fn implicit_builtin_type_fqn(local_or_fqn: &str) -> Option<&'static str> {
    match local_or_fqn {
        // allow both `Int` and `scoop.core.Int` spellings
        "Any" | "scoop.core.Any" => Some("scoop.core.Any"),
        "String" | "scoop.core.String" => Some("scoop.core.String"),
        "Unit" | "scoop.core.Unit" => Some("scoop.core.Unit"),
        "Nothing" | "scoop.core.Nothing" => Some("scoop.core.Nothing"),
        "Bool" | "scoop.core.Bool" => Some("scoop.core.Bool"),
        "Int" | "scoop.core.Int" => Some("scoop.core.Int"),
        "UInt" | "scoop.core.UInt" => Some("scoop.core.UInt"),
        "Option" | "scoop.core.Option" => Some("scoop.core.Option"),
        "Continuation" | "scoop.core.Continuation" => Some("scoop.core.Continuation"),
        "Task" | "scoop.core.Task" => Some("scoop.core.Task"),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VariancePos {
    /// out-position（协变位置）：例如返回类型、`val` 属性类型。
    Out,
    /// in-position（逆变位置）：例如参数类型、receiver 类型。
    In,
    /// in+out（不变位置）：例如 `var` 属性类型。
    Invariant,
}

impl VariancePos {
    fn as_str(self) -> &'static str {
        match self {
            VariancePos::Out => "out",
            VariancePos::In => "in",
            VariancePos::Invariant => "invariant",
        }
    }

    fn flip(self) -> Self {
        match self {
            VariancePos::Out => VariancePos::In,
            VariancePos::In => VariancePos::Out,
            VariancePos::Invariant => VariancePos::Invariant,
        }
    }

    fn compose(self, declared: Option<ast::TypeParamVariance>) -> Self {
        let Some(declared) = declared else {
            // invariant type parameter：无论外部位置如何，都会把内部位置“压扁”为 invariant。
            return VariancePos::Invariant;
        };

        // Kotlin-like variance composition：
        // - out(+1) / in(-1)；组合是乘法；invariant(0) 会使整体变 invariant。
        match (self, declared) {
            (VariancePos::Invariant, _) => VariancePos::Invariant,
            (_, ast::TypeParamVariance::Out) => self,
            (VariancePos::Out, ast::TypeParamVariance::In) => VariancePos::In,
            (VariancePos::In, ast::TypeParamVariance::In) => VariancePos::Out,
        }
    }
}

fn check_arity(fqn: &str, expected: usize, found: usize, span: Span) -> Result<(), TypeLowerError> {
    if expected == found {
        return Ok(());
    }
    Err(TypeLowerError::TypeArityMismatch {
        name: fqn.to_string(),
        expected,
        found,
        span: span.into(),
    })
}
