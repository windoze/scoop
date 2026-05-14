//! AST → HIR 的最小 lowering（TODO T0701）。
//!
//! 说明：
//! - 这里的 lowering 仅用于 `dump-hir` 的调试输出，因此实现上优先保证“稳定输出 + 不 panic”；。
//! - 完整 lowering（含类型推断结果、更多语法节点）会在后续任务（TODO T0702+）逐步补齐。

mod decls;
mod types;
mod util;

#[cfg(test)]
mod placeholder_inventory;

mod patterns;
mod sugar;

mod block;
mod expr;
mod stmt;

pub use types::{HirLowerError, HirStageError, LoweredHir};
pub(crate) use util::GenericTemplateSymbolSuffixIndex;
pub use util::mangle_nominal_fqn;
pub(crate) use util::{
    canonical_generic_fun_signature_key, canonical_generic_property_getter_signature_key,
    collect_generic_template_symbol_suffixes, stable_instance_fqn,
};

use std::collections::{HashMap, HashSet};

use crate::ast;
use crate::parser::parse_file;
use crate::resolve::Index;
use crate::session::Session;
use crate::source::SourceFile;
use crate::span::Span;
use crate::stable_id::StableConeKey;
use crate::ty::{
    BuiltinTypes, EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeParamType, TypeStore,
    ValueTypeKind,
};

use super::EFFECT_ROW_PARAM_DECL_FILE;
use super::{
    AccessorContract, AssignPlaceSiteIndex, Block, CallArg, CallArgBindingSiteIndex, CallSite,
    ClassInitIndex, ContinuationResumeCallSiteIndex, CtorCallSiteIndex, CtorDecl, CtorParamDecl,
    Decl, DeclMember, DeclTypeParam, EnumVariantDecl, Expr, ExprKind, ExtensionPropertyDecl,
    FieldDecl, FieldOrigin, File, FunDecl, Item, MemberFunDecl, NominalDecl,
    NonPureContinuationResumeCallSiteIndex, ObjectDecl, ObjectInitIndex, Param, PropertyDecl, Stmt,
    StmtKind, SupertypeDecl, SymbolId, TopLevelVarStorage, TypeAliasDecl, ValDecl, ValueRef,
    WithUpdateSiteIndex,
};

use types::*;
use util::*;

fn collect_top_level_fun_call_sites(
    files: &[(&SourceFile, &ast::File)],
) -> crate::hir::TopLevelFunCallSiteIndex {
    let mut sites = HashMap::new();
    for (source, file) in files {
        for (span, binding) in file.top_level_fun_call_bindings() {
            sites.insert(CallSite::new(source.path().to_path_buf(), span), binding);
        }
    }
    sites
}

fn collect_top_level_fun_call_sites_with_type_remap(
    files: &[(&SourceFile, &ast::File)],
    typecheck_types: Option<&TypeStore>,
    types: &mut TypeStore,
) -> crate::hir::TopLevelFunCallSiteIndex {
    let mut sites = collect_top_level_fun_call_sites(files);
    let Some(typecheck_types) = typecheck_types else {
        return sites;
    };
    for binding in sites.values_mut() {
        binding.type_args = binding
            .type_args
            .iter()
            .map(|&ty| types.re_intern_from(typecheck_types, ty))
            .collect();
        binding.eff_args = binding
            .eff_args
            .iter()
            .map(|row| {
                crate::ty::EffectRow::new(
                    row.terms
                        .iter()
                        .map(|&ty| types.re_intern_from(typecheck_types, ty))
                        .collect(),
                )
            })
            .collect();
    }
    sites
}

fn collect_call_arg_bindings(files: &[(&SourceFile, &ast::File)]) -> CallArgBindingSiteIndex {
    let mut sites = HashMap::new();
    for (source, file) in files {
        for (span, binding) in file.typechecked_call_arg_bindings() {
            sites.insert(CallSite::new(source.path().to_path_buf(), span), binding);
        }
    }
    sites
}

/// HIR lowering 的上下文（按单文件构建，用于 `dump-hir` 与 HIR fixtures）。
struct HirLowering<'a> {
    source: &'a SourceFile,
    file: &'a ast::File,
    index: &'a Index,
    /// typecheck 阶段的 TypeStore（若存在）。
    ///
    /// 用途：把 `ast::File` side table 中记录的 typecheck `TypeId` 重新 intern 到当前 HIR 的
    /// `TypeStore`，从而在 lowering 阶段恢复“表达式最终类型”。
    typecheck_types: Option<&'a TypeStore>,
    /// `type fqn -> ast::TypeKind` 的最小索引，用于决定 nominal type 是 ref 还是 value。
    type_kinds: &'a HashMap<String, ast::TypeKind>,
    /// delegated property（spec §10.4）索引：`Owner.prop` → lowering 所需的合成符号信息。
    ///
    /// 用途：
    /// - `receiver.prop` 读取：降糖为 `receiver.prop$delegate.getValue(receiver, PropertyMeta)`；
    /// - `receiver.prop = value` 写入：降糖为 `receiver.prop$delegate.setValue(receiver, PropertyMeta, value)`。
    ///
    /// 注意：HIR lowering 的目标是 `dump-hir`/fixtures 的稳定输出，因此这里仅生成“调用形状”，
    /// 不要求 `$delegate` 字段/`PropertyMeta` 常量在后端可执行（真实 codegen 留给后续任务）。
    delegated_properties: &'a DelegatedPropertyIndex<'a>,
    /// 当前 lowering 可见的完整编译单元 AST（含 sysroot/同编译单元其它文件）。
    ///
    /// 用途：
    /// - 跨文件 struct 默认字段 lowering 需要回到“声明处 AST”读取默认值表达式；
    /// - 这里保留 `(SourceFile, ast::File)` 对，按 `decl_file` 做查找即可。
    compilation_unit: &'a [(&'a SourceFile, &'a ast::File)],
    /// 顶层函数默认参数信息索引：`fqn -> params(default)`（用于 call-site 默认参数补齐，T1305）。
    default_arg_funs: HashMap<String, DefaultArgFunInfo>,
    /// struct 直接字段默认值信息索引：`struct fqn -> direct field params(default)`。
    default_arg_structs: HashMap<String, DefaultArgStructInfo>,
    /// 值类型 computed property getter 索引：`Owner.prop`。
    ///
    /// 用途：
    /// - `struct/enum` 的 getter-only property 访问需要在 HIR 阶段降糖为 getter 调用；
    /// - 避免 LLVM/codegen 再把它误当作 direct field 去查 layout。
    value_type_computed_properties: &'a HashSet<String>,
    /// ctor 调用点候选集合：callee span → candidate type fqns。
    ///
    /// 说明：HIR v0 仍把 ctor 调用的 callee 降为 `UnresolvedIdent`，因此需要 side table
    /// 把 resolver 的 call candidates 保留下来，供 LLVM codegen 决定“这是 ctor call”。
    ctor_call_sites: CtorCallSiteIndex,
    /// 动态 dispatch 调用点 side table：`source_path + call span + receiver_ty` → dispatch kind。
    dispatch_call_sites: super::DispatchCallSiteIndex,
    /// effect-op 调用点绑定信息：`source_path + call span` → arg_mapping / payload tuple。
    effect_op_call_sites: super::EffectOpCallSiteIndex,
    /// handler arm 多 binder payload tuple 索引：`source_path + op head span` → tuple `TypeId`。
    handle_payload_tuple_tys: super::HandlePayloadTupleSiteIndex,
    /// `with` copy-update 的 typechecked aggregate/update contract。
    with_update_contracts: WithUpdateSiteIndex,
    /// assignment statement LHS 的 typed HIR place contract。
    assign_place_contracts: AssignPlaceSiteIndex,
    /// 顶层可变全局变量（`@ThreadLocal/@Global`）索引（TODO T1023）。
    top_level_vars: super::TopLevelVarIndex,
    /// `@Extern` 顶层变量索引。
    extern_globals: super::ExternGlobalIndex,
    /// 顶层 `const val` 索引：供后端在表达式位置按声明类型回放 initializer。
    top_level_consts: super::TopLevelConstIndex,
    /// 普通顶层 immutable value 索引：供后端生成 once-init + 稳定读取主线。
    top_level_immutable_values: super::TopLevelImmutableValueIndex,
    /// `when` pattern binder 的精确类型索引：供后端恢复 binder 的原始 `TypeId`。
    when_pat_binding_tys: super::WhenPatBindingTypeIndex,
    symbols: SymbolInterner,
    /// 本文件内的“局部 symbol → 是否可变（var）”信息。
    ///
    /// 用途：closure capture set 需要知道捕获目标是否为 `var`，以便在后续 MIR lowering（T0714）
    /// 侧把可变捕获降为 box/alias 语义。
    local_mutability: HashMap<SymbolId, bool>,
    next_closure: u32,
    /// 当前 receiver lambda 中隐式 `this` 的合成声明 span。
    ///
    /// 说明：
    /// - 普通函数 / 普通 lambda 为 `None`；
    /// - receiver lambda lowering body 时会覆盖为当前 lambda 的合成 `this` 绑定；
    /// - 嵌套普通 lambda 会继承外层 receiver lambda 的 `this`，嵌套 receiver lambda 会再次覆盖。
    lambda_this_decl_span: Option<Span>,
    /// 合成局部绑定计数器：用于给 lowering 生成的临时局部变量分配唯一 decl span / 名字。
    next_synthetic_local: usize,
    /// 合成 helper call-site 计数器：用于给 synthetic helper call 分配稳定且不与用户源码冲突的 span。
    next_synthetic_call_site: usize,
    /// 类型表（HIR 内所有 `TypeId` 必须来自同一个 store）。
    types: &'a mut TypeStore,
    builtins: BuiltinTypes,
    /// type parameter 作用域栈：用于 lowering `T` 这类抽象类型引用。
    type_param_scopes: Vec<HashMap<String, TypeId>>,
    /// effect row parameter 作用域栈：用于 lowering `/ E` 或 `Type<eff E>` 这类 row 变量引用。
    effect_row_param_scopes: Vec<HashMap<String, EffectRowParamBinding>>,
    /// generic template 的 overload-aware 符号后缀索引。
    ///
    /// 说明：
    /// - HIR 兼容 lowering 仍需要为 concrete generic direct-call / function-value target 生成实例 FQN；
    /// - 当同一 `template.fqn` 存在多个 generic overload 时，必须与 MIR materialization 使用同一套
    ///   stable overload suffix 规则，避免生产路径上的实例声明名与调用目标继续碰撞。
    generic_template_symbol_suffixes: &'a util::GenericTemplateSymbolSuffixIndex,
    /// exact receiver 判定时使用的“已知存在子类/子实现”的 nominal 集合。
    known_receiver_subclasses: &'a crate::devirtualize::KnownReceiverSubclassIndex,
    /// class vtable slots（供 explicit MIR instance lowering 的 devirtualization 兼容桥接使用）。
    class_vtables: &'a crate::vtable::ClassVtableIndex,
    /// interface metadata（供 explicit MIR instance lowering 的 devirtualization 兼容桥接使用）。
    interfaces: &'a crate::itable::InterfaceIndex,
    /// class itables（供 explicit MIR instance lowering 的 devirtualization 兼容桥接使用）。
    class_itables: &'a crate::itable::ClassItableIndex,
    /// 是否把已 concrete 的非 intrinsic direct-call target 物化为最终实例 FQN。
    ///
    /// compilation-unit / LLVM frontend lowering 需要开启它，以便 backend 直接消费实例身份；
    /// dump / generic-template lowering 必须关闭它，保持 generic MIR template 不提前单态化。
    materialize_direct_call_targets: bool,
    /// 是否在 explicit MIR instance lowering 的 HIR 兼容前端上执行 exact-receiver devirtualization。
    devirtualize_dispatch_calls: bool,
    /// refactor typed HIR 的语句级 `comptime` 展开计划。
    runtime_comptime_plan: Option<&'a crate::comptime::RuntimeComptimePlan>,
    /// 当前 `comptime for` 展开迭代中的编译期值绑定（decl span -> const value）。
    comptime_value_scopes: Vec<HashMap<Span, crate::comptime::ConstValue>>,
    /// 当前展开体中需要重映射的局部声明 span，避免同一源码 body 多次 unroll 后局部 SymbolId 冲突。
    local_decl_span_overrides: Vec<HashMap<Span, Span>>,
    /// lowering 过程中发现的第一个 typed HIR contract 错误。
    stage_error: Option<HirStageError>,
}

/// 构造 `HirLowering` 时用到的非必需上下文集合。
///
/// 说明：
/// - 这些字段在不同 lowering 入口之间经常成组出现；
/// - 单独打包后可以避免初始化函数参数过多，同时保持调用点语义明确。
struct HirLoweringSetup<'a> {
    typecheck_types: Option<&'a TypeStore>,
    type_kinds: &'a HashMap<String, ast::TypeKind>,
    delegated_properties: &'a DelegatedPropertyIndex<'a>,
    compilation_unit: &'a [(&'a SourceFile, &'a ast::File)],
    default_arg_structs: HashMap<String, DefaultArgStructInfo>,
    value_type_computed_properties: &'a HashSet<String>,
    builtins: BuiltinTypes,
    generic_template_symbol_suffixes: &'a util::GenericTemplateSymbolSuffixIndex,
    known_receiver_subclasses: &'a crate::devirtualize::KnownReceiverSubclassIndex,
    class_vtables: &'a crate::vtable::ClassVtableIndex,
    interfaces: &'a crate::itable::InterfaceIndex,
    class_itables: &'a crate::itable::ClassItableIndex,
    materialize_direct_call_targets: bool,
    devirtualize_dispatch_calls: bool,
    runtime_comptime_plan: Option<&'a crate::comptime::RuntimeComptimePlan>,
}

#[derive(Clone)]
enum EffectRowParamBinding {
    Placeholder(TypeId),
    Concrete(EffectRow),
}

impl<'a> HirLowering<'a> {
    const PROPERTY_META_FQN: &'static str = "scoop.core.PropertyMeta";
    const ARRAY_BUILDER_NEW_FQN: &'static str = "scoop.core.__scoop_array_builder_new";
    const ARRAY_BUILDER_PUSH_FQN: &'static str = "scoop.core.__scoop_array_builder_push";
    const ARRAY_BUILDER_BUILD_ARRAY_FQN: &'static str =
        "scoop.core.__scoop_array_builder_build_array";
    const INT_PROGRESSION_FQN: &'static str = "scoop.core.IntProgression";
    const RANGE_TO_FQN: &'static str = "scoop.core.rangeTo";
    const RANGE_DEFAULT_STEP_FQN: &'static str = "scoop.core.__scoop_range_default_step";
    const SYNC_MUTEX_TYPE_FQN: &'static str = "scoop.sync.Mutex";
    const SYNC_MUTEX_CREATE_FQN: &'static str = "scoop.sync.mutexCreate";
    const SYNC_MUTEX_LOCK_FQN: &'static str = "scoop.sync.lock";
    const SYNC_MUTEX_UNLOCK_FQN: &'static str = "scoop.sync.unlock";
    const ARRAY_BUILDER_BUILD_MUTABLE_ARRAY_FQN: &'static str =
        "scoop.core.__scoop_array_builder_build_mutable_array";
    const RAISE_RAISE_FQN: &'static str = "scoop.core.Raise.raise";
    const RUNTIME_ERROR_NULL_ASSERTION_FAILED_FQN: &'static str =
        "scoop.core.RuntimeError.NullAssertionFailed";

    fn new(
        source: &'a SourceFile,
        file: &'a ast::File,
        index: &'a Index,
        types: &'a mut TypeStore,
        setup: HirLoweringSetup<'a>,
    ) -> Self {
        let HirLoweringSetup {
            typecheck_types,
            type_kinds,
            delegated_properties,
            compilation_unit,
            default_arg_structs,
            value_type_computed_properties,
            builtins,
            generic_template_symbol_suffixes,
            known_receiver_subclasses,
            class_vtables,
            interfaces,
            class_itables,
            materialize_direct_call_targets,
            devirtualize_dispatch_calls,
            runtime_comptime_plan,
        } = setup;
        Self {
            source,
            file,
            index,
            typecheck_types,
            type_kinds,
            delegated_properties,
            compilation_unit,
            default_arg_funs: HashMap::new(),
            default_arg_structs,
            value_type_computed_properties,
            ctor_call_sites: HashMap::new(),
            dispatch_call_sites: HashMap::new(),
            effect_op_call_sites: HashMap::new(),
            handle_payload_tuple_tys: HashMap::new(),
            with_update_contracts: HashMap::new(),
            assign_place_contracts: HashMap::new(),
            top_level_vars: HashMap::new(),
            extern_globals: HashMap::new(),
            top_level_consts: HashMap::new(),
            top_level_immutable_values: HashMap::new(),
            when_pat_binding_tys: HashMap::new(),
            symbols: SymbolInterner::default(),
            local_mutability: HashMap::new(),
            next_closure: 0,
            lambda_this_decl_span: None,
            next_synthetic_local: 0,
            next_synthetic_call_site: 0,
            types,
            builtins,
            type_param_scopes: Vec::new(),
            effect_row_param_scopes: Vec::new(),
            generic_template_symbol_suffixes,
            known_receiver_subclasses,
            class_vtables,
            interfaces,
            class_itables,
            materialize_direct_call_targets,
            devirtualize_dispatch_calls,
            runtime_comptime_plan,
            comptime_value_scopes: Vec::new(),
            local_decl_span_overrides: Vec::new(),
            stage_error: None,
        }
    }

    fn record_stage_error(
        &mut self,
        span: Span,
        reason: impl Into<String>,
        owner: impl Into<String>,
    ) {
        if self.stage_error.is_none() {
            self.stage_error = Some(HirStageError::new(
                self.source.path().to_path_buf(),
                span,
                reason,
                owner,
            ));
        }
    }

    fn take_stage_error(&mut self) -> Option<HirStageError> {
        self.stage_error.take()
    }

    fn intern_local_symbol(&mut self, decl_span: Span, mutable: bool) -> SymbolId {
        let decl_span = self.remap_local_decl_span(decl_span);
        let id = self.symbols.intern_local(self.source.path(), decl_span);
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

    fn remap_local_decl_span(&self, decl_span: Span) -> Span {
        for scope in self.local_decl_span_overrides.iter().rev() {
            if let Some(remapped) = scope.get(&decl_span) {
                return *remapped;
            }
        }
        decl_span
    }

    fn with_local_decl_span_overrides<T>(
        &mut self,
        overrides: HashMap<Span, Span>,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.local_decl_span_overrides.push(overrides);
        let result = f(self);
        let _ = self.local_decl_span_overrides.pop();
        result
    }

    fn with_comptime_value_binding<T>(
        &mut self,
        decl_span: Span,
        value: crate::comptime::ConstValue,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let mut scope = HashMap::new();
        scope.insert(decl_span, value);
        self.comptime_value_scopes.push(scope);
        let result = f(self);
        let _ = self.comptime_value_scopes.pop();
        result
    }

    fn comptime_value_for_decl_span(
        &self,
        decl_span: Span,
    ) -> Option<&crate::comptime::ConstValue> {
        for scope in self.comptime_value_scopes.iter().rev() {
            if let Some(value) = scope.get(&decl_span) {
                return Some(value);
            }
        }
        None
    }

    fn record_when_pat_binding_ty(&mut self, decl_span: Span, ty: TypeId) {
        let site = super::WhenPatBindingSite {
            source_path: self.source.path().to_path_buf(),
            decl_span,
        };
        self.when_pat_binding_tys.insert(site, ty);
    }

    fn intern_effect_row_param_marker(&mut self, name: String, decl_span: Span) -> TypeId {
        self.types.intern(TypeKind::Param(TypeParamType {
            name,
            decl_file: std::path::PathBuf::from(EFFECT_ROW_PARAM_DECL_FILE),
            decl_span,
        }))
    }

    fn push_effect_row_param_placeholder(&mut self, name: String, decl_span: Span) {
        let mut scope = HashMap::new();
        let marker = self.intern_effect_row_param_marker(name.clone(), decl_span);
        scope.insert(name, EffectRowParamBinding::Placeholder(marker));
        self.effect_row_param_scopes.push(scope);
    }

    fn push_effect_row_param_binding(&mut self, name: String, row: EffectRow) {
        let mut scope = HashMap::new();
        scope.insert(name, EffectRowParamBinding::Concrete(row));
        self.effect_row_param_scopes.push(scope);
    }

    fn effect_row_param_is_bound(&self, name: &str) -> bool {
        self.effect_row_param_scopes
            .iter()
            .rev()
            .any(|scope| scope.contains_key(name))
    }

    fn push_missing_fun_effect_placeholder(
        &mut self,
        eff_param: Option<&ast::EffectRowParam>,
    ) -> bool {
        let Some(eff_param) = eff_param else {
            return false;
        };
        let name = eff_param.name.text(self.source).to_string();
        if self.effect_row_param_is_bound(&name) {
            return false;
        }
        self.push_effect_row_param_placeholder(name, eff_param.name.span);
        true
    }

    fn pop_effect_row_param_binding(&mut self) {
        let _ = self.effect_row_param_scopes.pop();
    }

    fn call_site(&self, span: Span) -> CallSite {
        CallSite::new(self.source.path().to_path_buf(), span)
    }

    fn fresh_synthetic_local(
        &mut self,
        anchor: Span,
        prefix: &str,
        mutable: bool,
    ) -> (Span, SymbolId, String) {
        let index = self.next_synthetic_local;
        self.next_synthetic_local = self.next_synthetic_local.saturating_add(1);

        // 刻意把合成 span 放到文件末尾之后，避免与真实源码 decl span / call-site 冲突。
        let base = self
            .source
            .text()
            .len()
            .saturating_add(index.saturating_mul(2));
        let span = Span::new(base, base.saturating_add(1));
        let id = self.intern_local_symbol(span, mutable);
        let name = format!("{prefix}_{index}");
        let _ = anchor; // 目前仅保留参数，便于后续若需改成“锚定到原语句附近”时不改调用点。
        (span, id, name)
    }

    fn fresh_synthetic_call_site_span(&mut self, anchor: Span) -> Span {
        let index = self.next_synthetic_call_site;
        self.next_synthetic_call_site = self.next_synthetic_call_site.saturating_add(1);

        // helper call-site 需要与既有 synthetic local span 稳定隔离，避免仅因 call-site identity
        // 修复而重排大量临时 local span / snapshot。
        let base = self
            .source
            .text()
            .len()
            .saturating_add(1 << 20)
            .saturating_add(index.saturating_mul(2));
        let _ = anchor;
        Span::new(base, base.saturating_add(1))
    }

    fn with_foreign_ast_context<T>(
        &mut self,
        source: &'a SourceFile,
        file: &'a ast::File,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let previous_source = self.source;
        let previous_file = self.file;
        self.source = source;
        self.file = file;
        let result = f(self);
        self.source = previous_source;
        self.file = previous_file;
        result
    }

    fn with_lambda_this_decl_span<T>(
        &mut self,
        lambda_this_decl_span: Option<Span>,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let previous = self.lambda_this_decl_span;
        self.lambda_this_decl_span = lambda_this_decl_span;
        let result = f(self);
        self.lambda_this_decl_span = previous;
        result
    }

    fn lower_file(&mut self) -> File {
        let pkg_prefix = package_prefix(self.source, self.file.package.as_ref());
        self.default_arg_funs = self.collect_default_arg_funs(&pkg_prefix);
        let mut decls = Vec::new();
        let mut items = Vec::with_capacity(self.file.items.len());

        for item in &self.file.items {
            self.lower_item_into(&pkg_prefix, item, &mut items, &mut decls);
        }

        File { decls, items }
    }

    /// 扫描当前源文件内的顶层 `fun` 声明，收集“默认参数”信息（供 call-site 默认参数补齐使用）。
    ///
    /// 注意：
    /// - 这里只服务于早期单文件 codegen，因此只索引“当前文件内”的顶层函数；
    /// - 不尝试处理 overload set（同名重载）与泛型函数（它们需要 typecheck 的最终决议信息）。
    fn collect_default_arg_funs(&mut self, pkg_prefix: &str) -> HashMap<String, DefaultArgFunInfo> {
        let mut out: HashMap<String, DefaultArgFunInfo> = HashMap::new();

        for item in &self.file.items {
            let ast::Item::Fun(fun) = item else {
                continue;
            };

            // 当前阶段：默认参数补齐仅针对“非泛型 + 非 receiver”的顶层函数。
            if fun.receiver.is_some() || !fun.type_params.is_empty() {
                continue;
            }
            // 当前阶段：不处理 vararg（其“缺省语义”是空数组/0 个元素，而非 param default_value）。
            if fun.params.iter().any(|p| p.is_vararg) {
                continue;
            }

            let name = fun.name.text(self.source).to_string();
            let fqn = if pkg_prefix.is_empty() {
                name
            } else {
                format!("{pkg_prefix}.{name}")
            };

            // 过滤 overload set：同一个 fqn 出现多次时不做索引（避免与 typecheck 的 overload 决议冲突）。
            if out.contains_key(&fqn) {
                let _ = out.remove(&fqn);
                continue;
            }

            let mut params: Vec<DefaultArgParamInfo> = Vec::with_capacity(fun.params.len());
            for p in &fun.params {
                let name = p.name.text(self.source).to_string();
                params.push(DefaultArgParamInfo {
                    decl_span: p.name.span,
                    name,
                    is_vararg: p.is_vararg,
                    ty_ref: p.ty.clone(),
                    default_value: p.default_value.clone(),
                });
            }

            if !params.iter().any(|p| p.default_value.is_some()) {
                continue;
            }

            let required = params.iter().filter(|p| p.default_value.is_none()).count();
            out.insert(fqn, DefaultArgFunInfo { required, params });
        }

        out
    }

    fn lower_item_into(
        &mut self,
        pkg_prefix: &str,
        item: &ast::Item,
        out: &mut Vec<Item>,
        decls: &mut Vec<Decl>,
    ) {
        match item {
            ast::Item::Fun(fun) => out.push(Item::Fun(self.lower_fun_decl(pkg_prefix, fun))),
            ast::Item::Val(v) if matches!(v.binding, ast::ValBinding::Pattern(_)) => {
                self.lower_top_level_pattern_val_items(pkg_prefix, v, out);
            }
            ast::Item::Val(v) => out.push(Item::Val(self.lower_val_decl(
                pkg_prefix,
                v,
                ValScope::TopLevel,
            ))),
            ast::Item::TypeAlias(ta) => {
                decls.push(Decl::TypeAlias(self.lower_typealias_decl(pkg_prefix, ta)));
            }
            ast::Item::ComptimeIf(ci) => {
                self.record_stage_error(
                    ci.span,
                    "untrimmed package-level comptime if cannot enter HIR lowering",
                    "top-level item",
                );
            }
            ast::Item::Type(ty) => {
                decls.push(Decl::Nominal(self.lower_nominal_decl(pkg_prefix, ty)));
            }
            ast::Item::Object(obj) => {
                if let Some(decl) = self.lower_object_decl(pkg_prefix, obj) {
                    decls.push(Decl::Object(decl));
                }
            }
            ast::Item::ExtensionProperty(p) => {
                decls.push(Decl::ExtensionProperty(
                    self.lower_extension_property_decl(pkg_prefix, p),
                ));
                if let Some(getter) = self.lower_extension_property(pkg_prefix, p) {
                    out.push(getter);
                }
            }
        }
    }

    /// Collect member functions and value-type computed getters into the callable side table.
    fn collect_member_funs(&mut self, pkg_prefix: &str) -> Vec<FunDecl> {
        let mut out: Vec<FunDecl> = Vec::new();

        for item in &self.file.items {
            match item {
                ast::Item::Type(ty) => {
                    self.collect_member_funs_in_type_decl(pkg_prefix, ty, pkg_prefix, &mut out);
                }
                ast::Item::Object(obj) => {
                    self.collect_member_funs_in_object_decl(pkg_prefix, obj, pkg_prefix, &mut out);
                }
                ast::Item::Fun(_)
                | ast::Item::Val(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::TypeAlias(_)
                | ast::Item::ComptimeIf(_) => {}
            }
        }

        out
    }

    fn collect_member_funs_in_type_decl(
        &mut self,
        pkg_prefix: &str,
        decl: &ast::TypeDecl,
        prefix: &str,
        out: &mut Vec<FunDecl>,
    ) {
        let local_name = decl.name.text(self.source);
        let owner_fqn = join_prefix(prefix, local_name);

        let Some(body) = &decl.body else {
            return;
        };

        for member in &body.members {
            match member {
                ast::TypeMember::Property(prop)
                    if matches!(decl.kind, ast::TypeKind::Struct | ast::TypeKind::Enum)
                        && prop.getter.is_some() =>
                {
                    out.push(self.lower_value_property_getter_decl(
                        pkg_prefix,
                        &owner_fqn,
                        &decl.type_params,
                        decl.name.span,
                        prop,
                    ));
                }
                ast::TypeMember::Fun(fun) => {
                    out.push(self.lower_member_fun_decl(
                        pkg_prefix,
                        &owner_fqn,
                        &decl.type_params,
                        decl.name.span,
                        fun,
                    ));
                }
                ast::TypeMember::Type(nested) => {
                    self.collect_member_funs_in_type_decl(pkg_prefix, nested, &owner_fqn, out);
                }
                ast::TypeMember::Object(obj) => {
                    self.collect_member_funs_in_object_decl(pkg_prefix, obj, &owner_fqn, out);
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::InitBlock(_)
                | ast::TypeMember::SecondaryCtor(_) => {}
                ast::TypeMember::Property(_) => {}
            }
        }
    }

    fn collect_member_funs_in_object_decl(
        &mut self,
        pkg_prefix: &str,
        obj: &ast::ObjectDecl,
        prefix: &str,
        out: &mut Vec<FunDecl>,
    ) {
        let Some(name) = object_decl_name(self.source, obj) else {
            return;
        };
        let owner_fqn = join_prefix(prefix, &name);

        let this_decl_span = obj.name.as_ref().map(|n| n.span).unwrap_or(obj.span);

        let Some(body) = &obj.body else {
            return;
        };
        for member in &body.members {
            match member {
                ast::TypeMember::Fun(fun) => {
                    out.push(self.lower_member_fun_decl(
                        pkg_prefix,
                        &owner_fqn,
                        &[],
                        this_decl_span,
                        fun,
                    ));
                }
                ast::TypeMember::Type(nested) => {
                    self.collect_member_funs_in_type_decl(pkg_prefix, nested, &owner_fqn, out);
                }
                ast::TypeMember::Object(nested) => {
                    self.collect_member_funs_in_object_decl(pkg_prefix, nested, &owner_fqn, out);
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Property(_)
                | ast::TypeMember::InitBlock(_)
                | ast::TypeMember::SecondaryCtor(_) => {}
            }
        }
    }

    fn lower_fun_decl(&mut self, pkg_prefix: &str, fun: &ast::FunDecl) -> FunDecl {
        // 进入函数作用域：先把 type params lower 成 `TypeId`，保证签名与 body 内引用一致。
        self.push_type_params(&fun.type_params);
        let eff_binding_pushed = self.push_missing_fun_effect_placeholder(fun.eff_param.as_ref());

        let name = fun.name.text(self.source).to_string();
        let fqn = if pkg_prefix.is_empty() {
            name.clone()
        } else {
            format!("{pkg_prefix}.{name}")
        };

        let receiver_ty = fun.receiver.as_ref().map(|t| self.lower_type_ref(t));

        // 扩展函数的 receiver（`fun T.f(...)`）在 resolver 中会把 `this` 解析为一个局部绑定，
        // 且 decl_span 取 receiver type 的 span（见 `resolve::scopes` 中 `ThisContext.decl_span`）。
        // 为了让 codegen 能把 `this` 当作一个普通局部参数处理，这里把 receiver 显式降为第 0 个参数。
        let mut params: Vec<Param> =
            Vec::with_capacity(fun.params.len() + receiver_ty.is_some() as usize);
        if let Some(receiver) = fun.receiver.as_ref() {
            let span = receiver.span();
            let id = self.intern_local_symbol(span, false);
            let ty = receiver_ty.unwrap_or(self.builtins.any);
            params.push(Param {
                span,
                id,
                name: "this".to_string(),
                ty,
            });
        }

        for p in &fun.params {
            let name = p.name.text(self.source).to_string();
            let id = self.intern_local_symbol(p.name.span, false);
            let elem_ty =
                p.ty.as_ref()
                    .map(|t| self.lower_type_ref(t))
                    .unwrap_or(self.builtins.any);
            // T0113: vararg param type `T` → `Array<T>` (the function body uses it as an array).
            let ty = if p.is_vararg {
                self.types
                    .intern(crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Nominal(
                        crate::ty::NominalType {
                            fqn: "scoop.core.Array".to_string(),
                            args: vec![elem_ty],
                            eff: None,
                        },
                    )))
            } else {
                elem_ty
            };
            params.push(Param {
                span: p.name.span,
                id,
                name,
                ty,
            });
        }

        let is_const_fun = fun.modifiers.contains(&ast::Modifier::Const);
        let return_ty = fun
            .return_ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .or_else(|| self.typechecked_fun_return_ty(fun.name.span))
            .unwrap_or(self.builtins.any);

        let effects = self.lower_effect_row_expr(fun.effects.as_ref());
        // receiver 已作为显式参数降入 `params`，因此 HIR 的 function type 不再单独保留 receiver 位。
        let ty = self.types.ty_function(
            None,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            effects,
            fun.effects.as_ref().is_some_and(|r| r.closed),
        );

        let body_expected = self.expected_expr_for_param_ty(return_ty);
        let body = match &fun.body {
            ast::FunBody::Block(b) => {
                Some(self.lower_block_with_expected(pkg_prefix, b, body_expected))
            }
            ast::FunBody::Missing => None,
        };

        if eff_binding_pushed {
            self.pop_effect_row_param_binding();
        }
        self.pop_type_params();

        FunDecl {
            span: fun.span,
            fqn,
            name,
            source_path: self.source.path().to_path_buf(),
            is_const: is_const_fun,
            ty,
            params,
            return_ty,
            body,
        }
    }

    /// T0112: Synthesize an extension property's getter as a top-level function.
    ///
    /// The getter function has:
    /// - FQN = property FQN (e.g., `pkg.lastIndex`)
    /// - params = `[this: ReceiverType]`
    /// - return type = declared property type
    /// - body = getter body
    fn lower_extension_property(
        &mut self,
        pkg_prefix: &str,
        prop: &ast::ExtensionPropertyDecl,
    ) -> Option<Item> {
        let Some(getter) = &prop.getter else {
            return None;
        };

        self.push_type_params(&prop.type_params);

        let name = self.source.slice(prop.name.span).to_string();
        let fqn = if pkg_prefix.is_empty() {
            name.clone()
        } else {
            format!("{pkg_prefix}.{name}")
        };

        let receiver_ty = self.lower_type_ref(&prop.receiver);

        // Receiver as first parameter (same as extension functions).
        let receiver_span = prop.receiver.span();
        let receiver_id = self.intern_local_symbol(receiver_span, false);
        let params = vec![Param {
            span: receiver_span,
            id: receiver_id,
            name: "this".to_string(),
            ty: receiver_ty,
        }];

        let return_ty = prop
            .ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .unwrap_or(self.builtins.any);

        let effects = EffectRow::pure();
        let ty = self.types.ty_function(
            None,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            effects,
            false,
        );

        let body_expected = self.expected_expr_for_param_ty(return_ty);

        // Lower getter body.
        let body = match &getter.body {
            ast::AccessorBody::Block(b) => {
                Some(self.lower_block_with_expected(pkg_prefix, b, body_expected))
            }
            ast::AccessorBody::Expr(e) => {
                // `get() = expr` → synthesize a block with the expression as tail stmt.
                let lowered_expr = self.lower_expr_with_expected(pkg_prefix, e, body_expected);
                let expr_ty = lowered_expr.ty;
                Some(Block {
                    span: e.span,
                    ty: expr_ty,
                    stmts: vec![Stmt {
                        span: e.span,
                        ty: expr_ty,
                        kind: StmtKind::Expr(lowered_expr),
                    }],
                })
            }
            ast::AccessorBody::Missing => None,
        };

        self.pop_type_params();

        Some(Item::Fun(FunDecl {
            span: prop.span,
            fqn,
            name,
            source_path: self.source.path().to_path_buf(),
            is_const: false,
            ty,
            params,
            return_ty,
            body,
        }))
    }

    /// 降低一个 class/object 的 member `fun` 为”顶层函数形态”（显式 `this` 参数）。
    ///
    /// 约定：
    /// - `this_decl_span` 必须与 resolver 为 `this` 写回的 `ResolvedValueRef::Local { decl_span }` 对齐：
    ///   - class member：`decl.name.span`
    ///   - object member：`obj.name.span`（匿名 companion 用 `obj.span`）
    fn lower_member_fun_decl(
        &mut self,
        pkg_prefix: &str,
        owner_fqn: &str,
        owner_type_params: &[ast::TypeParam],
        this_decl_span: Span,
        fun: &ast::FunDecl,
    ) -> FunDecl {
        // owner type params 在 member 方法体内可见（例如 `class Box<T> { fun get(): T }`）。
        self.push_type_params(owner_type_params);
        self.push_type_params(&fun.type_params);
        let eff_binding_pushed = self.push_missing_fun_effect_placeholder(fun.eff_param.as_ref());

        let name = fun.name.text(self.source).to_string();
        let fqn = format!("{owner_fqn}.{name}");

        let this_id = self.intern_local_symbol(this_decl_span, false);
        let this_args: Vec<TypeId> = owner_type_params
            .iter()
            .filter_map(|p| self.lookup_type_param(p.name.text(self.source)))
            .collect();
        let this_ty = self.intern_nominal(owner_fqn.to_string(), this_args, None);

        let mut params: Vec<Param> = Vec::with_capacity(fun.params.len() + 1);
        params.push(Param {
            span: this_decl_span,
            id: this_id,
            name: "this".to_string(),
            ty: this_ty,
        });

        for p in &fun.params {
            let name = p.name.text(self.source).to_string();
            let id = self.intern_local_symbol(p.name.span, false);
            let ty =
                p.ty.as_ref()
                    .map(|t| self.lower_type_ref(t))
                    .unwrap_or(self.builtins.any);
            params.push(Param {
                span: p.name.span,
                id,
                name,
                ty,
            });
        }

        let is_const_fun = fun.modifiers.contains(&ast::Modifier::Const);
        let return_ty = fun
            .return_ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .or_else(|| self.typechecked_fun_return_ty(fun.name.span))
            .unwrap_or(self.builtins.any);

        let effects = self.lower_effect_row_expr(fun.effects.as_ref());
        let ty = self.types.ty_function(
            None,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            effects,
            fun.effects.as_ref().is_some_and(|r| r.closed),
        );

        let body_expected = self.expected_expr_for_param_ty(return_ty);
        let body = match &fun.body {
            ast::FunBody::Block(b) => {
                Some(self.lower_block_with_expected(pkg_prefix, b, body_expected))
            }
            ast::FunBody::Missing => None,
        };

        if eff_binding_pushed {
            self.pop_effect_row_param_binding();
        }
        self.pop_type_params(); // fun type params
        self.pop_type_params(); // owner type params

        FunDecl {
            span: fun.span,
            fqn,
            name,
            source_path: self.source.path().to_path_buf(),
            is_const: is_const_fun,
            ty,
            params,
            return_ty,
            body,
        }
    }

    /// 将值类型（struct/enum）的 getter-only computed property 降低为“顶层函数形态”。
    ///
    /// 约定：
    /// - FQN 直接复用属性 FQN（例如 `pkg.Point.doubled`）；
    /// - 第 0 个参数为显式 `this`；
    /// - body 直接来自 accessor getter body。
    fn lower_value_property_getter_decl(
        &mut self,
        pkg_prefix: &str,
        owner_fqn: &str,
        owner_type_params: &[ast::TypeParam],
        this_decl_span: Span,
        prop: &ast::PropertyDecl,
    ) -> FunDecl {
        let getter = prop.getter.as_ref().expect(
            "computed property getter collection only calls this helper for getter-only properties",
        );

        self.push_type_params(owner_type_params);

        let name = prop.name.text(self.source).to_string();
        let fqn = format!("{owner_fqn}.{name}");

        let this_id = self.intern_local_symbol(this_decl_span, false);
        let this_args: Vec<TypeId> = owner_type_params
            .iter()
            .filter_map(|p| self.lookup_type_param(p.name.text(self.source)))
            .collect();
        let this_ty = self.intern_nominal(owner_fqn.to_string(), this_args, None);
        let params = vec![Param {
            span: this_decl_span,
            id: this_id,
            name: "this".to_string(),
            ty: this_ty,
        }];

        let return_ty = prop
            .ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .unwrap_or(self.builtins.any);

        let ty = self.types.ty_function(
            None,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            EffectRow::pure(),
            false,
        );

        let body_expected = self.expected_expr_for_param_ty(return_ty);
        let body = match &getter.body {
            ast::AccessorBody::Block(b) => {
                Some(self.lower_block_with_expected(pkg_prefix, b, body_expected))
            }
            ast::AccessorBody::Expr(e) => {
                let lowered_expr = self.lower_expr_with_expected(pkg_prefix, e, body_expected);
                let expr_ty = lowered_expr.ty;
                Some(Block {
                    span: e.span,
                    ty: expr_ty,
                    stmts: vec![Stmt {
                        span: e.span,
                        ty: expr_ty,
                        kind: StmtKind::Expr(lowered_expr),
                    }],
                })
            }
            ast::AccessorBody::Missing => None,
        };

        self.pop_type_params();

        FunDecl {
            span: prop.span,
            fqn,
            name,
            source_path: self.source.path().to_path_buf(),
            is_const: false,
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
        let eff_binding_pushed = self.push_missing_fun_effect_placeholder(fun.eff_param.as_ref());
        let name = fun.name.text(self.source).to_string();
        let fqn = if pkg_prefix.is_empty() {
            name.clone()
        } else {
            format!("{pkg_prefix}.{name}")
        };

        let receiver_ty = fun.receiver.as_ref().map(|t| self.lower_type_ref(t));

        let mut params: Vec<Param> =
            Vec::with_capacity(fun.params.len() + receiver_ty.is_some() as usize);
        if let Some(receiver) = fun.receiver.as_ref() {
            let span = receiver.span();
            let id = self.intern_local_symbol(span, false);
            let ty = receiver_ty.unwrap_or(self.builtins.any);
            params.push(Param {
                span,
                id,
                name: "this".to_string(),
                ty,
            });
        }

        for p in &fun.params {
            let name = p.name.text(self.source).to_string();
            let id = self.intern_local_symbol(p.name.span, false);
            let elem_ty =
                p.ty.as_ref()
                    .map(|t| self.lower_type_ref(t))
                    .unwrap_or(self.builtins.any);
            // T0113: vararg param type `T` → `Array<T>` (the function body uses it as an array).
            let ty = if p.is_vararg {
                self.types
                    .intern(crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Nominal(
                        crate::ty::NominalType {
                            fqn: "scoop.core.Array".to_string(),
                            args: vec![elem_ty],
                            eff: None,
                        },
                    )))
            } else {
                elem_ty
            };
            params.push(Param {
                span: p.name.span,
                id,
                name,
                ty,
            });
        }

        let is_const_fun = fun.modifiers.contains(&ast::Modifier::Const);
        let return_ty = fun
            .return_ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .or_else(|| self.typechecked_fun_return_ty(fun.name.span))
            .unwrap_or(self.builtins.any);

        let effects = self.lower_effect_row_expr(fun.effects.as_ref());
        let ty = self.types.ty_function(
            None,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            effects,
            fun.effects.as_ref().is_some_and(|r| r.closed),
        );

        let body_expected = self.expected_expr_for_param_ty(return_ty);
        let body = match &fun.body {
            ast::FunBody::Block(b) => {
                Some(self.lower_block_with_expected(pkg_prefix, b, body_expected))
            }
            ast::FunBody::Missing => None,
        };

        let out = FunDecl {
            span: fun.span,
            fqn,
            name,
            source_path: self.source.path().to_path_buf(),
            is_const: is_const_fun,
            ty,
            params,
            return_ty,
            body,
        };
        if eff_binding_pushed {
            self.pop_effect_row_param_binding();
        }
        out
    }

    /// T0126: 在"已绑定 owner type params"的语境下降低成员方法。
    ///
    /// 与 `lower_member_fun_decl` 的区别：owner type params 已经通过 `push_type_param_bindings`
    /// 预绑定到具体类型（由调用方在调用前完成），因此 `this` 参数和方法体中的 owner type params
    /// 均会直接解析为具体 TypeId。
    ///
    /// `this_concrete_args` 是 owner 的具体类型实参（例如 `[Int]` for `Box<Int>`），用于
    /// 构造 `this` 参数的精确 nominal 类型（codegen 阶段需要从 `this.hir_ty` 提取 type args
    /// 来查找 class field 布局）。
    fn lower_member_fun_decl_with_bound_type_params(
        &mut self,
        pkg_prefix: &str,
        owner_fqn: &str,
        this_decl_span: Span,
        this_concrete_args: &[TypeId],
        fun: &ast::FunDecl,
    ) -> FunDecl {
        // 方法自身的 type params（如果有的话）仍然需要 push；
        // owner 的 type params 已由调用方在 push_type_param_bindings 中绑定。
        let fun_type_params_pushed = self.push_missing_type_params(&fun.type_params);
        let eff_binding_pushed = self.push_missing_fun_effect_placeholder(fun.eff_param.as_ref());

        let name = fun.name.text(self.source).to_string();
        let fqn = format!("{owner_fqn}.{name}");

        let this_id = self.intern_local_symbol(this_decl_span, false);
        let this_ty = self.intern_nominal(owner_fqn.to_string(), this_concrete_args.to_vec(), None);

        let mut params: Vec<Param> = Vec::with_capacity(fun.params.len() + 1);
        params.push(Param {
            span: this_decl_span,
            id: this_id,
            name: "this".to_string(),
            ty: this_ty,
        });

        for p in &fun.params {
            let name = p.name.text(self.source).to_string();
            let id = self.intern_local_symbol(p.name.span, false);
            let ty =
                p.ty.as_ref()
                    .map(|t| self.lower_type_ref(t))
                    .unwrap_or(self.builtins.any);
            params.push(Param {
                span: p.name.span,
                id,
                name,
                ty,
            });
        }

        let is_const_fun = fun.modifiers.contains(&ast::Modifier::Const);
        let return_ty = fun
            .return_ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .or_else(|| self.typechecked_fun_return_ty(fun.name.span))
            .unwrap_or(self.builtins.any);

        let effects = self.lower_effect_row_expr(fun.effects.as_ref());
        let ty = self.types.ty_function(
            None,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            effects,
            fun.effects.as_ref().is_some_and(|r| r.closed),
        );

        let body = match &fun.body {
            ast::FunBody::Block(b) => Some(self.lower_block(pkg_prefix, b)),
            ast::FunBody::Missing => None,
        };

        if eff_binding_pushed {
            self.pop_effect_row_param_binding();
        }
        if fun_type_params_pushed {
            self.pop_type_params(); // fun type params
        }

        FunDecl {
            span: fun.span,
            fqn,
            name,
            source_path: self.source.path().to_path_buf(),
            is_const: is_const_fun,
            ty,
            params,
            return_ty,
            body,
        }
    }

    /// 将值类型 computed property getter 在“已绑定 owner type params”的语境下降低为 HIR。
    fn lower_value_property_getter_decl_with_bound_type_params(
        &mut self,
        pkg_prefix: &str,
        owner_fqn: &str,
        this_decl_span: Span,
        this_concrete_args: &[TypeId],
        prop: &ast::PropertyDecl,
    ) -> FunDecl {
        let getter = prop.getter.as_ref().expect(
            "computed property getter collection only calls this helper for getter-only properties",
        );

        let name = prop.name.text(self.source).to_string();
        let fqn = format!("{owner_fqn}.{name}");

        let this_id = self.intern_local_symbol(this_decl_span, false);
        let this_ty = self.intern_nominal(owner_fqn.to_string(), this_concrete_args.to_vec(), None);
        let params = vec![Param {
            span: this_decl_span,
            id: this_id,
            name: "this".to_string(),
            ty: this_ty,
        }];

        let return_ty = prop
            .ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .unwrap_or(self.builtins.any);

        let ty = self.types.ty_function(
            None,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            EffectRow::pure(),
            false,
        );

        let body_expected = self.expected_expr_for_param_ty(return_ty);
        let body = match &getter.body {
            ast::AccessorBody::Block(b) => {
                Some(self.lower_block_with_expected(pkg_prefix, b, body_expected))
            }
            ast::AccessorBody::Expr(e) => {
                let lowered_expr = self.lower_expr_with_expected(pkg_prefix, e, body_expected);
                let expr_ty = lowered_expr.ty;
                Some(Block {
                    span: e.span,
                    ty: expr_ty,
                    stmts: vec![Stmt {
                        span: e.span,
                        ty: expr_ty,
                        kind: StmtKind::Expr(lowered_expr),
                    }],
                })
            }
            ast::AccessorBody::Missing => None,
        };

        FunDecl {
            span: prop.span,
            fqn,
            name,
            source_path: self.source.path().to_path_buf(),
            is_const: false,
            ty,
            params,
            return_ty,
            body,
        }
    }

    fn call_top_level_fun(
        &mut self,
        span: Span,
        fqn: &str,
        args: Vec<Expr>,
        ret_ty: TypeId,
    ) -> Expr {
        let call_span = self.fresh_synthetic_call_site_span(span);
        let fqn = fqn.to_string();
        let callee = Expr {
            span: call_span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn,
            }),
        };

        Expr {
            span: call_span,
            ty: ret_ty,
            kind: ExprKind::Call {
                callee: Box::new(callee),
                args: args.into_iter().map(CallArg::Positional).collect(),
            },
        }
    }

    fn lower_val_decl(&mut self, pkg_prefix: &str, v: &ast::ValDecl, scope: ValScope) -> ValDecl {
        // T0124: lower the declared type first so we can pass it as expected type for struct literals.
        let declared_ty_early = v.ty.as_ref().map(|t| self.lower_type_ref(t));
        let typechecked_init_ty = v
            .init
            .as_ref()
            .and_then(|init| self.typechecked_expr_ty(init.span));

        // T1317c：数组字面量 `[...]` 的 lowering 依赖”期望的容器类型”（Array vs MutableArray）。
        // 这里从显式的类型注解（若存在）向 initializer 传播该 hint。
        let init_expected = ExpectedExpr {
            value_ty: declared_ty_early,
            array_lit_target: v
                .ty
                .as_ref()
                .and_then(|ty| self.array_lit_target_from_type_ref(ty)),
            array_lit_ty: declared_ty_early,
            struct_lit_ty: declared_ty_early,
        };
        let init = v
            .init
            .as_ref()
            .map(|e| self.lower_expr_with_expected(pkg_prefix, e, init_expected));

        let ty = declared_ty_early
            .or(typechecked_init_ty)
            .or_else(|| init.as_ref().map(|e| e.ty))
            .unwrap_or(self.builtins.any);

        let mut top_level_fqn: Option<String> = None;
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
                        top_level_fqn = Some(fqn.clone());
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

        if scope == ValScope::TopLevel
            && let Some(fqn) = top_level_fqn.as_ref()
        {
            let extern_symbol = name
                .as_deref()
                .and_then(|default_name| self.extern_global_symbol(v, default_name));
            if let Some(symbol) = extern_symbol.clone() {
                self.extern_globals.insert(
                    fqn.clone(),
                    super::ExternGlobal {
                        fqn: fqn.clone(),
                        source_path: self.source.path().to_path_buf(),
                        span: v.span,
                        ty,
                        mutable: v.kind == ast::ValKind::Var,
                        symbol,
                        linkage: super::ExternGlobalLinkage::External,
                        storage: self
                            .top_level_var_storage_from_annotations(v)
                            .unwrap_or(TopLevelVarStorage::Global),
                        initializer_absent: v.init.is_none(),
                        unsafe_required: true,
                    },
                );
            } else if v.modifiers.contains(&ast::Modifier::Const) && v.kind == ast::ValKind::Val {
                self.top_level_consts.insert(
                    fqn.clone(),
                    super::TopLevelConst {
                        fqn: fqn.clone(),
                        source_path: self.source.path().to_path_buf(),
                        span: v.span,
                        ty,
                        init: init.clone(),
                    },
                );
            } else if v.kind == ast::ValKind::Val {
                self.top_level_immutable_values.insert(
                    fqn.clone(),
                    super::TopLevelImmutableValue {
                        fqn: fqn.clone(),
                        source_path: self.source.path().to_path_buf(),
                        span: v.span,
                        ty,
                        init: init.clone(),
                    },
                );
            }

            // T1023：顶层 `@ThreadLocal/@Global var` 需要后端生成静态存储。
            if extern_symbol.is_none()
                && v.kind == ast::ValKind::Var
                && let Some(storage) = self.top_level_var_storage_from_annotations(v)
            {
                self.top_level_vars.insert(
                    fqn.clone(),
                    super::TopLevelVar {
                        fqn: fqn.clone(),
                        source_path: self.source.path().to_path_buf(),
                        span: v.span,
                        storage,
                        ty,
                        init: init.clone(),
                    },
                );
            }
        }

        ValDecl {
            span: v.span,
            id,
            name,
            mutable: v.kind == ast::ValKind::Var,
            ty,
            init,
        }
    }

    fn extern_global_symbol(&self, v: &ast::ValDecl, default_name: &str) -> Option<String> {
        v.annotations
            .iter()
            .find_map(|ann| extern_annotation_symbol(self.source, ann, default_name))
    }

    fn top_level_var_storage_from_annotations(
        &self,
        v: &ast::ValDecl,
    ) -> Option<TopLevelVarStorage> {
        const THREAD_LOCAL_FQN: &str = "scoop.core.ThreadLocal";
        const GLOBAL_FQN: &str = "scoop.core.Global";

        if v.annotations
            .iter()
            .any(|ann| self.annotation_use_resolves_to_fqn(ann, THREAD_LOCAL_FQN))
        {
            Some(TopLevelVarStorage::ThreadLocal)
        } else if v
            .annotations
            .iter()
            .any(|ann| self.annotation_use_resolves_to_fqn(ann, GLOBAL_FQN))
        {
            Some(TopLevelVarStorage::Global)
        } else {
            None
        }
    }

    fn annotation_use_resolves_to_fqn(&self, ann: &ast::AnnotationUse, expected_fqn: &str) -> bool {
        // 与 typecheck 保持一致：复用 Index 的 import/package 解析逻辑，避免仅按未限定名匹配导致误判。
        let ty = ast::TypeRef::Path(ast::TypePath {
            span: ann.span,
            segments: ann.path.clone(),
            args: Vec::new(),
        });

        matches!(
            self.index.type_ref_to_fqn_in_file(self.source, self.file, &ty),
            Some(fqn) if fqn == expected_fqn
        )
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
                self.types.ty_function(
                    receiver,
                    params,
                    return_ty,
                    effects,
                    fun.effects.as_ref().is_some_and(|r| r.closed),
                )
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
            "scoop.core.Char" => return self.builtins.char_,
            "scoop.core.Float64" => return self.builtins.float64,
            "scoop.core.Float32" => return self.builtins.float32,
            "scoop.core.Int" => return self.builtins.int,
            // T1027：internal atomics（`__AtomicInt`）——与 `Int` 相同布局的内部原子整型。
            "scoop.unsafe.__AtomicInt" => return self.builtins.int,
            "scoop.core.UInt" => return self.builtins.uint,
            "scoop.core.UIntPtr" => return self.builtins.uint,
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
            if term.segments.len() == 1 && term.args.is_empty() {
                let name = term.segments[0].text(self.source);
                if let Some(binding) = self
                    .effect_row_param_scopes
                    .iter()
                    .rev()
                    .find_map(|scope| scope.get(name))
                {
                    match binding {
                        EffectRowParamBinding::Placeholder(marker) => terms.push(*marker),
                        EffectRowParamBinding::Concrete(row) => {
                            terms.extend(row.terms.iter().copied())
                        }
                    }
                    continue;
                }
            }
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

    fn type_param_is_bound(&self, name: &str) -> bool {
        self.type_param_scopes
            .iter()
            .rev()
            .any(|scope| scope.contains_key(name))
    }

    fn push_missing_type_params(&mut self, params: &[ast::TypeParam]) -> bool {
        let missing = params
            .iter()
            .filter(|param| !self.type_param_is_bound(param.name.text(self.source)))
            .cloned()
            .collect::<Vec<_>>();
        if missing.is_empty() {
            return false;
        }
        self.push_type_params(&missing);
        true
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

    fn decl_ast_context(
        &self,
        decl_file: &std::path::Path,
    ) -> Option<(&'a SourceFile, &'a ast::File)> {
        self.compilation_unit
            .iter()
            .copied()
            .find(|(source, _)| source.path() == decl_file)
    }
}

fn collect_default_arg_structs(
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> HashMap<String, DefaultArgStructInfo> {
    let mut out: HashMap<String, DefaultArgStructInfo> = HashMap::new();

    for (source, file) in compilation_unit {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            let ast::Item::Type(ty) = item else {
                continue;
            };
            collect_default_arg_structs_in_type_decl(source, ty, &pkg_prefix, &mut out);
        }
    }

    out
}

fn collect_value_type_computed_property_fqns(
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> HashSet<String> {
    let mut out: HashSet<String> = HashSet::new();

    for (source, file) in compilation_unit {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            match item {
                ast::Item::Type(ty) => {
                    collect_value_type_computed_property_fqns_in_type_decl(
                        source,
                        ty,
                        &pkg_prefix,
                        &mut out,
                    );
                }
                ast::Item::Object(obj) => {
                    collect_value_type_computed_property_fqns_in_object_decl(
                        source,
                        obj,
                        &pkg_prefix,
                        &mut out,
                    );
                }
                ast::Item::Fun(_)
                | ast::Item::Val(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::TypeAlias(_)
                | ast::Item::ComptimeIf(_) => {}
            }
        }
    }

    out
}

fn collect_value_type_computed_property_fqns_in_type_decl(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    prefix: &str,
    out: &mut HashSet<String>,
) {
    let local_name = decl.name.text(source).to_string();
    let type_fqn = join_prefix(prefix, &local_name);

    if matches!(decl.kind, ast::TypeKind::Struct | ast::TypeKind::Enum)
        && let Some(body) = &decl.body
    {
        for member in &body.members {
            let ast::TypeMember::Property(prop) = member else {
                continue;
            };
            if prop.getter.is_some() {
                out.insert(format!("{}.{}", type_fqn, prop.name.text(source)));
            }
        }
    }

    let Some(body) = &decl.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_value_type_computed_property_fqns_in_type_decl(
                    source, nested, &type_fqn, out,
                );
            }
            ast::TypeMember::Object(obj) => {
                collect_value_type_computed_property_fqns_in_object_decl(
                    source, obj, &type_fqn, out,
                );
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

fn collect_value_type_computed_property_fqns_in_object_decl(
    source: &SourceFile,
    obj: &ast::ObjectDecl,
    prefix: &str,
    out: &mut HashSet<String>,
) {
    let Some(obj_name) = object_decl_name(source, obj) else {
        return;
    };
    let obj_fqn = join_prefix(prefix, &obj_name);

    let Some(body) = &obj.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_value_type_computed_property_fqns_in_type_decl(
                    source, nested, &obj_fqn, out,
                );
            }
            ast::TypeMember::Object(nested) => {
                collect_value_type_computed_property_fqns_in_object_decl(
                    source, nested, &obj_fqn, out,
                );
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

fn collect_default_arg_structs_in_type_decl(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    prefix: &str,
    out: &mut HashMap<String, DefaultArgStructInfo>,
) {
    let local_name = decl.name.text(source).to_string();
    let type_fqn = if prefix.is_empty() {
        local_name
    } else {
        format!("{prefix}.{local_name}")
    };

    if matches!(decl.kind, ast::TypeKind::Struct) {
        let mut params: Vec<DefaultArgParamInfo> = Vec::new();

        if let Some(primary_ctor) = &decl.primary_ctor {
            for p in &primary_ctor.params {
                params.push(DefaultArgParamInfo {
                    decl_span: p.name.span,
                    name: p.name.text(source).to_string(),
                    is_vararg: p.is_vararg,
                    ty_ref: p.ty.clone(),
                    default_value: p.default_value.clone(),
                });
            }
        }

        if let Some(body) = &decl.body {
            for member in &body.members {
                let ast::TypeMember::Property(p) = member else {
                    continue;
                };
                if !p.is_direct_field() {
                    continue;
                }
                params.push(DefaultArgParamInfo {
                    decl_span: p.name.span,
                    name: p.name.text(source).to_string(),
                    is_vararg: false,
                    ty_ref: p.ty.clone(),
                    default_value: p.init.clone(),
                });
            }
        }

        if params.iter().any(|p| p.default_value.is_some()) {
            out.insert(
                type_fqn.clone(),
                DefaultArgStructInfo {
                    decl_file: source.path().to_path_buf(),
                    type_params: decl
                        .type_params
                        .iter()
                        .map(|p| p.name.text(source).to_string())
                        .collect(),
                    params,
                },
            );
        }
    }

    let Some(body) = &decl.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_default_arg_structs_in_type_decl(source, nested, &type_fqn, out);
            }
            ast::TypeMember::Object(obj) => {
                collect_default_arg_structs_in_object_decl(source, obj, &type_fqn, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

fn collect_default_arg_structs_in_object_decl(
    source: &SourceFile,
    obj: &ast::ObjectDecl,
    prefix: &str,
    out: &mut HashMap<String, DefaultArgStructInfo>,
) {
    let Some(obj_name) = obj
        .name
        .as_ref()
        .map(|id| id.text(source).to_string())
        .or_else(|| match obj.kind {
            ast::ObjectKind::Companion => Some("Companion".to_string()),
            ast::ObjectKind::Object => None,
        })
    else {
        return;
    };

    let obj_fqn = if prefix.is_empty() {
        obj_name
    } else {
        format!("{prefix}.{obj_name}")
    };

    let Some(body) = &obj.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_default_arg_structs_in_type_decl(source, nested, &obj_fqn, out);
            }
            ast::TypeMember::Object(nested) => {
                collect_default_arg_structs_in_object_decl(source, nested, &obj_fqn, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

#[derive(Clone, Copy)]
struct CompilationUnitInitCollectionInputs<'a> {
    index: &'a Index,
    type_kinds: &'a HashMap<String, ast::TypeKind>,
    known_receiver_subclasses: &'a crate::devirtualize::KnownReceiverSubclassIndex,
    class_vtables: &'a crate::vtable::ClassVtableIndex,
    interfaces: &'a crate::itable::InterfaceIndex,
    class_itables: &'a crate::itable::ClassItableIndex,
    typecheck_types: Option<&'a TypeStore>,
    materialize_direct_call_targets: bool,
    devirtualize_dispatch_calls: bool,
    builtins: BuiltinTypes,
}

fn collect_compilation_unit_object_and_class_inits(
    compilation_unit: &[(&SourceFile, &ast::File)],
    inputs: CompilationUnitInitCollectionInputs<'_>,
    types: &mut TypeStore,
) -> Result<
    (
        ObjectInitIndex,
        ClassInitIndex,
        CtorCallSiteIndex,
        super::DispatchCallSiteIndex,
        WithUpdateSiteIndex,
        AssignPlaceSiteIndex,
    ),
    HirLowerError,
> {
    let CompilationUnitInitCollectionInputs {
        index,
        type_kinds,
        known_receiver_subclasses,
        class_vtables,
        interfaces,
        class_itables,
        typecheck_types,
        materialize_direct_call_targets,
        devirtualize_dispatch_calls,
        builtins,
    } = inputs;
    let mut object_inits = ObjectInitIndex::new();
    let mut class_inits = ClassInitIndex::new();
    let mut ctor_call_sites = CtorCallSiteIndex::new();
    let mut dispatch_call_sites = super::DispatchCallSiteIndex::new();
    let mut with_update_contracts = WithUpdateSiteIndex::new();
    let mut assign_place_contracts = AssignPlaceSiteIndex::new();

    for (source, file) in compilation_unit {
        let init_collection_cx = InitCollectionCx {
            source,
            file,
            index,
            type_kinds,
            known_receiver_subclasses,
            class_vtables,
            interfaces,
            class_itables,
            typecheck_types,
            builtins,
            materialize_direct_call_targets,
            devirtualize_dispatch_calls,
        };
        let (
            file_object_inits,
            file_object_ctor_call_sites,
            file_object_dispatch_call_sites,
            file_object_with_update_contracts,
            file_object_assign_place_contracts,
        ) = collect_object_inits(init_collection_cx, types)?;
        object_inits.extend(file_object_inits);
        ctor_call_sites.extend(file_object_ctor_call_sites);
        dispatch_call_sites.extend(file_object_dispatch_call_sites);
        with_update_contracts.extend(file_object_with_update_contracts);
        assign_place_contracts.extend(file_object_assign_place_contracts);

        let (
            file_class_inits,
            file_class_ctor_call_sites,
            file_class_dispatch_call_sites,
            file_class_with_update_contracts,
            file_class_assign_place_contracts,
        ) = collect_class_inits(init_collection_cx, types)?;
        class_inits.extend(file_class_inits);
        ctor_call_sites.extend(file_class_ctor_call_sites);
        dispatch_call_sites.extend(file_class_dispatch_call_sites);
        with_update_contracts.extend(file_class_with_update_contracts);
        assign_place_contracts.extend(file_class_assign_place_contracts);
    }

    Ok((
        object_inits,
        class_inits,
        ctor_call_sites,
        dispatch_call_sites,
        with_update_contracts,
        assign_place_contracts,
    ))
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
    {
        let sources = [source];
        let mut files = [&mut ast];
        crate::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            session.sysroot(),
            &sources,
            &mut files,
        )?;
    }

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
    let nominal_variances = collect_nominal_variances(&pairs);
    let direct_supertypes = collect_direct_supertypes(&pairs, &index);
    let known_receiver_subclasses =
        crate::devirtualize::collect_known_receiver_subclasses(&direct_supertypes);
    let delegated_properties = collect_delegated_properties(&pairs);
    let default_arg_structs = collect_default_arg_structs(&pairs);
    let value_type_computed_properties = collect_value_type_computed_property_fqns(&pairs);
    let class_vtables = crate::vtable::collect_class_vtables(&pairs, &index)?;
    let (interfaces, class_itables) =
        crate::itable::collect_interfaces_and_class_itables(&pairs, &index, &class_vtables)?;
    let continuation_resume_call_sites = ast
        .continuation_resume_call_sites()
        .into_iter()
        .map(|span| CallSite::new(source.path().to_path_buf(), span))
        .collect();
    let non_pure_continuation_resume_call_sites = ast
        .non_pure_continuation_resume_call_sites()
        .into_iter()
        .map(|span| CallSite::new(source.path().to_path_buf(), span))
        .collect();

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let stable_cone_key = StableConeKey::for_virtual_source_path(source.path());
    let generic_template_symbol_suffixes =
        util::collect_generic_template_symbol_suffixes_with_stable_cone_key(
            &stable_cone_key,
            &index,
            &pairs,
        );

    // 先降 HIR（保持 fixtures 中 `TypeId` 分配顺序稳定），再补充 struct 布局索引供后端使用。
    let pkg_prefix = package_prefix(source, ast.package.as_ref());
    let (
        file,
        member_funs,
        mut ctor_call_sites,
        mut dispatch_call_sites,
        effect_op_call_sites,
        handle_payload_tuple_tys,
        mut with_update_contracts,
        mut assign_place_contracts,
        top_level_vars,
        extern_globals,
        top_level_consts,
        top_level_immutable_values,
        when_pat_binding_tys,
    ) = {
        let mut ctx = HirLowering::new(
            source,
            &ast,
            &index,
            &mut types,
            HirLoweringSetup {
                typecheck_types: None,
                type_kinds: &type_kinds,
                delegated_properties: &delegated_properties,
                compilation_unit: &pairs,
                default_arg_structs: default_arg_structs.clone(),
                value_type_computed_properties: &value_type_computed_properties,
                builtins,
                generic_template_symbol_suffixes: &generic_template_symbol_suffixes,
                known_receiver_subclasses: &known_receiver_subclasses,
                class_vtables: &class_vtables,
                interfaces: &interfaces,
                class_itables: &class_itables,
                materialize_direct_call_targets: false,
                devirtualize_dispatch_calls: false,
                runtime_comptime_plan: None,
            },
        );
        let file = ctx.lower_file();
        if let Some(err) = ctx.take_stage_error() {
            return Err(err.into());
        }
        let member_funs = ctx.collect_member_funs(&pkg_prefix);
        if let Some(err) = ctx.take_stage_error() {
            return Err(err.into());
        }
        ctx.record_missing_assign_place_contracts_in_file(&file);
        ctx.record_missing_assign_place_contracts_in_funs(&member_funs);
        let ctor_call_sites = std::mem::take(&mut ctx.ctor_call_sites);
        let dispatch_call_sites = std::mem::take(&mut ctx.dispatch_call_sites);
        let effect_op_call_sites = std::mem::take(&mut ctx.effect_op_call_sites);
        let handle_payload_tuple_tys = std::mem::take(&mut ctx.handle_payload_tuple_tys);
        let with_update_contracts = std::mem::take(&mut ctx.with_update_contracts);
        let assign_place_contracts = std::mem::take(&mut ctx.assign_place_contracts);
        let top_level_vars = std::mem::take(&mut ctx.top_level_vars);
        let extern_globals = std::mem::take(&mut ctx.extern_globals);
        let top_level_consts = std::mem::take(&mut ctx.top_level_consts);
        let top_level_immutable_values = std::mem::take(&mut ctx.top_level_immutable_values);
        let when_pat_binding_tys = std::mem::take(&mut ctx.when_pat_binding_tys);
        (
            file,
            member_funs,
            ctor_call_sites,
            dispatch_call_sites,
            effect_op_call_sites,
            handle_payload_tuple_tys,
            with_update_contracts,
            assign_place_contracts,
            top_level_vars,
            extern_globals,
            top_level_consts,
            top_level_immutable_values,
            when_pat_binding_tys,
        )
    };

    // T4016T2：sysroot/task.scoop 这类“实现文件依赖同编译单元里的声明元数据”的路径，
    // 需要从整个 compilation unit 收集 object/class side tables，而不是只看当前 lowering 的文件。
    let (
        object_inits,
        class_inits,
        side_table_ctor_call_sites,
        side_table_dispatch_call_sites,
        side_table_with_update_contracts,
        side_table_assign_place_contracts,
    ) = collect_compilation_unit_object_and_class_inits(
        &pairs,
        CompilationUnitInitCollectionInputs {
            index: &index,
            type_kinds: &type_kinds,
            known_receiver_subclasses: &known_receiver_subclasses,
            class_vtables: &class_vtables,
            interfaces: &interfaces,
            class_itables: &class_itables,
            typecheck_types: None,
            materialize_direct_call_targets: false,
            devirtualize_dispatch_calls: false,
            builtins,
        },
        &mut types,
    )?;
    ctor_call_sites.extend(side_table_ctor_call_sites);
    dispatch_call_sites.extend(side_table_dispatch_call_sites);
    with_update_contracts.extend(side_table_with_update_contracts);
    assign_place_contracts.extend(side_table_assign_place_contracts);

    // T1006：收集 `@Extern` 外部函数的符号名与 ABI（side table；不影响 dump-hir 输出）。
    let extern_funs = collect_extern_funs(source, &ast);
    let extern_libs = collect_extern_libs(&pairs);

    // T0811：早期 LLVM codegen 需要知道 struct 的字段顺序与字段类型，用于生成字段 GEP 索引。
    let mut struct_layouts = collect_struct_layouts(&pairs, &index, &mut types);
    // T0813：早期 LLVM codegen 需要知道 enum 的 variant tag 与 payload 字段类型，用于生成判别与解构。
    let mut enum_layouts = collect_enum_layouts(&pairs, &index, &mut types);
    // T0124：泛型 struct/enum 的具体实例化布局。
    struct_layouts.extend(collect_generic_struct_instantiation_layouts(
        &pairs, &index, &mut types,
    ));
    enum_layouts.extend(collect_generic_enum_instantiation_layouts(
        &pairs, &index, &mut types,
    ));
    // T0125：泛型 class 的具体实例化 ClassInit。
    let class_inits = {
        let mut ci = class_inits;
        ci.extend(collect_generic_class_instantiation_inits(
            &pairs, &mut types, &ci,
        ));
        ci
    };
    // T0126：为泛型 class 的具体实例化生成单态化的成员方法 FunDecl。
    let monomorphized_member_funs = collect_generic_member_fun_instantiations(
        &pairs,
        &index,
        &type_kinds,
        None,
        &mut types,
        builtins,
    );
    let mut member_funs = member_funs;
    member_funs.extend(monomorphized_member_funs);
    struct_layouts.extend(collect_generic_struct_instantiation_layouts(
        &pairs, &index, &mut types,
    ));
    enum_layouts.extend(collect_generic_enum_instantiation_layouts(
        &pairs, &index, &mut types,
    ));
    let class_inits = {
        let mut ci = class_inits;
        ci.extend(collect_generic_class_instantiation_inits(
            &pairs, &mut types, &ci,
        ));
        ci
    };
    let top_level_fun_call_sites = collect_top_level_fun_call_sites(&[(source, &ast)]);
    let call_arg_bindings = collect_call_arg_bindings(&[(source, &ast)]);
    let stable_type_param_keys =
        collect_stable_type_param_keys(&[(source, &ast)], &stable_cone_key);
    Ok(LoweredHir {
        file,
        stable_cone_key,
        stable_type_param_keys,
        member_funs,
        materialized_mir: None,
        types,
        struct_layouts,
        enum_layouts,
        extern_funs,
        extern_globals,
        extern_libs,
        top_level_vars,
        top_level_consts,
        top_level_immutable_values,
        top_level_fun_call_sites,
        call_arg_bindings,
        with_update_contracts,
        assign_place_contracts,
        object_inits,
        class_inits,
        class_vtables,
        interfaces,
        class_itables,
        ctor_call_sites,
        dispatch_call_sites,
        effect_op_call_sites,
        handle_payload_tuple_tys,
        continuation_resume_call_sites,
        non_pure_continuation_resume_call_sites,
        when_pat_binding_tys,
        nominal_kinds: type_kinds,
        nominal_variances,
        direct_supertypes,
        builtins,
    })
}

/// 为需要 typed HIR 事实的调试入口生成 HIR。
///
/// 与 `lower_for_dump` 的区别：
/// - 这里会运行 typecheck（annotations / type refs / exprs）并消费 AST side tables；
/// - 用于 `dump-mir` / MIR fixtures 这类需要 `Continuation.resume`、late-bound member
///   resolution、effect payload binding 等 typed 事实的路径；
/// - 但仍停留在 generic HIR/MIR template 边界：不会在这里额外 materialize
///   standalone generic fun 或 owner-specialized member fun 的 `::<...>` 实例。
pub fn lower_typed_for_dump(
    session: &Session,
    source: &SourceFile,
) -> Result<LoweredHir, HirLowerError> {
    let mut ast = parse_file(source)?;
    {
        let sources = [source];
        let mut files = [&mut ast];
        crate::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            session.sysroot(),
            &sources,
            &mut files,
        )?;
    }

    crate::typecheck::check_file_headers(source, &ast)?;
    crate::typecheck::check_file_struct_decls(source, &ast)?;

    let index = {
        let mut compilation_unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for file in &session.sysroot().files {
            compilation_unit.push((&file.source, &file.ast));
        }
        compilation_unit.push((source, &ast));
        Index::build(&compilation_unit)?
    };

    let headers = crate::resolve::check_file_headers(source, &ast, &index)?;
    crate::resolve::check_file_bodies(source, &mut ast, &index, &headers)?;

    let mut env = crate::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index)?;
    env.extend_from_file(source, &ast, &index)?;

    let mut typecheck_types = TypeStore::new();
    let builtins = typecheck_types.intern_builtins();
    crate::typecheck::check_file_annotations(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut typecheck_types,
        builtins,
    )?;
    crate::typecheck::check_file_properties(source, &ast, &index, &env)?;
    crate::typecheck::check_file_type_refs(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut typecheck_types,
        builtins,
    )?;
    crate::typecheck::check_file_exprs(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut typecheck_types,
        builtins,
    )?;

    let mut compilation_unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for file in &session.sysroot().files {
        compilation_unit.push((&file.source, &file.ast));
    }
    compilation_unit.push((source, &ast));
    let files_to_lower = [(source, &ast)];
    let runtime_comptime_plan = crate::comptime::plan_runtime_comptime_in_file(
        source,
        &ast,
        &compilation_unit,
        &typecheck_types,
    )?;
    let mut runtime_comptime_plans = HashMap::new();
    if !runtime_comptime_plan.is_empty() {
        runtime_comptime_plans.insert(source.path().to_path_buf(), runtime_comptime_plan);
    }

    lower_for_compilation_unit_multi_files_internal(
        &index,
        &compilation_unit,
        &files_to_lower,
        &[],
        Some(&env),
        &typecheck_types,
        CompilationUnitLoweringOptions::generic_template_only(
            StableConeKey::for_virtual_source_path(source.path()),
        )
        .with_runtime_comptime_plans(&runtime_comptime_plans),
    )
}

pub(crate) fn generic_template_symbol_suffixes_for_compilation_unit(
    stable_cone_key: &StableConeKey,
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> util::GenericTemplateSymbolSuffixIndex {
    util::collect_generic_template_symbol_suffixes_with_stable_cone_key(
        stable_cone_key,
        index,
        compilation_unit,
    )
}

/// 在“给定编译单元（多个源文件）”的上下文中，为其中一个文件生成 HIR。
///
/// 用途：
/// - `.cone` 打包时导出 `api.scoopir`（TODO T1104）需要跨文件可见的类型 kind 信息；
/// - 后续多包编译/链接流程也会复用类似的“多文件 lowering”入口。
///
/// 约定：
/// - `file` 必须已经过 resolver（至少 `check_file_headers + check_file_bodies`），
///   以保证标识符绑定信息已写回 AST；
/// - `compilation_unit` 应包含 sysroot + 当前 cone 的全部源文件（稳定排序可保证输出更可回归）。
pub fn lower_for_compilation_unit(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> Result<LoweredHir, HirLowerError> {
    lower_for_compilation_unit_with_stable_cone_key(
        StableConeKey::for_virtual_source_path(source.path()),
        source,
        file,
        index,
        compilation_unit,
    )
}

pub fn lower_for_compilation_unit_with_stable_cone_key(
    stable_cone_key: StableConeKey,
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> Result<LoweredHir, HirLowerError> {
    let type_kinds = collect_type_decl_kinds(compilation_unit);
    let nominal_variances = collect_nominal_variances(compilation_unit);
    let direct_supertypes = collect_direct_supertypes(compilation_unit, index);
    let known_receiver_subclasses =
        crate::devirtualize::collect_known_receiver_subclasses(&direct_supertypes);
    let delegated_properties = collect_delegated_properties(compilation_unit);
    let default_arg_structs = collect_default_arg_structs(compilation_unit);
    let value_type_computed_properties =
        collect_value_type_computed_property_fqns(compilation_unit);
    let class_vtables = crate::vtable::collect_class_vtables(compilation_unit, index)?;
    let (interfaces, class_itables) = crate::itable::collect_interfaces_and_class_itables(
        compilation_unit,
        index,
        &class_vtables,
    )?;
    let continuation_resume_call_sites = file
        .continuation_resume_call_sites()
        .into_iter()
        .map(|span| CallSite::new(source.path().to_path_buf(), span))
        .collect();
    let non_pure_continuation_resume_call_sites = file
        .non_pure_continuation_resume_call_sites()
        .into_iter()
        .map(|span| CallSite::new(source.path().to_path_buf(), span))
        .collect();

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let generic_template_symbol_suffixes =
        util::collect_generic_template_symbol_suffixes_with_stable_cone_key(
            &stable_cone_key,
            index,
            compilation_unit,
        );

    // 先降 HIR（保持 `TypeId` 分配顺序稳定），再补充 side tables（layout/extern/object init）。
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let (
        file_hir,
        member_funs,
        mut ctor_call_sites,
        mut dispatch_call_sites,
        effect_op_call_sites,
        handle_payload_tuple_tys,
        mut with_update_contracts,
        mut assign_place_contracts,
        top_level_vars,
        extern_globals,
        top_level_consts,
        top_level_immutable_values,
        when_pat_binding_tys,
    ) = {
        let mut ctx = HirLowering::new(
            source,
            file,
            index,
            &mut types,
            HirLoweringSetup {
                typecheck_types: None,
                type_kinds: &type_kinds,
                delegated_properties: &delegated_properties,
                compilation_unit,
                default_arg_structs: default_arg_structs.clone(),
                value_type_computed_properties: &value_type_computed_properties,
                builtins,
                generic_template_symbol_suffixes: &generic_template_symbol_suffixes,
                known_receiver_subclasses: &known_receiver_subclasses,
                class_vtables: &class_vtables,
                interfaces: &interfaces,
                class_itables: &class_itables,
                materialize_direct_call_targets: true,
                devirtualize_dispatch_calls: false,
                runtime_comptime_plan: None,
            },
        );
        let file_hir = ctx.lower_file();
        if let Some(err) = ctx.take_stage_error() {
            return Err(err.into());
        }
        let member_funs = ctx.collect_member_funs(&pkg_prefix);
        if let Some(err) = ctx.take_stage_error() {
            return Err(err.into());
        }
        ctx.record_missing_assign_place_contracts_in_file(&file_hir);
        ctx.record_missing_assign_place_contracts_in_funs(&member_funs);
        let ctor_call_sites = std::mem::take(&mut ctx.ctor_call_sites);
        let dispatch_call_sites = std::mem::take(&mut ctx.dispatch_call_sites);
        let effect_op_call_sites = std::mem::take(&mut ctx.effect_op_call_sites);
        let handle_payload_tuple_tys = std::mem::take(&mut ctx.handle_payload_tuple_tys);
        let with_update_contracts = std::mem::take(&mut ctx.with_update_contracts);
        let assign_place_contracts = std::mem::take(&mut ctx.assign_place_contracts);
        let top_level_vars = std::mem::take(&mut ctx.top_level_vars);
        let extern_globals = std::mem::take(&mut ctx.extern_globals);
        let top_level_consts = std::mem::take(&mut ctx.top_level_consts);
        let top_level_immutable_values = std::mem::take(&mut ctx.top_level_immutable_values);
        let when_pat_binding_tys = std::mem::take(&mut ctx.when_pat_binding_tys);
        (
            file_hir,
            member_funs,
            ctor_call_sites,
            dispatch_call_sites,
            effect_op_call_sites,
            handle_payload_tuple_tys,
            with_update_contracts,
            assign_place_contracts,
            top_level_vars,
            extern_globals,
            top_level_consts,
            top_level_immutable_values,
            when_pat_binding_tys,
        )
    };

    let (
        object_inits,
        class_inits,
        side_table_ctor_call_sites,
        side_table_dispatch_call_sites,
        side_table_with_update_contracts,
        side_table_assign_place_contracts,
    ) = collect_compilation_unit_object_and_class_inits(
        compilation_unit,
        CompilationUnitInitCollectionInputs {
            index,
            type_kinds: &type_kinds,
            known_receiver_subclasses: &known_receiver_subclasses,
            class_vtables: &class_vtables,
            interfaces: &interfaces,
            class_itables: &class_itables,
            typecheck_types: None,
            materialize_direct_call_targets: true,
            devirtualize_dispatch_calls: false,
            builtins,
        },
        &mut types,
    )?;
    ctor_call_sites.extend(side_table_ctor_call_sites);
    dispatch_call_sites.extend(side_table_dispatch_call_sites);
    with_update_contracts.extend(side_table_with_update_contracts);
    assign_place_contracts.extend(side_table_assign_place_contracts);
    let extern_funs = collect_extern_funs(source, file);
    let extern_libs = collect_extern_libs(compilation_unit);
    let mut struct_layouts = collect_struct_layouts(compilation_unit, index, &mut types);
    let mut enum_layouts = collect_enum_layouts(compilation_unit, index, &mut types);
    // T0124：泛型 struct/enum 的具体实例化布局。
    struct_layouts.extend(collect_generic_struct_instantiation_layouts(
        compilation_unit,
        index,
        &mut types,
    ));
    enum_layouts.extend(collect_generic_enum_instantiation_layouts(
        compilation_unit,
        index,
        &mut types,
    ));
    // T0125：泛型 class 的具体实例化 ClassInit。
    let class_inits = {
        let mut ci = class_inits;
        ci.extend(collect_generic_class_instantiation_inits(
            compilation_unit,
            &mut types,
            &ci,
        ));
        ci
    };
    let top_level_fun_call_sites = collect_top_level_fun_call_sites(&[(source, file)]);
    let call_arg_bindings = collect_call_arg_bindings(&[(source, file)]);
    let stable_type_param_keys = collect_stable_type_param_keys(compilation_unit, &stable_cone_key);

    Ok(LoweredHir {
        file: file_hir,
        stable_cone_key,
        stable_type_param_keys,
        member_funs,
        materialized_mir: None,
        types,
        struct_layouts,
        enum_layouts,
        extern_funs,
        extern_globals,
        extern_libs,
        top_level_vars,
        top_level_consts,
        top_level_immutable_values,
        top_level_fun_call_sites,
        call_arg_bindings,
        with_update_contracts,
        assign_place_contracts,
        object_inits,
        class_inits,
        class_vtables,
        interfaces,
        class_itables,
        ctor_call_sites,
        dispatch_call_sites,
        effect_op_call_sites,
        handle_payload_tuple_tys,
        continuation_resume_call_sites,
        non_pure_continuation_resume_call_sites,
        when_pat_binding_tys,
        nominal_kinds: type_kinds,
        nominal_variances,
        direct_supertypes,
        builtins,
    })
}

/// 在”给定编译单元（多个源文件）”的上下文中，把多个文件一起 lowering 为一个 `LoweredHir`。
///
/// 用途（T1315a）：
/// - `scoop build/run` 注入 `stdlib/*.scoop` 后，需要让这些文件里的顶层函数在后端可见；
/// - T0150c：HIR 继续保留本地 span，但不再在 lowering 时 eager parse Int/String 字面量；
///   后续阶段通过“声明所属源文件 + 本地 span”回查原文。
pub fn lower_for_compilation_unit_multi_files(
    entry_source: &SourceFile,
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
    files_to_lower: &[(&SourceFile, &ast::File)],
    monomorph_keys: &[crate::monomorph::MonomorphKey],
    typecheck_types: &TypeStore,
) -> Result<LoweredHir, HirLowerError> {
    let stable_cone_key = virtual_stable_cone_key_for_sources(Some(entry_source), compilation_unit);
    lower_for_compilation_unit_multi_files_internal(
        index,
        compilation_unit,
        files_to_lower,
        monomorph_keys,
        None,
        typecheck_types,
        CompilationUnitLoweringOptions::direct_lowered_hir(stable_cone_key),
    )
}

pub fn lower_for_compilation_unit_multi_files_with_type_env(
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
    files_to_lower: &[(&SourceFile, &ast::File)],
    monomorph_keys: &[crate::monomorph::MonomorphKey],
    type_env: Option<&crate::typecheck::TypeEnv>,
    typecheck_types: &TypeStore,
) -> Result<LoweredHir, HirLowerError> {
    let stable_cone_key = virtual_stable_cone_key_for_sources(
        files_to_lower.first().map(|(source, _)| *source),
        compilation_unit,
    );
    lower_for_compilation_unit_multi_files_internal(
        index,
        compilation_unit,
        files_to_lower,
        monomorph_keys,
        type_env,
        typecheck_types,
        CompilationUnitLoweringOptions::direct_lowered_hir(stable_cone_key),
    )
}

/// 为 build / single-file LLVM frontend 生成“由 MIR instance collection 决定实例集合”的 HIR 兼容输入。
///
/// 说明：
/// - 该入口先复用 typechecked compilation-unit facts 做 MIR materialization；
/// - 再只按 MIR 产出的 `InstanceKey` 集合生成当前 LLVM codegen 仍需要的 monomorphic HIR fun/member；
/// - 因而实例发现职责归属于 MIR，而不是继续由 HIR 自己扫描 `MonomorphKey` / `TypeStore`。
pub fn lower_for_compilation_unit_multi_files_via_mir_instance_collection(
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
    files_to_lower: &[(&SourceFile, &ast::File)],
    monomorph_requests: &[crate::monomorph::MonomorphRequest],
    type_env: Option<&crate::typecheck::TypeEnv>,
    typecheck_types: &TypeStore,
) -> Result<LoweredHir, Box<crate::mir::MirMaterializeError>> {
    lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_opt_level(
        index,
        compilation_unit,
        files_to_lower,
        monomorph_requests,
        type_env,
        typecheck_types,
        crate::opt::OptLevel::O0,
    )
}

pub fn lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_opt_level(
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
    files_to_lower: &[(&SourceFile, &ast::File)],
    monomorph_requests: &[crate::monomorph::MonomorphRequest],
    type_env: Option<&crate::typecheck::TypeEnv>,
    typecheck_types: &TypeStore,
    opt_level: crate::opt::OptLevel,
) -> Result<LoweredHir, Box<crate::mir::MirMaterializeError>> {
    let request_source_paths = files_to_lower
        .iter()
        .map(|(source, _)| source.path().to_path_buf())
        .collect::<Vec<_>>();
    lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_request_sources(
        index,
        compilation_unit,
        files_to_lower,
        monomorph_requests,
        type_env,
        typecheck_types,
        MirInstanceCollectionOptions {
            stable_cone_key: virtual_stable_cone_key_for_sources(
                files_to_lower.first().map(|(source, _)| *source),
                compilation_unit,
            ),
            request_source_paths: &request_source_paths,
            request_root_mode: crate::mir::MaterializeRequestRootMode::RequestSources,
            opt_level,
        },
    )
}

pub struct MirInstanceCollectionOptions<'a> {
    pub stable_cone_key: StableConeKey,
    pub request_source_paths: &'a [std::path::PathBuf],
    pub request_root_mode: crate::mir::MaterializeRequestRootMode<'a>,
    pub opt_level: crate::opt::OptLevel,
}

/// 为 build / frontend 生成“由 MIR instance collection 决定实例集合”的 HIR 兼容输入，
/// 但允许把“参与 lowering 的文件集合”和“允许贡献实例请求的 request roots”显式分离。
///
/// 说明：
/// - `files_to_lower` 仍决定哪些文件的顶层声明 / body 会进入 HIR 兼容输出；
/// - `request_source_paths` 只决定哪些源文件可以贡献 monomorphization 请求与 request-root
///   可达扫描；
/// - `request_root_mode` 决定 request roots 是整个 request source 集合，还是 production
///   executable 的 entry-main / export entry points；
/// - 这样 frontend 就能把 stdlib/helper/support sources 留在 lowering / fun_index 中，
///   同时避免把这些支持文件里未被入口触达的 generic 调用错误提升为实例收集种子。
/// - 返回值中的 `LoweredHir` 仍承载当前 LLVM codegen 所需的 HIR 兼容输入，但会额外挂住
///   `LoweredHir::materialized_mir()`，作为 production 主路径保留的 canonical materialized
///   MIR / summary 产物。
pub fn lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_request_sources(
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
    files_to_lower: &[(&SourceFile, &ast::File)],
    monomorph_requests: &[crate::monomorph::MonomorphRequest],
    type_env: Option<&crate::typecheck::TypeEnv>,
    typecheck_types: &TypeStore,
    options: MirInstanceCollectionOptions<'_>,
) -> Result<LoweredHir, Box<crate::mir::MirMaterializeError>> {
    let MirInstanceCollectionOptions {
        stable_cone_key,
        request_source_paths,
        request_root_mode,
        opt_level,
    } = options;
    let materialized =
        crate::mir::materialize_compilation_unit_from_typechecked_inputs_with_options(
            compilation_unit,
            index,
            type_env,
            typecheck_types,
            monomorph_requests,
            crate::mir::MaterializeCompilationUnitOptions {
                stable_cone_key: stable_cone_key.clone(),
                request_source_paths,
                request_root_mode,
                opt_level,
            },
        )?;
    let mut lowered = lower_for_compilation_unit_multi_files_internal(
        index,
        compilation_unit,
        files_to_lower,
        &[],
        type_env,
        typecheck_types,
        CompilationUnitLoweringOptions::explicit_mir_instances(
            stable_cone_key,
            &materialized.instance_keys,
            &materialized.types,
            true,
        ),
    )?;
    for ty in materialized.types.iter_ids() {
        let _ = lowered.types.re_intern_from(&materialized.types, ty);
    }
    lowered.materialized_mir = Some(materialized);
    Ok(lowered)
}

/// 为 typed dump / MIR materializer 构造“只保留 generic template”的多文件 HIR。
///
/// 该入口会复用 resolver/typecheck 事实，但显式关闭 HIR lowering 中遗留的 generic
/// `::<...>` 实例物化路径，使实例身份只在后续 MIR 层建立。
pub(crate) fn lower_generic_for_compilation_unit_multi_files_with_type_env(
    stable_cone_key: StableConeKey,
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
    files_to_lower: &[(&SourceFile, &ast::File)],
    type_env: Option<&crate::typecheck::TypeEnv>,
    typecheck_types: &TypeStore,
) -> Result<LoweredHir, HirLowerError> {
    lower_for_compilation_unit_multi_files_internal(
        index,
        compilation_unit,
        files_to_lower,
        &[],
        type_env,
        typecheck_types,
        CompilationUnitLoweringOptions::generic_template_only(stable_cone_key),
    )
}

enum CompilationUnitInstanceMode<'a> {
    DirectLoweredHir,
    ExplicitMirInstances {
        instance_keys: &'a [crate::mir::InstanceKey],
        instance_types: &'a TypeStore,
    },
    GenericTemplateOnly,
}

struct CompilationUnitLoweringOptions<'a> {
    stable_cone_key: StableConeKey,
    instance_mode: CompilationUnitInstanceMode<'a>,
    devirtualize_dispatch_calls: bool,
    runtime_comptime_plans: &'a HashMap<std::path::PathBuf, crate::comptime::RuntimeComptimePlan>,
}

impl<'a> CompilationUnitLoweringOptions<'a> {
    fn direct_lowered_hir(stable_cone_key: StableConeKey) -> Self {
        Self {
            stable_cone_key,
            instance_mode: CompilationUnitInstanceMode::DirectLoweredHir,
            devirtualize_dispatch_calls: false,
            runtime_comptime_plans: empty_runtime_comptime_plans(),
        }
    }

    fn explicit_mir_instances(
        stable_cone_key: StableConeKey,
        instance_keys: &'a [crate::mir::InstanceKey],
        instance_types: &'a TypeStore,
        devirtualize_dispatch_calls: bool,
    ) -> Self {
        Self {
            stable_cone_key,
            instance_mode: CompilationUnitInstanceMode::ExplicitMirInstances {
                instance_keys,
                instance_types,
            },
            devirtualize_dispatch_calls,
            runtime_comptime_plans: empty_runtime_comptime_plans(),
        }
    }

    fn generic_template_only(stable_cone_key: StableConeKey) -> Self {
        Self {
            stable_cone_key,
            instance_mode: CompilationUnitInstanceMode::GenericTemplateOnly,
            devirtualize_dispatch_calls: false,
            runtime_comptime_plans: empty_runtime_comptime_plans(),
        }
    }

    fn with_runtime_comptime_plans(
        mut self,
        runtime_comptime_plans: &'a HashMap<
            std::path::PathBuf,
            crate::comptime::RuntimeComptimePlan,
        >,
    ) -> Self {
        self.runtime_comptime_plans = runtime_comptime_plans;
        self
    }

    fn materialize_direct_call_targets(&self) -> bool {
        !matches!(
            self.instance_mode,
            CompilationUnitInstanceMode::GenericTemplateOnly
        )
    }
}

fn empty_runtime_comptime_plans()
-> &'static HashMap<std::path::PathBuf, crate::comptime::RuntimeComptimePlan> {
    static EMPTY: std::sync::OnceLock<
        HashMap<std::path::PathBuf, crate::comptime::RuntimeComptimePlan>,
    > = std::sync::OnceLock::new();
    EMPTY.get_or_init(HashMap::new)
}

fn virtual_stable_cone_key_for_sources(
    primary_source: Option<&SourceFile>,
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> StableConeKey {
    primary_source
        .map(|source| StableConeKey::for_virtual_source_path(source.path()))
        .or_else(|| {
            compilation_unit
                .first()
                .map(|(source, _)| StableConeKey::for_virtual_source_path(source.path()))
        })
        .unwrap_or_else(|| StableConeKey::new("virtual-cone", "0.0.0"))
}

fn lower_for_compilation_unit_multi_files_internal<'a>(
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
    files_to_lower: &[(&SourceFile, &ast::File)],
    monomorph_keys: &[crate::monomorph::MonomorphKey],
    type_env: Option<&crate::typecheck::TypeEnv>,
    typecheck_types: &TypeStore,
    options: CompilationUnitLoweringOptions<'a>,
) -> Result<LoweredHir, HirLowerError> {
    let materialize_direct_call_targets = options.materialize_direct_call_targets();
    let CompilationUnitLoweringOptions {
        stable_cone_key,
        instance_mode,
        devirtualize_dispatch_calls,
        runtime_comptime_plans,
    } = options;
    let type_kinds = collect_type_decl_kinds(compilation_unit);
    let nominal_variances = collect_nominal_variances(compilation_unit);
    let direct_supertypes = collect_direct_supertypes(compilation_unit, index);
    let known_receiver_subclasses =
        crate::devirtualize::collect_known_receiver_subclasses(&direct_supertypes);
    let delegated_properties = collect_delegated_properties(compilation_unit);
    let default_arg_structs = collect_default_arg_structs(compilation_unit);
    let value_type_computed_properties =
        collect_value_type_computed_property_fqns(compilation_unit);
    let class_vtables = crate::vtable::collect_class_vtables(compilation_unit, index)?;
    let (interfaces, class_itables) = match type_env {
        Some(env) => crate::itable::collect_runtime_interfaces_and_class_itables_with_env(
            compilation_unit,
            index,
            &class_vtables,
            env,
            typecheck_types,
        )?,
        None => crate::itable::collect_runtime_interfaces_and_class_itables(
            compilation_unit,
            index,
            &class_vtables,
            typecheck_types,
        )?,
    };

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let generic_template_symbol_suffixes =
        util::collect_generic_template_symbol_suffixes_with_stable_cone_key(
            &stable_cone_key,
            index,
            compilation_unit,
        );

    let mut decls: Vec<Decl> = Vec::new();
    let mut items: Vec<Item> = Vec::new();
    let mut member_funs: Vec<FunDecl> = Vec::new();
    let mut ctor_call_sites: CtorCallSiteIndex = HashMap::new();
    let mut dispatch_call_sites: super::DispatchCallSiteIndex = HashMap::new();
    let mut effect_op_call_sites: super::EffectOpCallSiteIndex = HashMap::new();
    let mut handle_payload_tuple_tys: super::HandlePayloadTupleSiteIndex = HashMap::new();
    let mut with_update_contracts: WithUpdateSiteIndex = HashMap::new();
    let mut assign_place_contracts: AssignPlaceSiteIndex = HashMap::new();
    let mut continuation_resume_call_sites: ContinuationResumeCallSiteIndex =
        ContinuationResumeCallSiteIndex::new();
    let mut non_pure_continuation_resume_call_sites: NonPureContinuationResumeCallSiteIndex =
        NonPureContinuationResumeCallSiteIndex::new();
    let mut top_level_vars: super::TopLevelVarIndex = HashMap::new();
    let mut extern_globals: super::ExternGlobalIndex = HashMap::new();
    let mut top_level_consts: super::TopLevelConstIndex = HashMap::new();
    let mut top_level_immutable_values: super::TopLevelImmutableValueIndex = HashMap::new();
    let mut when_pat_binding_tys: super::WhenPatBindingTypeIndex = HashMap::new();

    for (source, file) in files_to_lower {
        let (
            file_hir,
            file_member_funs,
            file_ctor_call_sites,
            file_dispatch_call_sites,
            file_effect_op_call_sites,
            file_handle_payload_tuple_tys,
            file_with_update_contracts,
            file_assign_place_contracts,
            file_top_level_vars,
            file_extern_globals,
            file_top_level_consts,
            file_top_level_immutable_values,
            file_when_pat_binding_tys,
        ) = {
            let mut ctx = HirLowering::new(
                source,
                file,
                index,
                &mut types,
                HirLoweringSetup {
                    typecheck_types: Some(typecheck_types),
                    type_kinds: &type_kinds,
                    delegated_properties: &delegated_properties,
                    compilation_unit,
                    default_arg_structs: default_arg_structs.clone(),
                    value_type_computed_properties: &value_type_computed_properties,
                    builtins,
                    generic_template_symbol_suffixes: &generic_template_symbol_suffixes,
                    known_receiver_subclasses: &known_receiver_subclasses,
                    class_vtables: &class_vtables,
                    interfaces: &interfaces,
                    class_itables: &class_itables,
                    materialize_direct_call_targets,
                    devirtualize_dispatch_calls,
                    runtime_comptime_plan: runtime_comptime_plans.get(source.path()),
                },
            );
            let file_hir = ctx.lower_file();
            if let Some(err) = ctx.take_stage_error() {
                return Err(err.into());
            }
            // 字面量已不再依赖“仅入口文件可切片”的旧路径，因此这里可以稳定收集所有文件的 member_funs。
            let pkg_prefix = package_prefix(source, file.package.as_ref());
            let file_member_funs = ctx.collect_member_funs(&pkg_prefix);
            if let Some(err) = ctx.take_stage_error() {
                return Err(err.into());
            }
            ctx.record_missing_assign_place_contracts_in_file(&file_hir);
            ctx.record_missing_assign_place_contracts_in_funs(&file_member_funs);
            let ctor_call_sites = std::mem::take(&mut ctx.ctor_call_sites);
            let dispatch_call_sites = std::mem::take(&mut ctx.dispatch_call_sites);
            let effect_op_call_sites = std::mem::take(&mut ctx.effect_op_call_sites);
            let handle_payload_tuple_tys = std::mem::take(&mut ctx.handle_payload_tuple_tys);
            let file_with_update_contracts = std::mem::take(&mut ctx.with_update_contracts);
            let file_assign_place_contracts = std::mem::take(&mut ctx.assign_place_contracts);
            let file_top_level_vars = std::mem::take(&mut ctx.top_level_vars);
            let file_extern_globals = std::mem::take(&mut ctx.extern_globals);
            let file_top_level_consts = std::mem::take(&mut ctx.top_level_consts);
            let file_top_level_immutable_values =
                std::mem::take(&mut ctx.top_level_immutable_values);
            let file_when_pat_binding_tys = std::mem::take(&mut ctx.when_pat_binding_tys);
            (
                file_hir,
                file_member_funs,
                ctor_call_sites,
                dispatch_call_sites,
                effect_op_call_sites,
                handle_payload_tuple_tys,
                file_with_update_contracts,
                file_assign_place_contracts,
                file_top_level_vars,
                file_extern_globals,
                file_top_level_consts,
                file_top_level_immutable_values,
                file_when_pat_binding_tys,
            )
        };

        ctor_call_sites.extend(file_ctor_call_sites);
        dispatch_call_sites.extend(file_dispatch_call_sites);
        effect_op_call_sites.extend(file_effect_op_call_sites);
        handle_payload_tuple_tys.extend(file_handle_payload_tuple_tys);
        with_update_contracts.extend(file_with_update_contracts);
        assign_place_contracts.extend(file_assign_place_contracts);
        continuation_resume_call_sites.extend(
            file.continuation_resume_call_sites()
                .into_iter()
                .map(|span| CallSite::new(source.path().to_path_buf(), span)),
        );
        non_pure_continuation_resume_call_sites.extend(
            file.non_pure_continuation_resume_call_sites()
                .into_iter()
                .map(|span| CallSite::new(source.path().to_path_buf(), span)),
        );

        top_level_vars.extend(file_top_level_vars);
        extern_globals.extend(file_extern_globals);
        top_level_consts.extend(file_top_level_consts);
        top_level_immutable_values.extend(file_top_level_immutable_values);
        when_pat_binding_tys.extend(file_when_pat_binding_tys);
        member_funs.extend(file_member_funs);
        decls.extend(file_hir.decls);
        items.extend(file_hir.items);
    }

    // side tables：保持与 `lower_for_compilation_unit` 一致的收集逻辑（先降 HIR，再补充 layout/init/extern）。
    let extern_funs = files_to_lower
        .iter()
        .flat_map(|(source, file)| collect_extern_funs(source, file))
        .collect();
    let extern_libs = collect_extern_libs(compilation_unit);
    let mut struct_layouts = collect_struct_layouts(compilation_unit, index, &mut types);
    let mut enum_layouts = collect_enum_layouts(compilation_unit, index, &mut types);
    // T0124：泛型 struct/enum 的具体实例化布局。
    struct_layouts.extend(collect_generic_struct_instantiation_layouts(
        compilation_unit,
        index,
        &mut types,
    ));
    enum_layouts.extend(collect_generic_enum_instantiation_layouts(
        compilation_unit,
        index,
        &mut types,
    ));

    let (
        object_inits,
        mut class_inits,
        side_table_ctor_call_sites,
        side_table_dispatch_call_sites,
        side_table_with_update_contracts,
        side_table_assign_place_contracts,
    ) = collect_compilation_unit_object_and_class_inits(
        compilation_unit,
        CompilationUnitInitCollectionInputs {
            index,
            type_kinds: &type_kinds,
            known_receiver_subclasses: &known_receiver_subclasses,
            class_vtables: &class_vtables,
            interfaces: &interfaces,
            class_itables: &class_itables,
            typecheck_types: Some(typecheck_types),
            materialize_direct_call_targets,
            devirtualize_dispatch_calls,
            builtins,
        },
        &mut types,
    )?;
    ctor_call_sites.extend(side_table_ctor_call_sites);
    dispatch_call_sites.extend(side_table_dispatch_call_sites);
    with_update_contracts.extend(side_table_with_update_contracts);
    assign_place_contracts.extend(side_table_assign_place_contracts);
    // T0125：泛型 class 的具体实例化 ClassInit（第一遍：处理文件中已有的泛型 class 实例化类型）。
    class_inits.extend(collect_generic_class_instantiation_inits(
        compilation_unit,
        &mut types,
        &class_inits,
    ));

    match instance_mode {
        CompilationUnitInstanceMode::DirectLoweredHir => {
            // T0127：为泛型独立函数的具体实例化生成单态化的 FunDecl。
            // 注意：必须在 class member monomorphization 之前运行，因为独立函数的单态化
            // 可能在 TypeStore 中创建新的泛型 class 实例化类型（例如 `Printer<Greeter>`），
            // 这些类型需要被后续的 class member monomorphization 发现。
            let monomorphized_funs =
                collect_generic_fun_instantiations(GenericFunInstantiationInputs {
                    compilation_unit,
                    monomorph_keys,
                    index,
                    type_kinds: &type_kinds,
                    types: &mut types,
                    builtins,
                    typecheck_types,
                    initial_items: &items,
                    initial_member_funs: &member_funs,
                    stable_cone_key: &stable_cone_key,
                });
            items.extend(monomorphized_funs.into_iter().map(Item::Fun));

            // T0130：第二遍 class 实例化 —— standalone fun monomorphization 可能在 TypeStore 中
            // 创建了新的泛型 class 实例化类型（例如 `Printer<Greeter>`），这里补充收集。
            class_inits.extend(collect_generic_class_instantiation_inits(
                compilation_unit,
                &mut types,
                &class_inits,
            ));

            // T0126：为泛型 class 的具体实例化生成单态化的成员方法 FunDecl。
            member_funs.extend(collect_generic_member_fun_instantiations(
                compilation_unit,
                index,
                &type_kinds,
                Some(typecheck_types),
                &mut types,
                builtins,
            ));
            struct_layouts.extend(collect_generic_struct_instantiation_layouts(
                compilation_unit,
                index,
                &mut types,
            ));
            enum_layouts.extend(collect_generic_enum_instantiation_layouts(
                compilation_unit,
                index,
                &mut types,
            ));
            class_inits.extend(collect_generic_class_instantiation_inits(
                compilation_unit,
                &mut types,
                &class_inits,
            ));
        }
        CompilationUnitInstanceMode::ExplicitMirInstances {
            instance_keys,
            instance_types,
        } => {
            let monomorphic_funs = collect_generic_fun_instantiations_from_instance_keys(
                ExplicitGenericFunInstantiationInputs {
                    compilation_unit,
                    instance_keys,
                    instance_types,
                    index,
                    type_kinds: &type_kinds,
                    types: &mut types,
                    builtins,
                    typecheck_types,
                    stable_cone_key: &stable_cone_key,
                },
            )?;
            items.extend(monomorphic_funs.into_iter().map(Item::Fun));

            class_inits.extend(collect_generic_class_instantiation_inits(
                compilation_unit,
                &mut types,
                &class_inits,
            ));

            member_funs.extend(
                collect_generic_member_fun_instantiations_from_instance_keys(
                    ExplicitGenericMemberInstantiationInputs {
                        compilation_unit,
                        instance_keys,
                        instance_types,
                        index,
                        type_kinds: &type_kinds,
                        types: &mut types,
                        builtins,
                        typecheck_types: Some(typecheck_types),
                        stable_cone_key: &stable_cone_key,
                    },
                )?,
            );
            let wanted_dispatch_member_fqns = class_itables
                .values()
                .flat_map(|entries| {
                    entries.iter().flat_map(|entry| {
                        entry
                            .method_impl_fqns
                            .iter()
                            .filter(|fqn| fqn.contains("::<"))
                            .cloned()
                    })
                })
                .collect::<HashSet<_>>();
            if !wanted_dispatch_member_fqns.is_empty() {
                let existing_member_fqns = member_funs
                    .iter()
                    .map(|fun| fun.fqn.clone())
                    .collect::<HashSet<_>>();
                member_funs.extend(
                    collect_generic_member_fun_instantiations(
                        compilation_unit,
                        index,
                        &type_kinds,
                        Some(typecheck_types),
                        &mut types,
                        builtins,
                    )
                    .into_iter()
                    .filter(|fun| wanted_dispatch_member_fqns.contains(&fun.fqn))
                    .filter(|fun| !existing_member_fqns.contains(&fun.fqn)),
                );
            }
            struct_layouts.extend(collect_generic_struct_instantiation_layouts(
                compilation_unit,
                index,
                &mut types,
            ));
            enum_layouts.extend(collect_generic_enum_instantiation_layouts(
                compilation_unit,
                index,
                &mut types,
            ));
            class_inits.extend(collect_generic_class_instantiation_inits(
                compilation_unit,
                &mut types,
                &class_inits,
            ));
        }
        CompilationUnitInstanceMode::GenericTemplateOnly => {}
    }

    let top_level_fun_call_sites = collect_top_level_fun_call_sites_with_type_remap(
        files_to_lower,
        Some(typecheck_types),
        &mut types,
    );
    let call_arg_bindings = collect_call_arg_bindings(files_to_lower);
    let stable_type_param_keys = collect_stable_type_param_keys(compilation_unit, &stable_cone_key);

    Ok(LoweredHir {
        file: File { decls, items },
        stable_cone_key,
        stable_type_param_keys,
        member_funs,
        materialized_mir: None,
        types,
        struct_layouts,
        enum_layouts,
        extern_funs,
        extern_globals,
        extern_libs,
        top_level_vars,
        top_level_consts,
        top_level_immutable_values,
        top_level_fun_call_sites,
        call_arg_bindings,
        with_update_contracts,
        assign_place_contracts,
        object_inits,
        class_inits,
        class_vtables,
        interfaces,
        class_itables,
        ctor_call_sites,
        dispatch_call_sites,
        effect_op_call_sites,
        handle_payload_tuple_tys,
        continuation_resume_call_sites,
        non_pure_continuation_resume_call_sites,
        when_pat_binding_tys,
        nominal_kinds: type_kinds,
        nominal_variances,
        direct_supertypes,
        builtins,
    })
}

pub(crate) struct LoweringInputs<'a> {
    pub(crate) source: &'a SourceFile,
    pub(crate) file: &'a ast::File,
    pub(crate) index: &'a Index,
    pub(crate) type_kinds: &'a HashMap<String, ast::TypeKind>,
    pub(crate) known_receiver_subclasses: &'a crate::devirtualize::KnownReceiverSubclassIndex,
    pub(crate) class_vtables: &'a crate::vtable::ClassVtableIndex,
    pub(crate) interfaces: &'a crate::itable::InterfaceIndex,
    pub(crate) class_itables: &'a crate::itable::ClassItableIndex,
    pub(crate) typecheck_types: Option<&'a TypeStore>,
    pub(crate) compilation_unit: &'a [(&'a SourceFile, &'a ast::File)],
    pub(crate) types: &'a mut TypeStore,
    pub(crate) builtins: BuiltinTypes,
    pub(crate) generic_template_symbol_suffixes: &'a util::GenericTemplateSymbolSuffixIndex,
    pub(crate) materialize_direct_call_targets: bool,
    pub(crate) devirtualize_dispatch_calls: bool,
}

pub(super) struct BoundMemberFunLoweringTarget<'a> {
    pub(super) owner_fqn: &'a str,
    pub(super) this_decl_span: Span,
    pub(super) this_concrete_args: &'a [TypeId],
    pub(super) fun: &'a ast::FunDecl,
}

pub(super) struct BoundValuePropertyGetterLoweringTarget<'a> {
    pub(super) owner_fqn: &'a str,
    pub(super) this_decl_span: Span,
    pub(super) this_concrete_args: &'a [TypeId],
    pub(super) property: &'a ast::PropertyDecl,
}

pub(crate) struct LoweredFunWithMirFacts {
    pub(crate) fun: FunDecl,
    pub(crate) dispatch_call_sites: super::DispatchCallSiteIndex,
    pub(crate) effect_op_call_sites: super::EffectOpCallSiteIndex,
    pub(crate) when_pat_binding_tys: super::WhenPatBindingTypeIndex,
    pub(crate) top_level_fun_call_sites: super::TopLevelFunCallSiteIndex,
}

/// 将给定的 `ast::FunDecl` 在”已绑定 type params”的语境下降低为 HIR（用于单态化，T0712）。
///
/// 说明：
/// - 该函数假设调用方已在 AST 上运行 resolver（headers + bodies），以便 `ValueIdent.resolved` 等信息可用；
/// - `type_bindings` 用于把函数声明处的 `T` 等 type params 映射为具体 `TypeId`（调用点推断结果）。
pub(crate) fn lower_fun_with_type_bindings(
    inputs: LoweringInputs<'_>,
    fun: &ast::FunDecl,
    type_bindings: impl IntoIterator<Item = (String, TypeId)>,
) -> FunDecl {
    lower_fun_with_bindings(inputs, fun, type_bindings, None)
}

pub(crate) fn lower_fun_with_bindings(
    inputs: LoweringInputs<'_>,
    fun: &ast::FunDecl,
    type_bindings: impl IntoIterator<Item = (String, TypeId)>,
    effect_binding: Option<(String, EffectRow)>,
) -> FunDecl {
    lower_fun_with_bindings_and_mir_facts(inputs, fun, type_bindings, effect_binding).fun
}

pub(crate) fn lower_fun_with_type_bindings_and_mir_facts(
    inputs: LoweringInputs<'_>,
    fun: &ast::FunDecl,
    type_bindings: impl IntoIterator<Item = (String, TypeId)>,
) -> LoweredFunWithMirFacts {
    lower_fun_with_bindings_and_mir_facts(inputs, fun, type_bindings, None)
}

fn lower_fun_with_bindings_and_mir_facts(
    inputs: LoweringInputs<'_>,
    fun: &ast::FunDecl,
    type_bindings: impl IntoIterator<Item = (String, TypeId)>,
    effect_binding: Option<(String, EffectRow)>,
) -> LoweredFunWithMirFacts {
    let LoweringInputs {
        source,
        file,
        index,
        type_kinds,
        known_receiver_subclasses,
        class_vtables,
        interfaces,
        class_itables,
        typecheck_types,
        compilation_unit,
        types,
        builtins,
        generic_template_symbol_suffixes,
        materialize_direct_call_targets,
        devirtualize_dispatch_calls,
    } = inputs;
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let delegated_properties = collect_delegated_properties(compilation_unit);
    let default_arg_structs = collect_default_arg_structs(compilation_unit);
    let value_type_computed_properties =
        collect_value_type_computed_property_fqns(compilation_unit);
    let mut ctx = HirLowering::new(
        source,
        file,
        index,
        types,
        HirLoweringSetup {
            typecheck_types,
            type_kinds,
            delegated_properties: &delegated_properties,
            compilation_unit,
            default_arg_structs,
            value_type_computed_properties: &value_type_computed_properties,
            builtins,
            generic_template_symbol_suffixes,
            known_receiver_subclasses,
            class_vtables,
            interfaces,
            class_itables,
            materialize_direct_call_targets,
            devirtualize_dispatch_calls,
            runtime_comptime_plan: None,
        },
    );
    let type_bindings = type_bindings.into_iter().collect::<Vec<_>>();
    let type_bindings_pushed = !type_bindings.is_empty();
    if type_bindings_pushed {
        ctx.push_type_param_bindings(type_bindings);
    }
    let effect_binding_pushed = if let Some((name, row)) = effect_binding {
        ctx.push_effect_row_param_binding(name, row);
        true
    } else {
        false
    };
    let out = ctx.lower_fun_decl_with_bound_type_params(&pkg_prefix, fun);
    if effect_binding_pushed {
        ctx.pop_effect_row_param_binding();
    }
    if type_bindings_pushed {
        ctx.pop_type_params();
    }
    LoweredFunWithMirFacts {
        fun: out,
        dispatch_call_sites: std::mem::take(&mut ctx.dispatch_call_sites),
        effect_op_call_sites: std::mem::take(&mut ctx.effect_op_call_sites),
        when_pat_binding_tys: std::mem::take(&mut ctx.when_pat_binding_tys),
        top_level_fun_call_sites: collect_top_level_fun_call_sites(&[(source, file)]),
    }
}

/// T0126: 将泛型类的成员方法在”已绑定 owner type params”的语境下降低为 HIR。
///
/// 说明：
/// - 类似 `lower_fun_with_type_bindings`，但处理的是 class member method（带隐式 `this` 参数）；
/// - `owner_type_bindings` 映射 owner 的 type params 到具体 TypeId（例如 `T → Int` for `Box<Int>`）；
/// - `this_concrete_args` 是 owner 的具体类型实参（例如 `[Int]` for `Box<Int>`），用于构造 `this` 的精确类型；
/// - 生成的 FunDecl 中 `this` 参数类型为具体的 `owner_fqn<concrete_args>`，所有引用 owner
///   type params 的地方均被替换为具体类型。
pub(crate) fn lower_member_fun_with_type_bindings(
    inputs: LoweringInputs<'_>,
    target: BoundMemberFunLoweringTarget<'_>,
    owner_type_bindings: impl IntoIterator<Item = (String, TypeId)>,
) -> FunDecl {
    lower_member_fun_with_bindings(
        inputs,
        target,
        owner_type_bindings,
        Vec::<(String, TypeId)>::new(),
        None,
    )
}

pub(crate) fn lower_member_fun_with_bindings(
    inputs: LoweringInputs<'_>,
    target: BoundMemberFunLoweringTarget<'_>,
    owner_type_bindings: impl IntoIterator<Item = (String, TypeId)>,
    fun_type_bindings: impl IntoIterator<Item = (String, TypeId)>,
    effect_binding: Option<(String, EffectRow)>,
) -> FunDecl {
    let LoweringInputs {
        source,
        file,
        index,
        type_kinds,
        known_receiver_subclasses,
        class_vtables,
        interfaces,
        class_itables,
        typecheck_types,
        compilation_unit,
        types,
        builtins,
        generic_template_symbol_suffixes,
        materialize_direct_call_targets,
        devirtualize_dispatch_calls,
    } = inputs;
    let BoundMemberFunLoweringTarget {
        owner_fqn,
        this_decl_span,
        this_concrete_args,
        fun,
    } = target;
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let delegated_properties = collect_delegated_properties(compilation_unit);
    let default_arg_structs = collect_default_arg_structs(compilation_unit);
    let value_type_computed_properties =
        collect_value_type_computed_property_fqns(compilation_unit);
    let mut ctx = HirLowering::new(
        source,
        file,
        index,
        types,
        HirLoweringSetup {
            typecheck_types,
            type_kinds,
            delegated_properties: &delegated_properties,
            compilation_unit,
            default_arg_structs,
            value_type_computed_properties: &value_type_computed_properties,
            builtins,
            generic_template_symbol_suffixes,
            known_receiver_subclasses,
            class_vtables,
            interfaces,
            class_itables,
            materialize_direct_call_targets,
            devirtualize_dispatch_calls,
            runtime_comptime_plan: None,
        },
    );
    // 先绑定 owner type params（例如 class Box<T> 的 T → Int），
    // 再由 lower_member_fun_decl_with_bound_type_params 处理方法 body。
    let owner_type_bindings = owner_type_bindings.into_iter().collect::<Vec<_>>();
    let owner_type_bindings_pushed = !owner_type_bindings.is_empty();
    if owner_type_bindings_pushed {
        ctx.push_type_param_bindings(owner_type_bindings);
    }
    let fun_type_bindings = fun_type_bindings.into_iter().collect::<Vec<_>>();
    let fun_type_bindings_pushed = !fun_type_bindings.is_empty();
    if fun_type_bindings_pushed {
        ctx.push_type_param_bindings(fun_type_bindings);
    }
    let effect_binding_pushed = if let Some((name, row)) = effect_binding {
        ctx.push_effect_row_param_binding(name, row);
        true
    } else {
        false
    };
    let out = ctx.lower_member_fun_decl_with_bound_type_params(
        &pkg_prefix,
        owner_fqn,
        this_decl_span,
        this_concrete_args,
        fun,
    );
    if effect_binding_pushed {
        ctx.pop_effect_row_param_binding();
    }
    if fun_type_bindings_pushed {
        ctx.pop_type_params();
    }
    if owner_type_bindings_pushed {
        ctx.pop_type_params();
    }
    out
}

/// 将值类型 computed property getter 在“已绑定 owner type params”的语境下降低为 HIR。
pub(crate) fn lower_value_property_getter_with_type_bindings(
    inputs: LoweringInputs<'_>,
    target: BoundValuePropertyGetterLoweringTarget<'_>,
    owner_type_bindings: impl IntoIterator<Item = (String, TypeId)>,
) -> FunDecl {
    let LoweringInputs {
        source,
        file,
        index,
        type_kinds,
        known_receiver_subclasses,
        class_vtables,
        interfaces,
        class_itables,
        typecheck_types,
        compilation_unit,
        types,
        builtins,
        generic_template_symbol_suffixes,
        materialize_direct_call_targets,
        devirtualize_dispatch_calls,
    } = inputs;
    let BoundValuePropertyGetterLoweringTarget {
        owner_fqn,
        this_decl_span,
        this_concrete_args,
        property,
    } = target;
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let delegated_properties = collect_delegated_properties(compilation_unit);
    let default_arg_structs = collect_default_arg_structs(compilation_unit);
    let value_type_computed_properties =
        collect_value_type_computed_property_fqns(compilation_unit);
    let mut ctx = HirLowering::new(
        source,
        file,
        index,
        types,
        HirLoweringSetup {
            typecheck_types,
            type_kinds,
            delegated_properties: &delegated_properties,
            compilation_unit,
            default_arg_structs,
            value_type_computed_properties: &value_type_computed_properties,
            builtins,
            generic_template_symbol_suffixes,
            known_receiver_subclasses,
            class_vtables,
            interfaces,
            class_itables,
            materialize_direct_call_targets,
            devirtualize_dispatch_calls,
            runtime_comptime_plan: None,
        },
    );
    let owner_type_bindings = owner_type_bindings.into_iter().collect::<Vec<_>>();
    let owner_type_bindings_pushed = !owner_type_bindings.is_empty();
    if owner_type_bindings_pushed {
        ctx.push_type_param_bindings(owner_type_bindings);
    }
    let out = ctx.lower_value_property_getter_decl_with_bound_type_params(
        &pkg_prefix,
        owner_fqn,
        this_decl_span,
        this_concrete_args,
        property,
    );
    if owner_type_bindings_pushed {
        ctx.pop_type_params();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::LiteralKind;
    use crate::hir::{CallArg, CallSite, ClassInitStep, WhenPat};
    use crate::resolve::Index;
    use crate::session::SessionOptions;
    use crate::ty::{RefTypeKind, TypeKind, TypeStore, ValueTypeKind};
    use crate::typecheck;
    use std::collections::HashSet;
    use std::path::PathBuf;

    fn collect_unresolved_member_names_in_expr(expr: &Expr, out: &mut Vec<String>) {
        match &expr.kind {
            ExprKind::MemberAccess { receiver, member } => {
                if member.resolved.is_none() {
                    out.push(member.name.clone());
                }
                collect_unresolved_member_names_in_expr(receiver, out);
            }
            ExprKind::Call { callee, args } => {
                collect_unresolved_member_names_in_expr(callee, out);
                for arg in args {
                    match arg {
                        CallArg::Positional(expr) => {
                            collect_unresolved_member_names_in_expr(expr, out)
                        }
                        CallArg::Named { value, .. } => {
                            collect_unresolved_member_names_in_expr(value, out)
                        }
                    }
                }
            }
            ExprKind::When { subject, arms } => {
                collect_unresolved_member_names_in_expr(subject, out);
                for arm in arms {
                    if let Some(guard) = arm.guard.as_ref() {
                        collect_unresolved_member_names_in_expr(guard, out);
                    }
                    collect_unresolved_member_names_in_expr(&arm.body, out);
                }
            }
            ExprKind::Block(block) => collect_unresolved_member_names_in_block(block, out),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                collect_unresolved_member_names_in_expr(cond, out);
                collect_unresolved_member_names_in_expr(then_branch, out);
                if let Some(else_branch) = else_branch.as_deref() {
                    collect_unresolved_member_names_in_expr(else_branch, out);
                }
            }
            ExprKind::Unary { expr, .. }
            | ExprKind::TypeCheck { expr, .. }
            | ExprKind::Cast { expr, .. } => collect_unresolved_member_names_in_expr(expr, out),
            ExprKind::Binary { lhs, rhs, .. } => {
                collect_unresolved_member_names_in_expr(lhs, out);
                collect_unresolved_member_names_in_expr(rhs, out);
            }
            ExprKind::StructLit { fields, .. } => {
                for field in fields {
                    collect_unresolved_member_names_in_expr(&field.value, out);
                }
            }
            ExprKind::TupleLit { elements } => {
                for element in elements {
                    collect_unresolved_member_names_in_expr(element, out);
                }
            }
            ExprKind::InterpolatedString { parts, .. } => {
                for part in parts {
                    if let crate::hir::InterpolatedStringPart::Expr { expr } = part {
                        collect_unresolved_member_names_in_expr(expr, out);
                    }
                }
            }
            ExprKind::Perform { args, .. } => {
                for arg in args {
                    match arg {
                        CallArg::Positional(expr) => {
                            collect_unresolved_member_names_in_expr(expr, out)
                        }
                        CallArg::Named { value, .. } => {
                            collect_unresolved_member_names_in_expr(value, out)
                        }
                    }
                }
            }
            ExprKind::Handle(handle) => {
                collect_unresolved_member_names_in_block(&handle.body, out);
                for arm in &handle.arms {
                    collect_unresolved_member_names_in_expr(&arm.body, out);
                }
                if let Some(finally) = handle.finally.as_ref() {
                    collect_unresolved_member_names_in_block(finally, out);
                }
            }
            ExprKind::Literal(_)
            | ExprKind::VarRef(_)
            | ExprKind::UnresolvedIdent { .. }
            | ExprKind::ClassLiteral(_)
            | ExprKind::Closure(_)
            | ExprKind::Missing
            | ExprKind::Todo(_) => {}
        }
    }

    fn collect_unresolved_member_names_in_block(block: &Block, out: &mut Vec<String>) {
        for stmt in &block.stmts {
            match &stmt.kind {
                StmtKind::Expr(expr) => collect_unresolved_member_names_in_expr(expr, out),
                StmtKind::Val(val) => {
                    if let Some(init) = val.init.as_ref() {
                        collect_unresolved_member_names_in_expr(init, out);
                    }
                }
                StmtKind::Assign { lhs, rhs, .. } => {
                    collect_unresolved_member_names_in_expr(lhs, out);
                    collect_unresolved_member_names_in_expr(rhs, out);
                }
                StmtKind::While { cond, body } => {
                    collect_unresolved_member_names_in_expr(cond, out);
                    collect_unresolved_member_names_in_block(body, out);
                }
                StmtKind::Return { value } => {
                    if let Some(value) = value.as_ref() {
                        collect_unresolved_member_names_in_expr(value, out);
                    }
                }
                StmtKind::Empty
                | StmtKind::Break { .. }
                | StmtKind::Continue { .. }
                | StmtKind::Todo(_) => {}
            }
        }
    }

    fn expr_contains_todo_kind(expr: &Expr, kind: &str) -> bool {
        match &expr.kind {
            ExprKind::Todo(found) => *found == kind,
            ExprKind::MemberAccess { receiver, .. } => expr_contains_todo_kind(receiver, kind),
            ExprKind::Call { callee, args } => {
                expr_contains_todo_kind(callee, kind)
                    || args.iter().any(|arg| match arg {
                        CallArg::Positional(expr) => expr_contains_todo_kind(expr, kind),
                        CallArg::Named { value, .. } => expr_contains_todo_kind(value, kind),
                    })
            }
            ExprKind::StructLit { fields, .. } => fields
                .iter()
                .any(|field| expr_contains_todo_kind(&field.value, kind)),
            ExprKind::TupleLit { elements } => elements
                .iter()
                .any(|element| expr_contains_todo_kind(element, kind)),
            ExprKind::When { subject, arms } => {
                expr_contains_todo_kind(subject, kind)
                    || arms
                        .iter()
                        .any(|arm| expr_contains_todo_kind(&arm.body, kind))
            }
            ExprKind::Block(block) => block.stmts.iter().any(|stmt| match &stmt.kind {
                StmtKind::Expr(expr) => expr_contains_todo_kind(expr, kind),
                StmtKind::Val(decl) => decl
                    .init
                    .as_ref()
                    .is_some_and(|expr| expr_contains_todo_kind(expr, kind)),
                StmtKind::Assign { lhs, rhs, .. } => {
                    expr_contains_todo_kind(lhs, kind) || expr_contains_todo_kind(rhs, kind)
                }
                StmtKind::While { cond, body } => {
                    expr_contains_todo_kind(cond, kind)
                        || body.stmts.iter().any(|body_stmt| match &body_stmt.kind {
                            StmtKind::Expr(expr) => expr_contains_todo_kind(expr, kind),
                            StmtKind::Val(decl) => decl
                                .init
                                .as_ref()
                                .is_some_and(|expr| expr_contains_todo_kind(expr, kind)),
                            _ => false,
                        })
                }
                StmtKind::Return { value } => value
                    .as_ref()
                    .is_some_and(|expr| expr_contains_todo_kind(expr, kind)),
                _ => false,
            }),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                expr_contains_todo_kind(cond, kind)
                    || expr_contains_todo_kind(then_branch, kind)
                    || else_branch
                        .as_deref()
                        .is_some_and(|expr| expr_contains_todo_kind(expr, kind))
            }
            ExprKind::Unary { expr, .. }
            | ExprKind::TypeCheck { expr, .. }
            | ExprKind::Cast { expr, .. } => expr_contains_todo_kind(expr, kind),
            ExprKind::Binary { lhs, rhs, .. } => {
                expr_contains_todo_kind(lhs, kind) || expr_contains_todo_kind(rhs, kind)
            }
            _ => false,
        }
    }

    fn collect_top_level_call_fqns_in_expr(expr: &Expr, out: &mut Vec<String>) {
        match &expr.kind {
            ExprKind::Call { callee, args } => {
                if let ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) = &callee.kind {
                    out.push(fqn.clone());
                }
                collect_top_level_call_fqns_in_expr(callee, out);
                for arg in args {
                    match arg {
                        CallArg::Positional(expr) => collect_top_level_call_fqns_in_expr(expr, out),
                        CallArg::Named { value, .. } => {
                            collect_top_level_call_fqns_in_expr(value, out)
                        }
                    }
                }
            }
            ExprKind::MemberAccess { receiver, .. } => {
                collect_top_level_call_fqns_in_expr(receiver, out);
            }
            ExprKind::When { subject, arms } => {
                collect_top_level_call_fqns_in_expr(subject, out);
                for arm in arms {
                    if let Some(guard) = arm.guard.as_ref() {
                        collect_top_level_call_fqns_in_expr(guard, out);
                    }
                    collect_top_level_call_fqns_in_expr(&arm.body, out);
                }
            }
            ExprKind::Block(block) => collect_top_level_call_fqns_in_block(block, out),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                collect_top_level_call_fqns_in_expr(cond, out);
                collect_top_level_call_fqns_in_expr(then_branch, out);
                if let Some(else_branch) = else_branch.as_deref() {
                    collect_top_level_call_fqns_in_expr(else_branch, out);
                }
            }
            ExprKind::Unary { expr, .. }
            | ExprKind::TypeCheck { expr, .. }
            | ExprKind::Cast { expr, .. } => collect_top_level_call_fqns_in_expr(expr, out),
            ExprKind::Binary { lhs, rhs, .. } => {
                collect_top_level_call_fqns_in_expr(lhs, out);
                collect_top_level_call_fqns_in_expr(rhs, out);
            }
            ExprKind::StructLit { fields, .. } => {
                for field in fields {
                    collect_top_level_call_fqns_in_expr(&field.value, out);
                }
            }
            ExprKind::TupleLit { elements } => {
                for element in elements {
                    collect_top_level_call_fqns_in_expr(element, out);
                }
            }
            ExprKind::InterpolatedString { parts, .. } => {
                for part in parts {
                    if let crate::hir::InterpolatedStringPart::Expr { expr } = part {
                        collect_top_level_call_fqns_in_expr(expr, out);
                    }
                }
            }
            ExprKind::Perform { args, .. } => {
                for arg in args {
                    match arg {
                        CallArg::Positional(expr) => collect_top_level_call_fqns_in_expr(expr, out),
                        CallArg::Named { value, .. } => {
                            collect_top_level_call_fqns_in_expr(value, out)
                        }
                    }
                }
            }
            ExprKind::Handle(handle) => {
                collect_top_level_call_fqns_in_block(&handle.body, out);
                for arm in &handle.arms {
                    collect_top_level_call_fqns_in_expr(&arm.body, out);
                }
                if let Some(finally) = handle.finally.as_ref() {
                    collect_top_level_call_fqns_in_block(finally, out);
                }
            }
            ExprKind::Literal(_)
            | ExprKind::VarRef(_)
            | ExprKind::UnresolvedIdent { .. }
            | ExprKind::ClassLiteral(_)
            | ExprKind::Closure(_)
            | ExprKind::Missing
            | ExprKind::Todo(_) => {}
        }
    }

    fn collect_top_level_call_fqns_in_block(block: &Block, out: &mut Vec<String>) {
        for stmt in &block.stmts {
            match &stmt.kind {
                StmtKind::Expr(expr) => collect_top_level_call_fqns_in_expr(expr, out),
                StmtKind::Val(val) => {
                    if let Some(init) = val.init.as_ref() {
                        collect_top_level_call_fqns_in_expr(init, out);
                    }
                }
                StmtKind::Assign { lhs, rhs, .. } => {
                    collect_top_level_call_fqns_in_expr(lhs, out);
                    collect_top_level_call_fqns_in_expr(rhs, out);
                }
                StmtKind::While { cond, body } => {
                    collect_top_level_call_fqns_in_expr(cond, out);
                    collect_top_level_call_fqns_in_block(body, out);
                }
                StmtKind::Return { value } => {
                    if let Some(value) = value.as_ref() {
                        collect_top_level_call_fqns_in_expr(value, out);
                    }
                }
                StmtKind::Empty
                | StmtKind::Break { .. }
                | StmtKind::Continue { .. }
                | StmtKind::Todo(_) => {}
            }
        }
    }

    fn find_top_level_call_in_block<'a>(block: &'a Block, fqn: &str) -> Option<&'a Expr> {
        block.stmts.iter().find_map(|stmt| match &stmt.kind {
            StmtKind::Expr(expr) => find_top_level_call_in_expr(expr, fqn),
            StmtKind::Val(val) => val
                .init
                .as_ref()
                .and_then(|expr| find_top_level_call_in_expr(expr, fqn)),
            StmtKind::Assign { lhs, rhs, .. } => find_top_level_call_in_expr(lhs, fqn)
                .or_else(|| find_top_level_call_in_expr(rhs, fqn)),
            StmtKind::While { cond, body } => find_top_level_call_in_expr(cond, fqn)
                .or_else(|| find_top_level_call_in_block(body, fqn)),
            StmtKind::Return { value } => value
                .as_ref()
                .and_then(|expr| find_top_level_call_in_expr(expr, fqn)),
            StmtKind::Empty
            | StmtKind::Break { .. }
            | StmtKind::Continue { .. }
            | StmtKind::Todo(_) => None,
        })
    }

    fn find_top_level_call_in_expr<'a>(expr: &'a Expr, fqn: &str) -> Option<&'a Expr> {
        match &expr.kind {
            ExprKind::Call { callee, args } => {
                if let ExprKind::VarRef(ValueRef::TopLevel {
                    fqn: callee_fqn, ..
                }) = &callee.kind
                    && callee_fqn == fqn
                {
                    return Some(expr);
                }
                find_top_level_call_in_expr(callee, fqn).or_else(|| {
                    args.iter().find_map(|arg| match arg {
                        CallArg::Positional(expr) => find_top_level_call_in_expr(expr, fqn),
                        CallArg::Named { value, .. } => find_top_level_call_in_expr(value, fqn),
                    })
                })
            }
            ExprKind::MemberAccess { receiver, .. } => find_top_level_call_in_expr(receiver, fqn),
            ExprKind::When { subject, arms } => {
                find_top_level_call_in_expr(subject, fqn).or_else(|| {
                    arms.iter().find_map(|arm| {
                        arm.guard
                            .as_ref()
                            .and_then(|guard| find_top_level_call_in_expr(guard, fqn))
                            .or_else(|| find_top_level_call_in_expr(&arm.body, fqn))
                    })
                })
            }
            ExprKind::Block(block) => find_top_level_call_in_block(block, fqn),
            ExprKind::Closure(closure) => find_top_level_call_in_expr(&closure.body, fqn),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => find_top_level_call_in_expr(cond, fqn)
                .or_else(|| find_top_level_call_in_expr(then_branch, fqn))
                .or_else(|| {
                    else_branch
                        .as_deref()
                        .and_then(|expr| find_top_level_call_in_expr(expr, fqn))
                }),
            ExprKind::Unary { expr, .. }
            | ExprKind::TypeCheck { expr, .. }
            | ExprKind::Cast { expr, .. } => find_top_level_call_in_expr(expr, fqn),
            ExprKind::Binary { lhs, rhs, .. } => find_top_level_call_in_expr(lhs, fqn)
                .or_else(|| find_top_level_call_in_expr(rhs, fqn)),
            ExprKind::StructLit { fields, .. } => fields
                .iter()
                .find_map(|field| find_top_level_call_in_expr(&field.value, fqn)),
            ExprKind::TupleLit { elements } => elements
                .iter()
                .find_map(|element| find_top_level_call_in_expr(element, fqn)),
            ExprKind::InterpolatedString { parts, .. } => {
                parts.iter().find_map(|part| match part {
                    crate::hir::InterpolatedStringPart::Expr { expr } => {
                        find_top_level_call_in_expr(expr, fqn)
                    }
                    crate::hir::InterpolatedStringPart::Text { .. } => None,
                })
            }
            ExprKind::Perform { args, .. } => args.iter().find_map(|arg| match arg {
                CallArg::Positional(expr) => find_top_level_call_in_expr(expr, fqn),
                CallArg::Named { value, .. } => find_top_level_call_in_expr(value, fqn),
            }),
            ExprKind::Handle(handle) => find_top_level_call_in_block(&handle.body, fqn)
                .or_else(|| {
                    handle
                        .arms
                        .iter()
                        .find_map(|arm| find_top_level_call_in_expr(&arm.body, fqn))
                })
                .or_else(|| {
                    handle
                        .finally
                        .as_ref()
                        .and_then(|block| find_top_level_call_in_block(block, fqn))
                }),
            ExprKind::Literal(_)
            | ExprKind::VarRef(_)
            | ExprKind::UnresolvedIdent { .. }
            | ExprKind::ClassLiteral(_)
            | ExprKind::Missing
            | ExprKind::Todo(_) => None,
        }
    }

    fn lower_typed_single_source_file(sess: &Session, source: &SourceFile) -> LoweredHir {
        let mut ast = parse_file(source).unwrap();
        {
            let sources = [source];
            let mut files = [&mut ast];
            crate::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
                sess.sysroot(),
                &sources,
                &mut files,
            )
            .unwrap();
        }

        typecheck::check_file_headers(source, &ast).unwrap();
        typecheck::check_file_struct_decls(source, &ast).unwrap();

        let index = {
            let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
            for file in &sess.sysroot().files {
                unit.push((&file.source, &file.ast));
            }
            unit.push((source, &ast));
            Index::build(&unit).unwrap()
        };
        let headers = crate::resolve::check_file_headers(source, &ast, &index).unwrap();
        crate::resolve::check_file_bodies(source, &mut ast, &index, &headers).unwrap();

        let mut env = typecheck::TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();
        env.extend_from_file(source, &ast, &index).unwrap();

        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();

        typecheck::check_file_annotations(
            source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_properties(source, &ast, &index, &env).unwrap();
        typecheck::check_file_inheritance(source, &ast, &index).unwrap();
        typecheck::check_file_interfaces(source, &ast, &index, &env).unwrap();
        typecheck::check_file_override_effects(
            source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_type_refs(
            source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_where_clauses(
            source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_overload_conflicts(
            source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_exprs(
            source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_type_layouts(&index, &env, &mut types, builtins).unwrap();

        let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for file in &sess.sysroot().files {
            unit.push((&file.source, &file.ast));
        }
        unit.push((source, &ast));

        lower_for_compilation_unit_multi_files(
            source,
            &index,
            &unit,
            &[(source, &ast)],
            &[],
            &types,
        )
        .unwrap()
    }

    fn refactor_session() -> Session {
        Session::with_options(SessionOptions::new()).unwrap()
    }

    fn find_fun<'a>(lowered: &'a LoweredHir, fqn: &str) -> &'a FunDecl {
        lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == fqn => Some(fun),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected HIR fun {fqn}"))
    }

    #[test]
    fn refactor_hir_collects_scoop_extern_abi_metadata() {
        let sess = refactor_session();
        let src = SourceFile::new_virtual(
            "<mem>/refactor_hir_collects_scoop_extern_abi_metadata.scoop",
            r#"
package fixtures.hir

import scoop.core.*

@Extern("managed_echo", abi = "scoop")
fun managedEcho(value: String): String
"#,
        );

        let lowered = lower_typed_single_source_file(&sess, &src);
        let extern_fun = lowered
            .extern_funs
            .get("fixtures.hir.managedEcho")
            .expect("expected extern fun side-table entry");

        assert_eq!(extern_fun.abi, crate::hir::ExternAbi::Scoop);
        assert_eq!(extern_fun.symbol, "managed_echo");
        assert_eq!(
            extern_fun.callable_abi_identity(),
            crate::hir::CallableAbiIdentity::ManagedExtern
        );
    }

    #[test]
    fn refactor_hir_comptime_expands_block_if_for_and_package_if() {
        let sess = refactor_session();
        let src = SourceFile::new_virtual(
            "<mem>/refactor_hir_comptime.scoop",
            r#"
package fixtures.hir_comptime

const val ENABLED: Bool = false

comptime if (true) {
    fun packageSelected(): Int { return 3 }
} else {
    fun packageSelected(): Int { return 4 }
}

fun fromBlock(): Int {
    comptime {
        return 7
    }
}

fun selectedBranch(): Int {
    comptime if (ENABLED) {
        return 1
    } else {
        return 2
    }
}

fun unrolled(): Int {
    var acc: Int = 0
    comptime for (i in 1..3) {
        acc = acc + 1
    }
    return acc
}

fun returnsIterationValue(): Int {
    comptime for (i in 1..3) {
        return i
    }
    return 0
}
"#,
        );

        let lowered = crate::pipeline::lower_typed_hir_for_dump(&sess, &src)
            .expect("refactor HIR stage should accept expanded comptime control flow");
        let dump = format!("{:#?}", lowered.file);
        assert!(
            !dump.contains("Todo"),
            "expanded HIR must not contain Todo: {dump}"
        );
        assert!(
            !dump.contains("comptime_"),
            "expanded HIR must not retain comptime placeholder reasons: {dump}"
        );

        let selected = find_fun(&lowered, "fixtures.hir_comptime.selectedBranch");
        let selected_body = selected.body.as_ref().expect("selectedBranch has body");
        let [selected_stmt] = selected_body.stmts.as_slice() else {
            panic!(
                "selectedBranch should contain exactly the chosen return, got {:?}",
                selected_body.stmts
            );
        };
        let StmtKind::Return { value: Some(expr) } = &selected_stmt.kind else {
            panic!("selectedBranch should lower to a return, got {selected_stmt:?}");
        };
        assert_eq!(src.slice(expr.span), "2");

        let package_selected = find_fun(&lowered, "fixtures.hir_comptime.packageSelected");
        let package_body = package_selected
            .body
            .as_ref()
            .expect("packageSelected has body");
        let [package_stmt] = package_body.stmts.as_slice() else {
            panic!(
                "packageSelected should contain one selected return, got {:?}",
                package_body.stmts
            );
        };
        let StmtKind::Return { value: Some(expr) } = &package_stmt.kind else {
            panic!("packageSelected should lower to a return, got {package_stmt:?}");
        };
        assert_eq!(src.slice(expr.span), "3");

        let from_block = find_fun(&lowered, "fixtures.hir_comptime.fromBlock");
        let from_block_body = from_block.body.as_ref().expect("fromBlock has body");
        assert!(
            matches!(
                from_block_body.stmts.as_slice(),
                [Stmt {
                    kind: StmtKind::Return { .. },
                    ..
                }]
            ),
            "comptime block should inline its generated return: {:?}",
            from_block_body.stmts
        );

        let unrolled = find_fun(&lowered, "fixtures.hir_comptime.unrolled");
        let unrolled_body = unrolled.body.as_ref().expect("unrolled has body");
        let assign_count = unrolled_body
            .stmts
            .iter()
            .filter(|stmt| matches!(stmt.kind, StmtKind::Assign { .. }))
            .count();
        assert_eq!(assign_count, 3, "comptime for should unroll three copies");

        let returns_iteration_value =
            find_fun(&lowered, "fixtures.hir_comptime.returnsIterationValue");
        let returns_body = returns_iteration_value
            .body
            .as_ref()
            .expect("returnsIterationValue has body");
        let first_return = returns_body
            .stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                StmtKind::Return { value: Some(expr) } => Some(expr),
                _ => None,
            })
            .expect("unrolled comptime for should emit returns");
        assert!(
            matches!(
                first_return.kind,
                ExprKind::Literal(LiteralKind::SynthInt(1))
            ),
            "comptime for binder should lower to a synthesized literal, got {:?}",
            first_return.kind
        );
    }

    #[test]
    fn refactor_hir_tail_if_uses_declared_return_type_hint() {
        let sess = refactor_session();
        let src = SourceFile::new_virtual(
            "<mem>/refactor_hir_tail_if_return_expected.scoop",
            r#"
fun main(): Int {
    if (true) { 3 } else { 1 }
}
"#,
        );

        let lowered = crate::pipeline::lower_typed_hir_for_dump(&sess, &src)
            .expect("refactor HIR stage should lower a tail if with declared return type");
        let main = find_fun(&lowered, "main");
        let body = main.body.as_ref().expect("main has body");
        assert_eq!(
            body.ty, main.return_ty,
            "body type should match function return type"
        );

        let [
            Stmt {
                kind: StmtKind::Expr(expr),
                ..
            },
        ] = body.stmts.as_slice()
        else {
            panic!(
                "main should contain a single tail expr stmt, got {:?}",
                body.stmts
            );
        };
        let ExprKind::If {
            then_branch,
            else_branch,
            ..
        } = &expr.kind
        else {
            panic!("main tail should remain an if expr, got {expr:?}");
        };

        assert_eq!(
            expr.ty, main.return_ty,
            "if expr should inherit function return type"
        );
        assert_eq!(
            then_branch.ty, main.return_ty,
            "then branch should stay Int"
        );
        assert_eq!(
            else_branch.as_deref().expect("if has else branch").ty,
            main.return_ty,
            "else branch should stay Int"
        );
    }

    fn lower_typed_single_source_file_via_mir_instance_collection(
        sess: &Session,
        source: &SourceFile,
    ) -> LoweredHir {
        let mut ast = parse_file(source).unwrap();
        {
            let sources = [source];
            let mut files = [&mut ast];
            crate::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
                sess.sysroot(),
                &sources,
                &mut files,
            )
            .unwrap();
        }

        typecheck::check_file_headers(source, &ast).unwrap();
        typecheck::check_file_struct_decls(source, &ast).unwrap();

        let index = {
            let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
            for file in &sess.sysroot().files {
                unit.push((&file.source, &file.ast));
            }
            unit.push((source, &ast));
            Index::build(&unit).unwrap()
        };
        let headers = crate::resolve::check_file_headers(source, &ast, &index).unwrap();
        crate::resolve::check_file_bodies(source, &mut ast, &index, &headers).unwrap();

        let mut env = typecheck::TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();
        env.extend_from_file(source, &ast, &index).unwrap();

        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();

        typecheck::check_file_annotations(
            source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_properties(source, &ast, &index, &env).unwrap();
        typecheck::check_file_inheritance(source, &ast, &index).unwrap();
        typecheck::check_file_interfaces(source, &ast, &index, &env).unwrap();
        typecheck::check_file_override_effects(
            source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_type_refs(
            source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_where_clauses(
            source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_overload_conflicts(
            source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_exprs(
            source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_type_layouts(&index, &env, &mut types, builtins).unwrap();

        let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for file in &sess.sysroot().files {
            unit.push((&file.source, &file.ast));
        }
        unit.push((source, &ast));

        lower_for_compilation_unit_multi_files_via_mir_instance_collection(
            &index,
            &unit,
            &[(source, &ast)],
            &[],
            Some(&env),
            &types,
        )
        .unwrap()
    }

    #[test]
    fn compilation_unit_via_mir_instances_materializes_non_intrinsic_direct_call_targets() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<t5000e3d_via_mir_instances>",
            r#"
package fixtures.t5000e3d

import scoop.core.*

fun <T> id(x: T): T { return x }

object Box {
    fun <T> memberId(x: T): T { return id(x) }
}

fun main(): Int {
    val a: Int = id(1)
    val b: Int = Box.memberId(2)
    return a + b
}
"#,
        );

        let lowered = lower_typed_single_source_file_via_mir_instance_collection(&sess, &source);

        let main = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.t5000e3d.main" => Some(fun),
                _ => None,
            })
            .expect("应收集到 fixtures.t5000e3d.main");
        let main_body = main.body.as_ref().expect("main 应有 body");
        assert!(
            find_top_level_call_in_block(main_body, "fixtures.t5000e3d.id::<Int>").is_some(),
            "via-mir compilation-unit lowering 应把 standalone generic direct-call 物化为实例目标"
        );
        assert!(
            find_top_level_call_in_block(main_body, "fixtures.t5000e3d.Box.memberId::<Int>")
                .is_some(),
            "via-mir compilation-unit lowering 应把 generic member direct-call 物化为实例目标"
        );

        let member_fun = lowered
            .member_funs
            .iter()
            .find(|fun| fun.fqn == "fixtures.t5000e3d.Box.memberId::<Int>")
            .expect("应收集到 Box.memberId::<Int> 单态成员实例");
        let member_body = member_fun.body.as_ref().expect("memberId::<Int> 应有 body");
        assert!(
            find_top_level_call_in_block(member_body, "fixtures.t5000e3d.id::<Int>").is_some(),
            "单态成员实例体内的 generic direct-call 应直接指向已实例化 target"
        );
    }

    #[test]
    fn compilation_unit_via_mir_instances_keeps_overloaded_generic_identity_distinct() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<t5000e3d_overload_identity>",
            r#"
package fixtures.t5000e3d

import scoop.core.*

fun <T> pick(x: T): T { return x }
fun <T> pick(x: T, y: T): T { return y }

object Box {
    fun <T> pick(x: T): T { return x }
    fun <T> pick(x: T, y: T): T { return y }
}

fun main(): Int {
    val a: Int = pick(1)
    val b: Int = pick(1, 2)
    val c: Int = Box.pick(3)
    val d: Int = Box.pick(3, 4)
    val f: (Int) -> Int = pick<Int>
    val e: Int = f(5)
    return a + b + c + d + e
}
"#,
        );

        let lowered = lower_typed_single_source_file_via_mir_instance_collection(&sess, &source);

        let top_level_pick_instances = lowered
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fun(fun)
                    if fun.fqn.starts_with("fixtures.t5000e3d.pick::<Int>")
                        && fun.fqn != "fixtures.t5000e3d.main" =>
                {
                    Some((fun.fqn.clone(), fun.params.len()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let top_level_pick_fqns = top_level_pick_instances
            .iter()
            .map(|(fqn, _)| fqn.clone())
            .collect::<HashSet<_>>();
        assert_eq!(
            top_level_pick_fqns.len(),
            2,
            "via-mir lowering 应为同名 generic overload 的相同 Int 实例保留两个不同的顶层实例名"
        );
        let unary_pick_fqn = top_level_pick_instances
            .iter()
            .find_map(|(fqn, param_count)| (*param_count == 1).then_some(fqn.clone()))
            .expect("应收集到 unary pick::<Int> 实例");
        let binary_pick_fqn = top_level_pick_instances
            .iter()
            .find_map(|(fqn, param_count)| (*param_count == 2).then_some(fqn.clone()))
            .expect("应收集到 binary pick::<Int> 实例");
        assert_ne!(
            unary_pick_fqn, binary_pick_fqn,
            "两个 overload 的实例名必须保持 distinct identity"
        );

        let member_pick_fqns = lowered
            .member_funs
            .iter()
            .filter(|fun| fun.fqn.starts_with("fixtures.t5000e3d.Box.pick::<Int>"))
            .map(|fun| fun.fqn.clone())
            .collect::<HashSet<_>>();
        assert_eq!(
            member_pick_fqns.len(),
            2,
            "via-mir lowering 应为同名 generic member overload 的相同 Int 实例保留两个不同的成员实例名"
        );

        let main = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.t5000e3d.main" => Some(fun),
                _ => None,
            })
            .expect("应收集到 fixtures.t5000e3d.main");
        let main_body = main.body.as_ref().expect("main 应有 body");
        let mut main_call_fqns = Vec::new();
        collect_top_level_call_fqns_in_block(main_body, &mut main_call_fqns);
        let direct_top_level_calls = main_call_fqns
            .iter()
            .filter(|fqn| fqn.starts_with("fixtures.t5000e3d.pick::<Int>"))
            .cloned()
            .collect::<HashSet<_>>();
        assert_eq!(
            direct_top_level_calls.len(),
            2,
            "main 里的 direct-call target 应区分两个 overloaded top-level generic 实例"
        );
        let direct_member_calls = main_call_fqns
            .iter()
            .filter(|fqn| fqn.starts_with("fixtures.t5000e3d.Box.pick::<Int>"))
            .cloned()
            .collect::<HashSet<_>>();
        assert_eq!(
            direct_member_calls.len(),
            2,
            "main 里的 direct-call target 应区分两个 overloaded generic member 实例"
        );

        let fun_value_init = main_body
            .stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                StmtKind::Val(ValDecl {
                    name: Some(name),
                    init: Some(init),
                    ..
                }) if name == "f" => Some(init),
                _ => None,
            })
            .expect("main 应包含函数值绑定 f");
        let ExprKind::Closure(closure) = &fun_value_init.kind else {
            panic!(
                "generic top-level function value 应被 lower 为 closure，实际为 {:?}",
                fun_value_init.kind
            );
        };
        let mut closure_call_fqns = Vec::new();
        collect_top_level_call_fqns_in_expr(&closure.body, &mut closure_call_fqns);
        assert!(
            closure_call_fqns.contains(&unary_pick_fqn),
            "函数值 closure 体应调用 unary overload 的 overload-aware 实例 target"
        );
        assert!(
            !closure_call_fqns.contains(&binary_pick_fqn),
            "函数值 closure 体不应误调用 binary overload 的实例 target"
        );
    }

    #[test]
    fn compilation_unit_via_mir_instances_keeps_overloaded_generic_identity_path_stable() {
        let sess = Session::new().unwrap();
        let program = r#"
package fixtures.t5000e3d

import scoop.core.*

fun <T> pick(x: T): T { return x }
fun <T> pick(x: T, y: T): T { return y }

object Box {
    fun <T> pick(x: T): T { return x }
    fun <T> pick(x: T, y: T): T { return y }
}

fun main(): Int {
    val a: Int = pick(1)
    val b: Int = pick(1, 2)
    val c: Int = Box.pick(3)
    val d: Int = Box.pick(3, 4)
    return a + b + c + d
}
"#;
        let source_a = SourceFile::new_virtual(
            "/tmp/root-a/fixtures/t5000e3d_overload_identity.scoop",
            program,
        );
        let source_b = SourceFile::new_virtual(
            "/tmp/root-b/fixtures/t5000e3d_overload_identity.scoop",
            program,
        );

        let lowered_a =
            lower_typed_single_source_file_via_mir_instance_collection(&sess, &source_a);
        let lowered_b =
            lower_typed_single_source_file_via_mir_instance_collection(&sess, &source_b);

        let top_level_instances = |lowered: &LoweredHir| {
            lowered
                .file
                .items
                .iter()
                .filter_map(|item| match item {
                    Item::Fun(fun)
                        if fun.fqn.starts_with("fixtures.t5000e3d.pick::<Int>")
                            && fun.fqn != "fixtures.t5000e3d.main" =>
                    {
                        Some(fun.fqn.clone())
                    }
                    _ => None,
                })
                .collect::<HashSet<_>>()
        };
        let member_instances = |lowered: &LoweredHir| {
            lowered
                .member_funs
                .iter()
                .filter(|fun| fun.fqn.starts_with("fixtures.t5000e3d.Box.pick::<Int>"))
                .map(|fun| fun.fqn.clone())
                .collect::<HashSet<_>>()
        };
        let main_direct_targets = |lowered: &LoweredHir| {
            let main = lowered
                .file
                .items
                .iter()
                .find_map(|item| match item {
                    Item::Fun(fun) if fun.fqn == "fixtures.t5000e3d.main" => Some(fun),
                    _ => None,
                })
                .expect("应收集到 fixtures.t5000e3d.main");
            let main_body = main.body.as_ref().expect("main 应有 body");
            let mut call_fqns = Vec::new();
            collect_top_level_call_fqns_in_block(main_body, &mut call_fqns);
            call_fqns
                .into_iter()
                .filter(|fqn| {
                    fqn.starts_with("fixtures.t5000e3d.pick::<Int>")
                        || fqn.starts_with("fixtures.t5000e3d.Box.pick::<Int>")
                })
                .collect::<HashSet<_>>()
        };

        assert_eq!(
            top_level_instances(&lowered_a),
            top_level_instances(&lowered_b),
            "不同源码根路径下的 top-level generic overload 实例名应保持一致"
        );
        assert_eq!(
            member_instances(&lowered_a),
            member_instances(&lowered_b),
            "不同源码根路径下的 generic member overload 实例名应保持一致"
        );
        assert_eq!(
            main_direct_targets(&lowered_a),
            main_direct_targets(&lowered_b),
            "不同源码根路径下的 overload-aware direct-call target 应保持一致"
        );
    }

    #[test]
    fn typed_hir_dump_keeps_generic_direct_calls_as_template_targets() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<t5000e3d_typed_dump>",
            r#"
package fixtures.t5000e3d

import scoop.core.*

fun <T> id(x: T): T { return x }

object Box {
    fun <T> memberId(x: T): T { return id(x) }
}

fun main(): Int {
    val a: Int = id(1)
    val b: Int = Box.memberId(2)
    return a + b
}
"#,
        );

        let lowered = lower_typed_for_dump(&sess, &source).unwrap();

        let main = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.t5000e3d.main" => Some(fun),
                _ => None,
            })
            .expect("应收集到 fixtures.t5000e3d.main");
        let main_body = main.body.as_ref().expect("main 应有 body");
        assert!(
            find_top_level_call_in_block(main_body, "fixtures.t5000e3d.id").is_some(),
            "typed dump 仍应保留 standalone generic direct-call 的 template target"
        );
        assert!(
            find_top_level_call_in_block(main_body, "fixtures.t5000e3d.id::<Int>").is_none(),
            "typed dump 不应提前把 standalone generic direct-call 物化为实例目标"
        );

        let member_fun = lowered
            .member_funs
            .iter()
            .find(|fun| fun.fqn == "fixtures.t5000e3d.Box.memberId")
            .expect("typed dump 应保留 generic member template");
        let member_body = member_fun
            .body
            .as_ref()
            .expect("memberId template 应有 body");
        assert!(
            find_top_level_call_in_block(member_body, "fixtures.t5000e3d.id").is_some(),
            "typed dump 中 generic member template 体内的 direct-call 仍应指向 template target"
        );
        assert!(
            find_top_level_call_in_block(member_body, "fixtures.t5000e3d.id::<Int>").is_none(),
            "typed dump 不应提前把 generic member template 体内的 direct-call 物化为实例目标"
        );
    }

    #[test]
    fn lower_typed_single_source_file_preserves_inferred_fun_return_types() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<t5000e3c_inferred_return_ty>",
            r#"
package fixtures.t5000e3c

import scoop.core.*

class Box {
    fun value() { 1 }
}

fun helper() { 1 }

fun main() {}
"#,
        );

        let lowered = lower_typed_single_source_file(&sess, &source);

        let helper = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.t5000e3c.helper" => Some(fun),
                _ => None,
            })
            .expect("应收集到 helper");
        assert_eq!(lowered.types.display(helper.return_ty).to_string(), "Int");

        let main = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.t5000e3c.main" => Some(fun),
                _ => None,
            })
            .expect("应收集到 main");
        assert_eq!(lowered.types.display(main.return_ty).to_string(), "Unit");

        let value_method = lowered
            .member_funs
            .iter()
            .find(|fun| fun.fqn == "fixtures.t5000e3c.Box.value")
            .expect("应收集到 Box.value");
        assert_eq!(
            lowered.types.display(value_method.return_ty).to_string(),
            "Int"
        );
    }

    #[test]
    fn lower_typed_single_source_file_expands_irrefutable_top_level_pattern_into_hidden_subject() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<t4004b>",
            r#"
package fixtures.t4004b

import scoop.core.*

val (left, right) = (7, 9)

fun main(): Int {
    return left + right
}
"#,
        );

        let lowered = lower_typed_single_source_file(&sess, &source);
        let left_fqn = "fixtures.t4004b.left";
        let top_level_value_names = lowered
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Val(val) => val.name.as_deref(),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(top_level_value_names.contains(&"left"));
        assert!(top_level_value_names.contains(&"right"));

        let hidden_subjects = lowered
            .top_level_immutable_values
            .keys()
            .filter(|fqn| fqn.contains("__top_level_pattern_") && fqn.ends_with("__subject"))
            .cloned()
            .collect::<Vec<_>>();
        let hidden_checks = lowered
            .top_level_immutable_values
            .keys()
            .filter(|fqn| fqn.contains("__top_level_pattern_") && fqn.ends_with("__check"))
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(hidden_subjects.len(), 1);
        assert_eq!(hidden_checks.len(), 0);
        let hidden_subject = &hidden_subjects[0];

        let left = lowered
            .top_level_immutable_values
            .get(left_fqn)
            .expect("应收集到可见 binder left 的顶层 immutable value 记录");
        let init = left.init.as_ref().expect("binder 应带 initializer");
        let ExprKind::MemberAccess { receiver, member } = &init.kind else {
            panic!("irrefutable tuple binder initializer 应直接做成员访问提取");
        };
        assert!(
            matches!(
                &receiver.kind,
                ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) if fqn == hidden_subject
            ),
            "tuple binder 提取应复用隐藏 subject 顶层值"
        );
        assert_eq!(member.name, "_0");
    }

    fn assert_raise_runtime_error_effect_ty(types: &TypeStore, effect_ty: TypeId) {
        let TypeKind::Ref(RefTypeKind::Nominal(effect_nominal)) = types.kind(effect_ty) else {
            panic!(
                "effect_ty 应为 effect nominal，实际为 {:?}",
                types.kind(effect_ty)
            );
        };
        assert_eq!(effect_nominal.fqn, "scoop.core.Raise");
        assert_eq!(effect_nominal.args.len(), 1);

        match types.kind(effect_nominal.args[0]) {
            TypeKind::Ref(RefTypeKind::Nominal(arg_nominal))
            | TypeKind::Value(ValueTypeKind::Nominal(arg_nominal)) => {
                assert_eq!(arg_nominal.fqn, "scoop.core.RuntimeError");
            }
            other => panic!("Raise 的类型实参应为 RuntimeError，实际为 {:?}", other),
        }
    }

    fn assert_raise_int_effect_ty(types: &TypeStore, effect_ty: TypeId) {
        let TypeKind::Ref(RefTypeKind::Nominal(effect_nominal)) = types.kind(effect_ty) else {
            panic!(
                "effect_ty 应为 effect nominal，实际为 {:?}",
                types.kind(effect_ty)
            );
        };
        assert_eq!(effect_nominal.fqn, "scoop.core.Raise");
        assert_eq!(effect_nominal.args.len(), 1);
        assert!(
            matches!(
                types.kind(effect_nominal.args[0]),
                TypeKind::Value(ValueTypeKind::Int)
            ),
            "Raise 的类型实参应为 Int，实际为 {:?}",
            types.kind(effect_nominal.args[0])
        );
    }

    fn find_raise_perform_effect_ty_in_block(block: &Block) -> Option<TypeId> {
        block
            .stmts
            .iter()
            .find_map(find_raise_perform_effect_ty_in_stmt)
    }

    fn find_raise_perform_effect_ty_in_stmt(stmt: &Stmt) -> Option<TypeId> {
        match &stmt.kind {
            StmtKind::Expr(expr) => find_raise_perform_effect_ty_in_expr(expr),
            StmtKind::Val(val) => val
                .init
                .as_ref()
                .and_then(find_raise_perform_effect_ty_in_expr),
            StmtKind::Assign { lhs, rhs, .. } => find_raise_perform_effect_ty_in_expr(lhs)
                .or_else(|| find_raise_perform_effect_ty_in_expr(rhs)),
            StmtKind::While { cond, body } => find_raise_perform_effect_ty_in_expr(cond)
                .or_else(|| find_raise_perform_effect_ty_in_block(body)),
            StmtKind::Return { value } => value
                .as_ref()
                .and_then(find_raise_perform_effect_ty_in_expr),
            StmtKind::Empty
            | StmtKind::Break { .. }
            | StmtKind::Continue { .. }
            | StmtKind::Todo(_) => None,
        }
    }

    fn find_raise_perform_effect_ty_in_expr(expr: &Expr) -> Option<TypeId> {
        match &expr.kind {
            ExprKind::Perform { effect_ty, op, .. } if op.fqn == "scoop.core.Raise.raise" => {
                Some(*effect_ty)
            }
            ExprKind::Call { callee, args } => find_raise_perform_effect_ty_in_expr(callee)
                .or_else(|| {
                    args.iter().find_map(|arg| match arg {
                        CallArg::Positional(expr) => find_raise_perform_effect_ty_in_expr(expr),
                        CallArg::Named { value, .. } => find_raise_perform_effect_ty_in_expr(value),
                    })
                }),
            ExprKind::MemberAccess { receiver, .. } => {
                find_raise_perform_effect_ty_in_expr(receiver)
            }
            ExprKind::When { subject, arms } => find_raise_perform_effect_ty_in_expr(subject)
                .or_else(|| {
                    arms.iter().find_map(|arm| {
                        arm.guard
                            .as_ref()
                            .and_then(find_raise_perform_effect_ty_in_expr)
                            .or_else(|| find_raise_perform_effect_ty_in_expr(&arm.body))
                    })
                }),
            ExprKind::Block(block) => find_raise_perform_effect_ty_in_block(block),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => find_raise_perform_effect_ty_in_expr(cond)
                .or_else(|| find_raise_perform_effect_ty_in_expr(then_branch))
                .or_else(|| {
                    else_branch
                        .as_deref()
                        .and_then(find_raise_perform_effect_ty_in_expr)
                }),
            ExprKind::Unary { expr, .. }
            | ExprKind::TypeCheck { expr, .. }
            | ExprKind::Cast { expr, .. } => find_raise_perform_effect_ty_in_expr(expr),
            ExprKind::Binary { lhs, rhs, .. } => find_raise_perform_effect_ty_in_expr(lhs)
                .or_else(|| find_raise_perform_effect_ty_in_expr(rhs)),
            ExprKind::StructLit { fields, .. } => fields
                .iter()
                .find_map(|field| find_raise_perform_effect_ty_in_expr(&field.value)),
            ExprKind::TupleLit { elements } => elements
                .iter()
                .find_map(find_raise_perform_effect_ty_in_expr),
            ExprKind::InterpolatedString { parts, .. } => {
                parts.iter().find_map(|part| match part {
                    crate::hir::InterpolatedStringPart::Expr { expr } => {
                        find_raise_perform_effect_ty_in_expr(expr)
                    }
                    crate::hir::InterpolatedStringPart::Text { .. } => None,
                })
            }
            ExprKind::Perform { args, .. } => args.iter().find_map(|arg| match arg {
                CallArg::Positional(expr) => find_raise_perform_effect_ty_in_expr(expr),
                CallArg::Named { value, .. } => find_raise_perform_effect_ty_in_expr(value),
            }),
            ExprKind::Handle(handle) => find_raise_perform_effect_ty_in_block(&handle.body)
                .or_else(|| {
                    handle
                        .arms
                        .iter()
                        .find_map(|arm| find_raise_perform_effect_ty_in_expr(&arm.body))
                })
                .or_else(|| {
                    handle
                        .finally
                        .as_ref()
                        .and_then(find_raise_perform_effect_ty_in_block)
                }),
            ExprKind::Literal(_)
            | ExprKind::VarRef(_)
            | ExprKind::UnresolvedIdent { .. }
            | ExprKind::ClassLiteral(_)
            | ExprKind::Closure(_)
            | ExprKind::Missing
            | ExprKind::Todo(_) => None,
        }
    }

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

        let output = crate::pipeline::load_typed_hir_stage_output_for_dump(&sess, &file).unwrap();
        let actual = output.stable_dump();

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

        let output = crate::pipeline::load_typed_hir_stage_output_for_dump(&sess, &file).unwrap();
        let actual = output.stable_dump();

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

        let output = crate::pipeline::load_typed_hir_stage_output_for_dump(&sess, &file).unwrap();
        let actual = output.stable_dump();

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

        let output = crate::pipeline::load_typed_hir_stage_output_for_dump(&sess, &file).unwrap();
        let actual = output.stable_dump();

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

        let output = crate::pipeline::load_typed_hir_stage_output_for_dump(&sess, &file).unwrap();
        let actual = output.stable_dump();

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

        let output = crate::pipeline::load_typed_hir_stage_output_for_dump(&sess, &file).unwrap();
        let actual = output.stable_dump();

        let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/closure_capture_val.hir");
        let expected = std::fs::read_to_string(&golden_path).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn lower_float_literals_to_typed_hir_literals() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual(
            "<mem>",
            "package a\nfun main() { val a = 2.75; val b = 0.5f; val c = 1e3 }\n",
        );

        let lowered = lower_for_dump(&sess, &src).unwrap();
        let Item::Fun(fun) = &lowered.file.items[0] else {
            panic!("期望第一个 item 为函数");
        };
        let body = fun.body.as_ref().expect("main 应有函数体");
        let [stmt_a, stmt_b, stmt_c] = body.stmts.as_slice() else {
            panic!("期望 main 中包含三个 val 语句");
        };

        let StmtKind::Val(val_a) = &stmt_a.kind else {
            panic!("第一个语句应为 val");
        };
        let init_a = val_a.init.as_ref().expect("a 应有 initializer");
        assert!(matches!(
            init_a.kind,
            ExprKind::Literal(LiteralKind::Float64(value)) if value == 2.75
        ));
        assert_eq!(lowered.types.display(init_a.ty).to_string(), "Float64");

        let StmtKind::Val(val_b) = &stmt_b.kind else {
            panic!("第二个语句应为 val");
        };
        let init_b = val_b.init.as_ref().expect("b 应有 initializer");
        assert!(matches!(
            init_b.kind,
            ExprKind::Literal(LiteralKind::Float32(value)) if value == 0.5
        ));
        assert_eq!(lowered.types.display(init_b.ty).to_string(), "Float32");

        let StmtKind::Val(val_c) = &stmt_c.kind else {
            panic!("第三个语句应为 val");
        };
        let init_c = val_c.init.as_ref().expect("c 应有 initializer");
        assert!(matches!(
            init_c.kind,
            ExprKind::Literal(LiteralKind::Float64(value)) if value == 1000.0
        ));
        assert_eq!(lowered.types.display(init_c.ty).to_string(), "Float64");
    }

    #[test]
    fn lower_for_compilation_unit_multi_files_includes_non_entry_top_level_funs() {
        let sess = Session::new().unwrap();

        let src_lib = SourceFile::new_virtual(
            "<lib>",
            r#"
package fixtures.t1315a

import scoop.core.*

fun id(x: Int): Int { return x }
"#,
        );
        let src_main = SourceFile::new_virtual(
            "<main>",
            r#"
package fixtures.t1315a

import scoop.core.*

fun main(): Int { return id(1) }
"#,
        );

        let mut ast_lib = parse_file(&src_lib).unwrap();
        let mut ast_main = parse_file(&src_main).unwrap();

        let index = {
            let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
            for f in &sess.sysroot().files {
                pairs.push((&f.source, &f.ast));
            }
            pairs.push((&src_lib, &ast_lib));
            pairs.push((&src_main, &ast_main));
            Index::build(&pairs).unwrap()
        };

        let h_lib = crate::resolve::check_file_headers(&src_lib, &ast_lib, &index).unwrap();
        crate::resolve::check_file_bodies(&src_lib, &mut ast_lib, &index, &h_lib).unwrap();

        let h_main = crate::resolve::check_file_headers(&src_main, &ast_main, &index).unwrap();
        crate::resolve::check_file_bodies(&src_main, &mut ast_main, &index, &h_main).unwrap();

        let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &sess.sysroot().files {
            unit.push((&f.source, &f.ast));
        }
        unit.push((&src_lib, &ast_lib));
        unit.push((&src_main, &ast_main));

        let files_to_lower = vec![(&src_lib, &ast_lib), (&src_main, &ast_main)];
        let empty_types = TypeStore::new();
        let lowered = lower_for_compilation_unit_multi_files(
            &src_main,
            &index,
            &unit,
            &files_to_lower,
            &[],
            &empty_types,
        )
        .unwrap();

        let fun_fqns: HashSet<&str> = lowered
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fun(fun) => Some(fun.fqn.as_str()),
                _ => None,
            })
            .collect();

        assert!(fun_fqns.contains("fixtures.t1315a.id"));
        assert!(fun_fqns.contains("fixtures.t1315a.main"));
    }

    #[test]
    fn lower_for_compilation_unit_multi_files_preserves_non_entry_call_site_side_tables() {
        let sess = Session::new().unwrap();

        let src_helper = SourceFile::new_virtual(
            "<t4006r_helper>",
            r#"
package fixtures.t4006r

import scoop.core.*

class Box(val x: Int = 10, val y: Int = 20)

public fun helper_sum(): Int {
    val box = Box(y = 32)
    return box.x + box.y
}

public fun resume_once(k: Continuation<Int, Unit, eff Pure>): Unit / Raise<RuntimeError> {
    k.resume(1)
}
"#,
        );
        let src_main = SourceFile::new_virtual(
            "<t4006r_main>",
            r#"
package fixtures.t4006r

import scoop.core.*

fun main(): Int {
    return helper_sum()
}
"#,
        );

        let mut ast_helper = parse_file(&src_helper).unwrap();
        let mut ast_main = parse_file(&src_main).unwrap();

        typecheck::check_file_headers(&src_helper, &ast_helper).unwrap();
        typecheck::check_file_struct_decls(&src_helper, &ast_helper).unwrap();
        typecheck::check_file_headers(&src_main, &ast_main).unwrap();
        typecheck::check_file_struct_decls(&src_main, &ast_main).unwrap();

        let index = {
            let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
            for f in &sess.sysroot().files {
                unit.push((&f.source, &f.ast));
            }
            unit.push((&src_helper, &ast_helper));
            unit.push((&src_main, &ast_main));
            Index::build(&unit).unwrap()
        };

        let headers_helper =
            crate::resolve::check_file_headers(&src_helper, &ast_helper, &index).unwrap();
        crate::resolve::check_file_bodies(&src_helper, &mut ast_helper, &index, &headers_helper)
            .unwrap();

        let headers_main =
            crate::resolve::check_file_headers(&src_main, &ast_main, &index).unwrap();
        crate::resolve::check_file_bodies(&src_main, &mut ast_main, &index, &headers_main).unwrap();

        let mut env = typecheck::TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();
        env.extend_from_file(&src_helper, &ast_helper, &index)
            .unwrap();
        env.extend_from_file(&src_main, &ast_main, &index).unwrap();

        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();

        for (source, ast, headers) in [
            (&src_helper, &ast_helper, &headers_helper),
            (&src_main, &ast_main, &headers_main),
        ] {
            typecheck::check_file_annotations(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .unwrap();
            typecheck::check_file_properties(source, ast, &index, &env).unwrap();
            typecheck::check_file_inheritance(source, ast, &index).unwrap();
            typecheck::check_file_interfaces(source, ast, &index, &env).unwrap();
            typecheck::check_file_override_effects(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .unwrap();
            typecheck::check_file_type_refs(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .unwrap();
            typecheck::check_file_where_clauses(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .unwrap();
            typecheck::check_file_overload_conflicts(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .unwrap();
            typecheck::check_file_exprs(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .unwrap();
        }

        typecheck::check_file_type_layouts(&index, &env, &mut types, builtins).unwrap();

        let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &sess.sysroot().files {
            unit.push((&f.source, &f.ast));
        }
        unit.push((&src_helper, &ast_helper));
        unit.push((&src_main, &ast_main));

        let lowered = lower_for_compilation_unit_multi_files(
            &src_main,
            &index,
            &unit,
            &[(&src_helper, &ast_helper), (&src_main, &ast_main)],
            &[],
            &types,
        )
        .unwrap();

        let helper_sum = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.t4006r.helper_sum" => Some(fun),
                _ => None,
            })
            .expect("应收集到 helper_sum");
        let helper_sum_body = helper_sum.body.as_ref().expect("helper_sum 应有 body");
        fn find_call_span_in_expr(expr: &Expr) -> Option<Span> {
            if let ExprKind::Call { .. } = &expr.kind {
                return Some(expr.span);
            }
            if let ExprKind::Block(block) = &expr.kind {
                return block.stmts.iter().find_map(find_call_span_in_stmt);
            }
            None
        }

        fn find_call_span_in_stmt(stmt: &Stmt) -> Option<Span> {
            match &stmt.kind {
                StmtKind::Expr(expr) => find_call_span_in_expr(expr),
                StmtKind::Val(val_decl) => val_decl.init.as_ref().and_then(find_call_span_in_expr),
                StmtKind::Return { value } => value.as_ref().and_then(find_call_span_in_expr),
                _ => None,
            }
        }

        let ctor_call_span = helper_sum_body
            .stmts
            .iter()
            .find_map(find_call_span_in_stmt)
            .expect("helper_sum 应包含 ctor call initializer");
        assert!(
            lowered.ctor_call_sites.contains_key(&CallSite::new(
                src_helper.path().to_path_buf(),
                ctor_call_span,
            )),
            "非入口文件中的 ctor 调用点应保留在 lowering side table 中"
        );

        let resume_once = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.t4006r.resume_once" => Some(fun),
                _ => None,
            })
            .expect("应收集到 resume_once");
        assert_eq!(
            lowered.types.display(resume_once.params[0].ty).to_string(),
            "scoop.core.Continuation<Int, Unit, eff Pure>"
        );
        let resume_body = resume_once.body.as_ref().expect("resume_once 应有 body");
        let resume_span = match resume_body.stmts.as_slice() {
            [
                Stmt {
                    kind: StmtKind::Expr(expr),
                    ..
                },
            ] => match &expr.kind {
                ExprKind::Call { .. } => expr.span,
                other => panic!("resume_once 的唯一语句应为 call expr，实际为 {:?}", other),
            },
            stmts => panic!("resume_once body 形态不符合预期: {:?}", stmts),
        };
        assert!(
            lowered
                .continuation_resume_call_sites
                .contains(&CallSite::new(src_helper.path().to_path_buf(), resume_span,)),
            "非入口文件中的 Continuation.resume 调用点应保留在 lowering side table 中"
        );
    }

    #[test]
    fn lower_for_compilation_unit_multi_files_preserves_safe_member_access_resolution() {
        let sess = Session::new().unwrap();
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/run-pass/safe_member_access_ref_and_extension_basic.scoop");
        let source = SourceFile::load(&fixture_path).unwrap();
        let mut ast = parse_file(&source).unwrap();
        {
            let sources = [&source];
            let mut files = [&mut ast];
            crate::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
                sess.sysroot(),
                &sources,
                &mut files,
            )
            .unwrap();
        }

        typecheck::check_file_headers(&source, &ast).unwrap();
        typecheck::check_file_struct_decls(&source, &ast).unwrap();

        let index = {
            let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
            for f in &sess.sysroot().files {
                unit.push((&f.source, &f.ast));
            }
            unit.push((&source, &ast));
            Index::build(&unit).unwrap()
        };
        let headers = crate::resolve::check_file_headers(&source, &ast, &index).unwrap();
        crate::resolve::check_file_bodies(&source, &mut ast, &index, &headers).unwrap();

        let mut env = typecheck::TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();
        env.extend_from_file(&source, &ast, &index).unwrap();

        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();

        typecheck::check_file_annotations(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_properties(&source, &ast, &index, &env).unwrap();
        typecheck::check_file_inheritance(&source, &ast, &index).unwrap();
        typecheck::check_file_interfaces(&source, &ast, &index, &env).unwrap();
        typecheck::check_file_override_effects(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_type_refs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_where_clauses(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_overload_conflicts(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_exprs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_type_layouts(&index, &env, &mut types, builtins).unwrap();

        let safe_debug = format!("{:?}", ast.safe_member_access_resolved.borrow());
        assert!(safe_debug.contains("User.score"), "{safe_debug}");
        assert!(safe_debug.contains("Config.port"), "{safe_debug}");
        assert!(safe_debug.contains("doubleScore"), "{safe_debug}");

        let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &sess.sysroot().files {
            unit.push((&f.source, &f.ast));
        }
        unit.push((&source, &ast));

        let lowered = lower_for_compilation_unit_multi_files(
            &source,
            &index,
            &unit,
            &[(&source, &ast)],
            &[],
            &types,
        )
        .unwrap();

        let mut unresolved_member_names = Vec::new();
        let mut top_level_call_fqns = Vec::new();
        for item in &lowered.file.items {
            if let Item::Fun(fun) = item
                && let Some(body) = fun.body.as_ref()
            {
                collect_unresolved_member_names_in_block(body, &mut unresolved_member_names);
                collect_top_level_call_fqns_in_block(body, &mut top_level_call_fqns);
            }
        }

        assert!(!unresolved_member_names.iter().any(|name| name == "score"));
        assert!(!unresolved_member_names.iter().any(|name| name == "port"));
        assert!(top_level_call_fqns.iter().any(|fqn| fqn == "doubleScore"));
    }

    #[test]
    fn lower_typed_single_source_file_preserves_chained_member_access_resolution() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<t4006v>",
            r#"
package fixtures.t4006v

import scoop.core.*

struct Tag(val label: String, val score: Int)

class Node(val name: String, val tag: Tag, val value: Int)

class Holder(val node: Node)

fun main(): Int {
    val holder: Holder = Holder(Node("root", Tag { label: "alpha", score: 7 }, 42))
    val label: String = holder.node.tag.label
    println(label)
    return holder.node.tag.score
}
"#,
        );

        let lowered = lower_typed_single_source_file(&sess, &source);
        let mut unresolved_member_names = Vec::new();
        for item in &lowered.file.items {
            if let Item::Fun(fun) = item
                && let Some(body) = fun.body.as_ref()
            {
                collect_unresolved_member_names_in_block(body, &mut unresolved_member_names);
            }
        }

        assert!(
            !unresolved_member_names.iter().any(|name| name == "label"),
            "{unresolved_member_names:?}"
        );
        assert!(
            !unresolved_member_names.iter().any(|name| name == "score"),
            "{unresolved_member_names:?}"
        );
    }

    #[test]
    fn lower_typed_single_source_file_expands_with_update_over_tuple_nested_paths() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<t4010a1>",
            r#"
package fixtures.t4010a1

import scoop.core.*

struct Point(val x: Int, val y: Int)

fun use(pair: (Point, (Int, Int))) {
    val updated: (Point, (Int, Int)) = pair with { _0.x: 10, _1._0: 30 }
}
"#,
        );

        let lowered = lower_typed_single_source_file(&sess, &source);
        let fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.t4010a1.use" => Some(fun),
                _ => None,
            })
            .expect("expected lowered function");
        let body = fun.body.as_ref().expect("expected function body");
        let updated_init = body
            .stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                StmtKind::Val(decl) if decl.name.as_deref() == Some("updated") => {
                    decl.init.as_ref()
                }
                _ => None,
            })
            .expect("expected updated init");

        assert!(
            !expr_contains_todo_kind(updated_init, "with_update"),
            "with-update lowering should not fall back to Todo: {updated_init:#?}"
        );

        let ExprKind::Block(block) = &updated_init.kind else {
            panic!("with-update init should lower to block: {updated_init:#?}");
        };
        assert_eq!(block.stmts.len(), 4, "{block:#?}");

        let StmtKind::Val(base_decl) = &block.stmts[0].kind else {
            panic!(
                "first with-update stmt should bind synthetic base: {:#?}",
                block.stmts[0]
            );
        };
        assert_eq!(base_decl.name.as_deref(), Some("$with_base"));

        let StmtKind::Expr(rebuilt_expr) = &block.stmts[3].kind else {
            panic!(
                "second with-update stmt should be rebuilt value: {:#?}",
                block.stmts[3]
            );
        };

        let ExprKind::TupleLit { elements } = &rebuilt_expr.kind else {
            panic!("with-update over tuple should rebuild tuple literal: {rebuilt_expr:#?}");
        };
        assert_eq!(elements.len(), 2);

        let ExprKind::StructLit { fields, .. } = &elements[0].kind else {
            panic!(
                "first tuple element should rebuild nested struct: {:#?}",
                elements[0]
            );
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "x");
        assert!(matches!(
            &fields[0].value.kind,
            ExprKind::VarRef(ValueRef::Local { name, .. }) if name.starts_with("__with_update_value")
        ));
        assert_eq!(fields[1].name, "y");
        assert!(matches!(
            fields[1].value.kind,
            ExprKind::MemberAccess { .. }
        ));

        let ExprKind::TupleLit {
            elements: nested_tuple,
        } = &elements[1].kind
        else {
            panic!(
                "second tuple element should rebuild nested tuple: {:#?}",
                elements[1]
            );
        };
        assert_eq!(nested_tuple.len(), 2);
        assert!(matches!(
            &nested_tuple[0].kind,
            ExprKind::VarRef(ValueRef::Local { name, .. }) if name.starts_with("__with_update_value")
        ));
        assert!(matches!(
            nested_tuple[1].kind,
            ExprKind::MemberAccess { .. }
        ));
    }

    #[test]
    fn lower_typed_single_source_file_expands_with_update_over_enum_payload_paths() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<t4010a2b>",
            r#"
package fixtures.t4010a2b

import scoop.core.*

struct Point(val x: Int, val y: Int)

enum Result {
    Ok(val point: Point),
    Err(val code: Int),
}

fun use(r: Result) {
    val updated: Result = r with { Ok.point.x: 10, Err.code: 20 }
}
"#,
        );

        let lowered = lower_typed_single_source_file(&sess, &source);
        let fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.t4010a2b.use" => Some(fun),
                _ => None,
            })
            .expect("expected lowered function");
        let body = fun.body.as_ref().expect("expected function body");
        let updated_init = body
            .stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                StmtKind::Val(decl) if decl.name.as_deref() == Some("updated") => {
                    decl.init.as_ref()
                }
                _ => None,
            })
            .expect("expected updated init");

        assert!(
            !expr_contains_todo_kind(updated_init, "with_update"),
            "enum with-update lowering should not fall back to Todo: {updated_init:#?}"
        );

        let ExprKind::Block(block) = &updated_init.kind else {
            panic!("enum with-update init should lower to block: {updated_init:#?}");
        };
        assert_eq!(block.stmts.len(), 4, "{block:#?}");

        let StmtKind::Expr(rebuilt_expr) = &block.stmts[3].kind else {
            panic!(
                "second with-update stmt should be rebuilt value: {:#?}",
                block.stmts[3]
            );
        };

        let ExprKind::When { subject, arms } = &rebuilt_expr.kind else {
            panic!("enum with-update should rebuild through when: {rebuilt_expr:#?}");
        };
        assert!(matches!(
            &subject.kind,
            ExprKind::VarRef(ValueRef::Local { name, .. }) if name == "$with_base"
        ));
        assert_eq!(arms.len(), 2);

        let ok_arm = arms
            .iter()
            .find(|arm| matches!(&arm.pat, WhenPat::Variant { name, .. } if name == "Ok"))
            .expect("expected Ok arm");
        let ExprKind::Call { callee, args } = &ok_arm.body.kind else {
            panic!("Ok arm should rebuild variant ctor: {:#?}", ok_arm.body);
        };
        assert!(matches!(
            &callee.kind,
            ExprKind::UnresolvedIdent { name } if name == "Ok"
        ));
        assert_eq!(args.len(), 1);
        let CallArg::Positional(ok_payload) = &args[0] else {
            panic!("Ok arm payload should be positional: {:#?}", args[0]);
        };
        let ExprKind::StructLit { fields, .. } = &ok_payload.kind else {
            panic!("Ok payload should rebuild nested Point: {ok_payload:#?}");
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "x");
        assert!(matches!(
            &fields[0].value.kind,
            ExprKind::VarRef(ValueRef::Local { name, .. }) if name.starts_with("__with_update_value")
        ));
        assert_eq!(fields[1].name, "y");
        assert!(matches!(
            fields[1].value.kind,
            ExprKind::MemberAccess { .. }
        ));

        let err_arm = arms
            .iter()
            .find(|arm| matches!(&arm.pat, WhenPat::Variant { name, .. } if name == "Err"))
            .expect("expected Err arm");
        let ExprKind::Call { callee, args } = &err_arm.body.kind else {
            panic!("Err arm should rebuild variant ctor: {:#?}", err_arm.body);
        };
        assert!(matches!(
            &callee.kind,
            ExprKind::UnresolvedIdent { name } if name == "Err"
        ));
        assert_eq!(args.len(), 1);
        let CallArg::Positional(err_payload) = &args[0] else {
            panic!("Err arm payload should be positional: {:#?}", args[0]);
        };
        assert!(matches!(
            &err_payload.kind,
            ExprKind::VarRef(ValueRef::Local { name, .. }) if name.starts_with("__with_update_value")
        ));
    }

    #[test]
    fn lower_typed_single_source_file_rewrites_value_computed_property_access_to_getter_call() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<t4010b1>",
            r#"
package fixtures.t4010b1

import scoop.core.*

struct Point(val x: Int) {
    val doubled: Int
        get() = this.x * 2
}

fun use() {
    val result: Int = Point(3).doubled
}
"#,
        );

        let lowered = lower_typed_single_source_file(&sess, &source);

        let find_result_init = |fun_fqn: &str| {
            let fun = lowered
                .file
                .items
                .iter()
                .find_map(|item| match item {
                    Item::Fun(fun) if fun.fqn == fun_fqn => Some(fun),
                    _ => None,
                })
                .expect("expected lowered function");
            let body = fun.body.as_ref().expect("expected function body");
            body.stmts
                .iter()
                .find_map(|stmt| match &stmt.kind {
                    StmtKind::Val(decl) if decl.name.as_deref() == Some("result") => {
                        decl.init.as_ref()
                    }
                    _ => None,
                })
                .expect("expected result initializer")
        };

        let point_result_init = find_result_init("fixtures.t4010b1.use");
        let ExprKind::Call {
            callee: point_callee,
            args: point_args,
        } = &point_result_init.kind
        else {
            panic!(
                "value computed property access should lower to getter call: {point_result_init:#?}"
            );
        };
        let ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) = &point_callee.kind else {
            panic!("getter callee should be top-level value ref: {point_callee:#?}");
        };
        assert_eq!(fqn, "fixtures.t4010b1.Point.doubled");
        assert_eq!(point_args.len(), 1);

        assert!(
            lowered
                .member_funs
                .iter()
                .any(|fun| fun.fqn == "fixtures.t4010b1.Point.doubled"),
            "expected non-generic computed property getter to be collected as member callable"
        );
    }

    #[test]
    fn via_mir_instance_collection_materializes_generic_value_property_getter_target() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<t4010b1a>",
            r#"
package fixtures.t4010b1a

import scoop.core.*

struct Box<T>(val value: T) {
    val readBack: T
        get() = this.value
}

fun main(): Int {
    val result: Int = Box(2).readBack
    return result
}
"#,
        );

        let lowered = lower_typed_single_source_file_via_mir_instance_collection(&sess, &source);

        let main = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.t4010b1a.main" => Some(fun),
                _ => None,
            })
            .expect("应收集到 fixtures.t4010b1a.main");
        let body = main.body.as_ref().expect("main 应有 body");
        let result_init = body
            .stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                StmtKind::Val(decl) if decl.name.as_deref() == Some("result") => decl.init.as_ref(),
                _ => None,
            })
            .expect("main 应包含 result 初始化");
        let ExprKind::Call { callee, args } = &result_init.kind else {
            panic!("generic getter access 应 lowering 成 direct call: {result_init:#?}");
        };
        let ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) = &callee.kind else {
            panic!("generic getter callee 应为 top-level value ref: {callee:#?}");
        };
        assert_eq!(fqn, "fixtures.t4010b1a.Box.readBack::<Int>");
        assert_eq!(args.len(), 1);

        let getter = lowered
            .member_funs
            .iter()
            .find(|fun| fun.fqn == "fixtures.t4010b1a.Box.readBack::<Int>")
            .expect("应收集到具体化后的 getter 实例");
        assert!(matches!(
            lowered.types.kind(getter.return_ty),
            crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::Int)
        ));
    }

    #[test]
    fn lower_for_compilation_unit_multi_files_preserves_effect_ty_in_class_init_side_tables() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<t3014b>",
            r#"
package fixtures.t3014b

import scoop.core.*

class BoomClass() {
    val x: Int = Raise.raise(RuntimeError.NullAssertionFailed)
}

fun main(): Int { return 0 }
"#,
        );

        let lowered = lower_typed_single_source_file(&sess, &source);

        let class_init = lowered
            .class_inits
            .get("fixtures.t3014b.BoomClass")
            .expect("应收集到 BoomClass 的 class init");
        let class_property_init = class_init
            .steps
            .iter()
            .find_map(|step| match step {
                ClassInitStep::PropertyInit { field_fqn, init }
                    if field_fqn == "fixtures.t3014b.BoomClass.x" =>
                {
                    Some(init)
                }
                _ => None,
            })
            .expect("BoomClass.x 应存在 property initializer");
        let class_effect_ty = match &class_property_init.kind {
            ExprKind::Perform { effect_ty, op, .. } => {
                assert_eq!(op.fqn, "scoop.core.Raise.raise");
                *effect_ty
            }
            other => panic!(
                "BoomClass.x initializer 应 lower 为 Perform，实际为 {:?}",
                other
            ),
        };
        assert_raise_runtime_error_effect_ty(&lowered.types, class_effect_ty);
    }

    #[test]
    fn lower_typed_single_source_file_preserves_effect_ty_in_observable_delegate_callback() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<t3014c>",
            r#"
package fixtures.t3014c

import scoop.core.*
import scoop.delegates.*

class Counter() {
    var x: Int by observable(0) { old, new ->
        if (new == 1) {
            Raise.raise(7)
        }
        println(old)
    }
}

fun main(): Int {
    val counter: Counter = Counter()
    try {
        counter.x = 1
    } catch (e: Int) {
        println(e)
    }
    return 0
}
"#,
        );

        let lowered = lower_typed_single_source_file(&sess, &source);
        let main_fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.t3014c.main" => Some(fun),
                _ => None,
            })
            .expect("应收集到 fixtures.t3014c.main");
        let main_body = main_fun.body.as_ref().expect("main 应有 body");
        let effect_ty = find_raise_perform_effect_ty_in_block(main_body)
            .expect("observable callback 内的 Raise.raise 应被 lower 为带 effect_ty 的 Perform");
        assert_raise_int_effect_ty(&lowered.types, effect_ty);
    }

    #[test]
    fn lower_typed_single_source_file_records_statement_position_continuation_resume_call_site() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<t3016c0_stmt_resume>",
            r#"
package fixtures.t3016c0

import scoop.core.*

fun run(k: Continuation<Int, Unit, eff Pure>): Unit / Raise<RuntimeError> {
    k.resume(1)
}
"#,
        );

        let lowered = lower_typed_single_source_file(&sess, &source);
        let run_fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.t3016c0.run" => Some(fun),
                _ => None,
            })
            .expect("应收集到 fixtures.t3016c0.run");
        assert_eq!(
            lowered.types.display(run_fun.params[0].ty).to_string(),
            "scoop.core.Continuation<Int, Unit, eff Pure>"
        );
        let body = run_fun.body.as_ref().expect("run 应有 body");
        let resume_span = match body.stmts.as_slice() {
            [
                Stmt {
                    kind: StmtKind::Expr(expr),
                    ..
                },
            ] => match &expr.kind {
                ExprKind::Call { .. } => expr.span,
                other => panic!("run 的唯一语句应为 call expr，实际为 {:?}", other),
            },
            stmts => panic!("run body 语句数不符合预期: {:?}", stmts),
        };

        assert_eq!(lowered.continuation_resume_call_sites.len(), 1);
        assert!(
            lowered
                .continuation_resume_call_sites
                .contains(&CallSite::new(source.path().to_path_buf(), resume_span)),
            "statement-position `Continuation.resume(...)` 应写入 continuation_resume_call_sites"
        );
    }

    #[test]
    fn lower_typed_single_source_file_does_not_record_effect_op_named_resume_as_builtin_call_site()
    {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<t3016c0r_effect_op_resume>",
            r#"
package fixtures.t3016c0r

import scoop.core.*

effect Echo {
    fun resume(value: Int): Unit
}

fun run(): Unit / Echo {
    Echo.resume(1)
}
"#,
        );

        let lowered = lower_typed_single_source_file(&sess, &source);
        let run_fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.t3016c0r.run" => Some(fun),
                _ => None,
            })
            .expect("应收集到 fixtures.t3016c0r.run");
        let body = run_fun.body.as_ref().expect("run 应有 body");
        match body.stmts.as_slice() {
            [
                Stmt {
                    kind: StmtKind::Expr(expr),
                    ..
                },
            ] => match &expr.kind {
                ExprKind::Perform { .. } => {}
                other => panic!("effect op `resume` 应 lower 为 Perform，实际为 {:?}", other),
            },
            stmts => panic!("run body 语句数不符合预期: {:?}", stmts),
        }

        assert!(
            lowered.continuation_resume_call_sites.is_empty(),
            "effect op `resume` 不应污染 continuation_resume_call_sites"
        );
    }

    #[test]
    fn lower_for_compilation_unit_multi_files_preserves_effect_ty_in_cross_file_observable_delegate_callback()
     {
        let sess = Session::new().unwrap();
        let src_model = SourceFile::new_virtual(
            "<t3014cr_model>",
            r#"
package fixtures.t3014cr

import scoop.core.*
import scoop.delegates.*

class Counter() {
    var x: Int by observable(0) { old, new ->
        if (new == 1) {
            Raise.raise(7)
        }
        println(old)
    }
}
"#,
        );
        let src_main = SourceFile::new_virtual(
            "<t3014cr_main>",
            r#"
package fixtures.t3014cr

import scoop.core.*

fun main(): Int {
    val counter: Counter = Counter()
    try {
        counter.x = 1
    } catch (e: Int) {
        println(e)
    }
    return 0
}
"#,
        );

        let mut ast_model = parse_file(&src_model).unwrap();
        let mut ast_main = parse_file(&src_main).unwrap();

        let index = {
            let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
            for f in &sess.sysroot().files {
                unit.push((&f.source, &f.ast));
            }
            unit.push((&src_model, &ast_model));
            unit.push((&src_main, &ast_main));
            Index::build(&unit).unwrap()
        };

        let headers_model =
            crate::resolve::check_file_headers(&src_model, &ast_model, &index).unwrap();
        crate::resolve::check_file_bodies(&src_model, &mut ast_model, &index, &headers_model)
            .unwrap();
        let headers_main =
            crate::resolve::check_file_headers(&src_main, &ast_main, &index).unwrap();
        crate::resolve::check_file_bodies(&src_main, &mut ast_main, &index, &headers_main).unwrap();

        let mut env = typecheck::TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();
        env.extend_from_file(&src_model, &ast_model, &index)
            .unwrap();
        env.extend_from_file(&src_main, &ast_main, &index).unwrap();

        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();

        typecheck::check_file_annotations(
            &src_model,
            &ast_model,
            &index,
            &headers_model.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_properties(&src_model, &ast_model, &index, &env).unwrap();
        typecheck::check_file_inheritance(&src_model, &ast_model, &index).unwrap();
        typecheck::check_file_interfaces(&src_model, &ast_model, &index, &env).unwrap();
        typecheck::check_file_override_effects(
            &src_model,
            &ast_model,
            &index,
            &headers_model.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_type_refs(
            &src_model,
            &ast_model,
            &index,
            &headers_model.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_where_clauses(
            &src_model,
            &ast_model,
            &index,
            &headers_model.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_overload_conflicts(
            &src_model,
            &ast_model,
            &index,
            &headers_model.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_exprs(
            &src_model,
            &ast_model,
            &index,
            &headers_model.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_type_layouts(&index, &env, &mut types, builtins).unwrap();

        let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &sess.sysroot().files {
            unit.push((&f.source, &f.ast));
        }
        unit.push((&src_model, &ast_model));
        unit.push((&src_main, &ast_main));

        let lowered = lower_for_compilation_unit_multi_files(
            &src_main,
            &index,
            &unit,
            &[(&src_main, &ast_main)],
            &[],
            &types,
        )
        .unwrap();

        let main_fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.t3014cr.main" => Some(fun),
                _ => None,
            })
            .expect("应收集到 fixtures.t3014cr.main");
        let main_body = main_fun.body.as_ref().expect("main 应有 body");
        let effect_ty = find_raise_perform_effect_ty_in_block(main_body).expect(
            "跨文件 observable callback 内的 Raise.raise 应被 lower 为带 effect_ty 的 Perform",
        );
        assert_raise_int_effect_ty(&lowered.types, effect_ty);
    }

    #[test]
    fn typed_hir_keeps_effect_generic_member_type_apply_on_direct_call_path() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual(
            "<mem>/hir_member_effect_type_apply.scoop",
            r#"
package fixtures.hirreview

class Box {
    fun <eff E = Pure> forward(): Int / E {
        return 1
    }
}

fun <eff E = Pure> wrap(box: Box): Int / E {
    return box.forward<eff E>()
}
"#,
        );

        let lowered = lower_typed_for_dump(&sess, &src).unwrap();
        let wrap = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.hirreview.wrap" => Some(fun),
                _ => None,
            })
            .expect("应收集到 fixtures.hirreview.wrap");
        let body = wrap.body.as_ref().expect("wrap 应有函数体");
        let call_expr = body
            .stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                StmtKind::Return { value: Some(expr) } => Some(expr),
                _ => None,
            })
            .expect("wrap 应包含返回调用");
        let ExprKind::Call { callee, args } = &call_expr.kind else {
            panic!(
                "期望 wrap 返回值被 lower 为 direct call，实际为 {:?}",
                call_expr.kind
            );
        };
        assert_eq!(args.len(), 1, "成员 direct-call 应携带隐式 receiver 实参");
        match &callee.kind {
            ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) => {
                assert_eq!(fqn, "fixtures.hirreview.Box.forward");
            }
            other => panic!("期望 callee 已被降糖为顶层函数引用，实际为 {other:?}"),
        }
    }

    #[test]
    fn typed_hir_lowers_safe_member_type_apply_as_safe_direct_call() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual(
            "<mem>/hir_safe_member_effect_type_apply.scoop",
            r#"
package fixtures.hirreview

class Box {
    fun <eff E = Pure> forward(): Int / E {
        return 1
    }
}

fun <eff E = Pure> wrap(box: Box?): Int? / E {
    return box?.forward<eff E>()
}
"#,
        );

        let lowered = lower_typed_for_dump(&sess, &src)
            .expect("safe member direct-call + TypeApply 应能通过 typed HIR lowering");
        let wrap = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.hirreview.wrap" => Some(fun),
                _ => None,
            })
            .expect("应收集到 fixtures.hirreview.wrap");
        let body = wrap.body.as_ref().expect("wrap 应有函数体");
        let ret_expr = body
            .stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                StmtKind::Return { value: Some(expr) } => Some(expr),
                _ => None,
            })
            .expect("wrap 应包含返回表达式");
        let ExprKind::When { arms, .. } = &ret_expr.kind else {
            panic!(
                "safe member call 应被 lower 为 when desugar，实际为 {:?}",
                ret_expr.kind
            );
        };
        let some_arm = arms
            .iter()
            .find(|arm| matches!(&arm.pat, super::super::WhenPat::Variant { name, .. } if name == "Some"))
            .expect("safe call desugar 应包含 Some 分支");
        let ExprKind::Call {
            callee: some_callee,
            args: some_args,
        } = &some_arm.body.kind
        else {
            panic!(
                "Some 分支应包装 Some(inner_call)，实际为 {:?}",
                some_arm.body.kind
            );
        };
        match &some_callee.kind {
            ExprKind::UnresolvedIdent { name } => assert_eq!(name, "Some"),
            other => panic!("Some 分支外层应调用 Some(...)，实际为 {other:?}"),
        }
        let [CallArg::Positional(inner_call)] = some_args.as_slice() else {
            panic!("Some 分支应只包装一个 inner_call，实际为 {:?}", some_args);
        };
        let ExprKind::Call { callee, args } = &inner_call.kind else {
            panic!(
                "safe call 的 inner_call 应为 direct call，实际为 {:?}",
                inner_call.kind
            );
        };
        assert_eq!(
            args.len(),
            1,
            "safe member direct-call 仍应携带隐式 receiver 实参"
        );
        assert_eq!(
            some_arm.body.ty, ret_expr.ty,
            "Some 分支包装后的表达式应保留 `Int?` 结果类型"
        );
        match &callee.kind {
            ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) => {
                assert_eq!(fqn, "fixtures.hirreview.Box.forward");
            }
            other => panic!("inner_call 应已降糖为顶层函数引用，实际为 {other:?}"),
        }
        let none_arm = arms
            .iter()
            .find(|arm| matches!(&arm.pat, super::super::WhenPat::Variant { name, .. } if name == "None"))
            .expect("safe call desugar 应包含 None 分支");
        let ExprKind::Call {
            callee: none_callee,
            args: none_args,
        } = &none_arm.body.kind
        else {
            panic!(
                "None 分支应重建为 0 参 variant ctor 调用，实际为 {:?}",
                none_arm.body.kind
            );
        };
        assert!(
            none_args.is_empty(),
            "None 分支应重建为 `None()` 而不是保留 unresolved value"
        );
        assert_eq!(
            none_arm.body.ty, ret_expr.ty,
            "None 分支应保留与 safe call 一致的 `Int?` 结果类型"
        );
        match &none_callee.kind {
            ExprKind::UnresolvedIdent { name } => assert_eq!(name, "None"),
            other => panic!("None 分支应调用 None()，实际为 {other:?}"),
        }
    }

    #[test]
    fn typed_hir_lowers_companion_member_type_apply_as_direct_call() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual(
            "<mem>/hir_companion_member_effect_type_apply.scoop",
            r#"
package fixtures.hirreview

class Box {
    companion object {
        fun <eff E = Pure> forward(): Int / E {
            return 1
        }
    }
}

fun <eff E = Pure> wrap(): Int / E {
    return Box.forward<eff E>()
}
"#,
        );

        let lowered = lower_typed_for_dump(&sess, &src)
            .expect("companion member direct-call + TypeApply 应能通过 typed HIR lowering");
        let wrap = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.hirreview.wrap" => Some(fun),
                _ => None,
            })
            .expect("应收集到 fixtures.hirreview.wrap");
        let body = wrap.body.as_ref().expect("wrap 应有函数体");
        let call_expr = body
            .stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                StmtKind::Return { value: Some(expr) } => Some(expr),
                _ => None,
            })
            .expect("wrap 应包含返回调用");
        let ExprKind::Call { callee, args } = &call_expr.kind else {
            panic!(
                "期望 companion member 调用被 lower 为 direct call，实际为 {:?}",
                call_expr.kind
            );
        };
        assert_eq!(
            args.len(),
            1,
            "companion direct-call 应注入 companion receiver 实参"
        );
        match &callee.kind {
            ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) => {
                assert_eq!(fqn, "fixtures.hirreview.Box.Companion.forward");
            }
            other => panic!("期望 callee 已被降糖为 companion 顶层函数引用，实际为 {other:?}"),
        }
        let [CallArg::Positional(receiver)] = args.as_slice() else {
            panic!(
                "companion direct-call 应只注入一个 receiver，实际为 {:?}",
                args
            );
        };
        match &receiver.kind {
            ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) => {
                assert_eq!(fqn, "fixtures.hirreview.Box.Companion");
            }
            other => panic!("receiver 应为 companion object 单例值，实际为 {other:?}"),
        }
    }

    #[test]
    fn typed_hir_preserves_function_typed_nested_call_callee() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual(
            "<mem>/hir_nested_callable_call.scoop",
            r#"
package fixtures.hirreview

fun make(x: Int): () -> Int {
    return { x }
}

fun main(): Int {
    return make(1)()
}
"#,
        );

        let lowered = lower_typed_for_dump(&sess, &src)
            .expect("nested callable call should lower to typed HIR");
        let main = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.hirreview.main" => Some(fun),
                _ => None,
            })
            .expect("应收集到 fixtures.hirreview.main");
        let body = main.body.as_ref().expect("main 应有函数体");
        let call_expr = body
            .stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                StmtKind::Return { value: Some(expr) } => Some(expr),
                _ => None,
            })
            .expect("main 应包含返回调用");
        let ExprKind::Call { callee, .. } = &call_expr.kind else {
            panic!("期望外层表达式被 lower 为调用，实际为 {:?}", call_expr.kind);
        };
        assert!(
            matches!(
                lowered.types.kind(callee.ty),
                TypeKind::Ref(RefTypeKind::Function(_))
            ),
            "调用返回的 callable 在 typed HIR 中应保留函数类型，实际为 {}",
            lowered.types.display(callee.ty)
        );
    }

    #[test]
    fn typed_hir_top_level_immutable_receiver_closure_keeps_length_as_call_in_side_table() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual(
            "<mem>/hir_top_level_receiver_closure_side_table.scoop",
            r#"
import scoop.core.*

val topNamed: String.(Int) -> Int = { n: Int -> this.length() + n }
"#,
        );

        let lowered = lower_typed_for_dump(&sess, &src)
            .expect("top-level receiver closure should lower to typed HIR");
        let value = lowered
            .top_level_immutable_values
            .get("topNamed")
            .expect("topNamed 应进入 top_level_immutable_values side table");
        let init = value.init.as_ref().expect("topNamed 应有 initializer");
        let ExprKind::Closure(closure) = &init.kind else {
            panic!("topNamed initializer 应为 closure，实际为 {:?}", init.kind);
        };
        let ExprKind::Binary { lhs, .. } = &closure.body.kind else {
            panic!(
                "receiver closure body 应为 binary，实际为 {:?}",
                closure.body.kind
            );
        };
        let ExprKind::Call { callee, args } = &lhs.kind else {
            panic!("length 调用应保留为 Call，实际为 {:?}", lhs.kind);
        };
        assert!(args.is_empty(), "String.length() 不应携带实参: {args:?}");
        let ExprKind::MemberAccess { member, .. } = &callee.kind else {
            panic!(
                "length 调用 callee 应为 MemberAccess，实际为 {:?}",
                callee.kind
            );
        };
        assert_eq!(member.name, "length");
        assert!(
            matches!(
                lowered.types.kind(callee.ty),
                TypeKind::Ref(RefTypeKind::Function(_))
            ),
            "length callee 在 side table 中应是函数类型，实际为 {}",
            lowered.types.display(callee.ty)
        );
    }

    #[test]
    fn typed_hir_continuation_resume_unit_sugar_canonicalizes_zero_arg_and_explicit_unit() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual(
            "<mem>/hir_continuation_resume_unit_sugar.scoop",
            r#"
package fixtures.hirreview

import scoop.core.*

fun takesUnit(value: Unit): Unit {}

fun resumeZero(k: Continuation<Unit, Unit, eff Pure>): Unit / Raise<RuntimeError> {
    k.resume()
    k.resume(())
    takesUnit()
    takesUnit(())
}
"#,
        );

        let lowered = lower_typed_for_dump(&sess, &src).unwrap();
        let resume_zero = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.hirreview.resumeZero" => Some(fun),
                _ => None,
            })
            .expect("应收集到 fixtures.hirreview.resumeZero");
        let body = resume_zero.body.as_ref().expect("resumeZero 应有函数体");

        let call_exprs: Vec<&Expr> = body
            .stmts
            .iter()
            .filter_map(|stmt| match &stmt.kind {
                StmtKind::Expr(expr) => Some(expr),
                _ => None,
            })
            .collect();
        assert_eq!(call_exprs.len(), 4, "resumeZero 应包含 4 个调用表达式语句");

        for (index, call_expr) in call_exprs.iter().enumerate() {
            let ExprKind::Call { callee, args } = &call_expr.kind else {
                panic!("期望调用表达式，实际为 {:?}", call_expr.kind);
            };
            match index {
                0 | 1 => {
                    match &callee.kind {
                        ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) => {
                            assert_eq!(fqn, "scoop.core.Continuation.resume");
                        }
                        other => panic!("resume call 应 lower 为 direct call，实际为 {other:?}"),
                    }
                    let [CallArg::Positional(receiver), CallArg::Positional(arg)] = args.as_slice()
                    else {
                        panic!(
                            "typed HIR 应把 continuation.resume canonicalize 为 receiver + Unit 实参: {:?}",
                            args
                        );
                    };
                    assert!(
                        matches!(receiver.kind, ExprKind::VarRef(ValueRef::Local { .. })),
                        "resume direct-call 的第 0 个实参应为 receiver，实际为 {:?}",
                        receiver.kind
                    );
                    assert!(
                        matches!(arg.kind, ExprKind::Literal(LiteralKind::Unit)),
                        "canonicalized resume payload 应为显式 Unit literal，实际为 {:?}",
                        arg.kind
                    );
                }
                _ => {
                    let [CallArg::Positional(arg)] = args.as_slice() else {
                        panic!(
                            "typed HIR 应把 zero-arg Unit sugar canonicalize 为单个实参: {:?}",
                            args
                        );
                    };
                    assert!(
                        matches!(arg.kind, ExprKind::Literal(LiteralKind::Unit)),
                        "canonicalized Unit sugar 实参应为显式 Unit literal，实际为 {:?}",
                        arg.kind
                    );
                }
            }
        }
    }

    #[test]
    fn typed_hir_unit_single_param_zero_arg_call_canonicalizes_to_unit_literal() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual(
            "<mem>/hir_unit_single_param_zero_arg_call.scoop",
            r#"
package fixtures.hirreview

fun takesUnit(value: Unit): Unit {}

fun run(): Unit {
    takesUnit()
    takesUnit(())
}
"#,
        );

        let lowered = lower_typed_for_dump(&sess, &src).unwrap();
        let run = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.hirreview.run" => Some(fun),
                _ => None,
            })
            .expect("应收集到 fixtures.hirreview.run");
        let body = run.body.as_ref().expect("run 应有函数体");

        let call_exprs: Vec<&Expr> = body
            .stmts
            .iter()
            .filter_map(|stmt| match &stmt.kind {
                StmtKind::Expr(expr) => Some(expr),
                _ => None,
            })
            .collect();
        assert_eq!(call_exprs.len(), 2, "run 应包含 2 个调用表达式语句");

        for call_expr in call_exprs {
            let ExprKind::Call { args, .. } = &call_expr.kind else {
                panic!("期望调用表达式，实际为 {:?}", call_expr.kind);
            };
            let [CallArg::Positional(arg)] = args.as_slice() else {
                panic!(
                    "typed HIR 应把 `takesUnit()` canonicalize 为单个 Unit 实参: {:?}",
                    args
                );
            };
            assert!(
                matches!(arg.kind, ExprKind::Literal(LiteralKind::Unit)),
                "canonicalized Unit 实参应为显式 Unit literal，实际为 {:?}",
                arg.kind
            );
        }
    }
}
