//! 类型检查（AST + resolved symbols → typed / typed-HIR）。
//!
//! 本模块覆盖编译管线的 `typecheck` 阶段：把已 resolve 的声明头与函数体做双向
//! 类型检查与推断，产出 typed 信息（供后续 lowering）。设计见
//! `~/.claude/plans/calm-doodling-muffin.md`（~28 模块、M1–M8 里程碑）。
//!
//! 当前落地（Phase C 地基）：
//! - [`diagnostics`]：`scoop::typecheck::*` 诊断码与构造辅助；
//! - [`env`]：[`env::TypeEnv`]——类型存储 + 内建类型表 + 对 resolve [`Index`](crate::resolve::Index)
//!   的查询（nominal ref/value 等）；
//! - [`lower`]：[`lower::TypeLowering`]——`ast::TypeRef → TypeId`（类型引用降级）。
//!
//! 表达式 / 语句检查器（M1 起）随里程碑推进补齐。

pub mod diagnostics;
pub mod env;
pub mod expr;
pub mod extern_fn;
pub mod lower;
pub mod release_hook;

pub use env::TypeEnv;
pub use lower::TypeLowering;

use scoop2_base::Interner;
use scoop2_base::diag::DiagnosticSink;

/// 完整的 typecheck 管线（resolve → typecheck）。接收与 `resolve::run_program` 相同的
/// 输入，内部完成 header 收集 → import → 类型引用 → body 解析 → 扩展 → 类型检查。
pub fn run_typecheck(
    inputs: &[crate::resolve::InputFile],
    interner: &mut Interner,
    diags: &mut DiagnosticSink,
) {
    use crate::resolve::{
        ConeKind, Index, InputOrigin, Resolution, body, collect, imports, type_refs,
    };
    use crate::syntax::ast::File;

    // ---- Phase 1：收集所有 header ----
    let mut index = Index::new();
    for inp in inputs {
        let cone_name = collect::package_prefix_of(inp.file, interner);
        let cone_kind = match inp.origin {
            InputOrigin::User => ConeKind::Bin,
            InputOrigin::Sysroot => ConeKind::Syslib,
        };
        let cone = if cone_name.is_empty() {
            let fallback = match inp.origin {
                InputOrigin::User => "<user>",
                InputOrigin::Sysroot => "<sysroot>",
            };
            index.intern_cone(fallback, cone_kind)
        } else {
            index.intern_cone(&cone_name, cone_kind)
        };
        collect::collect_file(inp.file, inp.file_id, cone, &mut index, interner, diags);
    }
    index.resolve_extensions(interner);

    // ---- Phase 2：解析用户文件 ----
    struct UserFile<'a> {
        file: &'a File,
        prefix: String,
        imports: imports::ImportTable,
        resolution: Resolution,
        trusted: bool,
    }
    let mut user_files: Vec<UserFile> = Vec::new();
    for inp in inputs.iter().filter(|i| i.origin == InputOrigin::User) {
        let prefix = collect::package_prefix_of(inp.file, interner);
        let imports = imports::ImportTable::collect(inp.file, inp.file_id, &index, interner, diags);
        type_refs::resolve_file_type_refs(inp.file, &index, &imports, interner, diags, &prefix);
        let mut resolution = Resolution::new();
        body::resolve_file_bodies(
            inp.file,
            &index,
            &imports,
            interner,
            diags,
            &mut resolution,
            &prefix,
        );
        user_files.push(UserFile {
            file: inp.file,
            prefix,
            imports,
            resolution,
            trusted: inp.trusted,
        });
    }

    // ---- Phase 3：类型检查 ----
    // 先为所有文件构建 imports（ImportTable::collect 需要 &mut interner）。
    let mut file_state: Vec<(usize, String, imports::ImportTable)> = Vec::new();
    for (i, inp) in inputs.iter().enumerate() {
        let prefix = collect::package_prefix_of(inp.file, interner);
        let imports = imports::ImportTable::collect(inp.file, inp.file_id, &index, interner, diags);
        file_state.push((i, prefix, imports));
    }
    // 创建 TypeEnv（借用 interner 不可变）。
    let mut env = TypeEnv::new(&index, interner);
    for &(i, ref prefix, ref imports) in &file_state {
        let inp = &inputs[i];
        env::register_top_level_signatures(&mut env, inp.file, imports, prefix, diags);
        env::register_members(&mut env, inp.file, imports, prefix, diags);
        env::register_constructors(&mut env, inp.file, imports, prefix, diags);
        env::register_clayout_structs(&mut env, inp.file, prefix);
    }
    // 检查每个用户文件的函数体。
    for uf in &user_files {
        check_file_bodies(
            uf.file,
            &mut env,
            &uf.imports,
            &uf.resolution,
            diags,
            &uf.prefix,
            uf.trusted,
        );
    }
}

/// 检查一个文件的**顶层 + 成员**函数体 + 声明头语义检查。
fn check_file_bodies(
    file: &crate::syntax::ast::File,
    env: &mut TypeEnv,
    imports: &crate::resolve::imports::ImportTable,
    resolution: &crate::resolve::Resolution,
    diags: &mut DiagnosticSink,
    package_prefix: &str,
    trusted: bool,
) {
    use crate::syntax::ast::{ItemKind, ModifierKind};
    use std::collections::{HashMap, HashSet};
    let empty_tp: HashMap<scoop2_base::Symbol, crate::ty::TypeParamType> = HashMap::new();
    for item in &file.items {
        // @Experimental 注解校验（item 级目标是合法的）。
        check_experimental_annotations(item_annotations(item), false, env.interner, diags);
        match &item.kind {
            ItemKind::Fun(d) => {
                // @Intrinsic 只能在受信任 syslib cone 中声明。
                if !trusted && has_annotation(&d.annotations, "Intrinsic", env.interner) {
                    let name_text = env.interner.resolve(d.name.symbol).to_string();
                    diags.push(diagnostics::intrinsic_decl_requires_trusted_syslib(
                        "函数",
                        &name_text,
                        d.name.span,
                    ));
                }
                // @Intrinsic 函数不能有 body。
                if has_annotation(&d.annotations, "Intrinsic", env.interner) && d.body.is_some() {
                    diags.push(diagnostics::intrinsic_fun_must_have_no_body(d.name.span));
                }
                // 普通函数必须提供函数体（@Intrinsic / @Extern 允许省略；abstract/interface 成员由声明上下文判定）。
                let is_extern = has_annotation(&d.annotations, "Extern", env.interner);
                let is_intrinsic = has_annotation(&d.annotations, "Intrinsic", env.interner);
                if d.body.is_none() && !is_extern && !is_intrinsic {
                    let what = if d.receiver.is_some() {
                        "普通扩展函数必须提供函数体"
                    } else {
                        "普通顶层函数必须提供函数体"
                    };
                    diags.push(diagnostics::fun_must_have_body_detail(what, d.name.span));
                }
                // @Extern 函数 ABI 契约校验（顶层）。
                if is_extern {
                    extern_fn::check_extern_fun(
                        env,
                        imports,
                        diags,
                        package_prefix,
                        d,
                        extern_fn::ExternSite::TopLevel,
                    );
                }
                // 独立 `@CallingConvention`（未叠加 `@Extern`）的 native ABI 校验。
                if !is_extern && has_annotation(&d.annotations, "CallingConvention", env.interner) {
                    extern_fn::check_calling_convention(env, imports, diags, package_prefix, d);
                }
                // entry-point `main` 签名校验（spec P4 §13）。
                let name_text = env.interner.resolve(d.name.symbol);
                if name_text == "main" && d.receiver.is_none() {
                    check_main_signature(d, env, imports, package_prefix, diags);
                }
                // 扩展函数：this = 接收者类型（lowered）。
                let ext_this_ty = d.receiver.as_ref().map(|recv| {
                    let mut lower = crate::typecheck::lower::TypeLowering::new(
                        env,
                        imports,
                        empty_tp.clone(),
                        package_prefix.to_string(),
                        diags,
                    );
                    lower.lower(recv)
                });
                check_one_fun(
                    d,
                    env,
                    imports,
                    resolution,
                    diags,
                    package_prefix,
                    &empty_tp,
                    ext_this_ty,
                );
            }
            ItemKind::Type(d) => {
                // annotation class 限制（spec P5 §9）。
                let is_annotation = d
                    .modifiers
                    .iter()
                    .any(|m| m.kind == ModifierKind::Annotation);
                if is_annotation && let Some(err) = annotation_class_error(d, env.interner) {
                    // 与 legacy 一致：按固定顺序短路，仅报首个 annotation-class 错误。
                    diags.push(err);
                }
                // where 子句校验（目标在当前声明 / 无重复）。
                check_where_clause(
                    d.where_clause.as_ref(),
                    d.type_params.as_ref(),
                    env.interner,
                    diags,
                );
                // `@Target` / `@Retention` 只能用于 annotation class 声明。
                if !is_annotation {
                    for ann in &d.annotations {
                        let Some(name) = ann
                            .path
                            .segments
                            .last()
                            .map(|s| env.interner.resolve(s.symbol))
                        else {
                            continue;
                        };
                        if matches!(name, "Target" | "Retention") {
                            diags.push(diagnostics::meta_annotation_invalid_target(
                                &format!("@{name}"),
                                ann.path
                                    .segments
                                    .last()
                                    .map(|s| s.span)
                                    .unwrap_or(ann.path.span),
                            ));
                        }
                    }
                }
                // @ReleaseHook 宿主 / 释放函数 / 实参契约（短路）。
                if has_annotation(&d.annotations, "ReleaseHook", env.interner) {
                    release_hook::check_release_hook(env, diags, d, package_prefix);
                }
                let is_intrinsic_type = has_annotation(&d.annotations, "Intrinsic", env.interner);
                // @Intrinsic 只能在受信任 syslib cone 中声明。
                if !trusted && is_intrinsic_type {
                    let fqn_text = {
                        let name_text = env.interner.resolve(d.name.symbol);
                        if package_prefix.is_empty() {
                            name_text.to_string()
                        } else {
                            format!("{package_prefix}.{name_text}")
                        }
                    };
                    diags.push(diagnostics::intrinsic_decl_requires_trusted_syslib(
                        "类型",
                        &fqn_text,
                        d.name.span,
                    ));
                }
                // @Intrinsic 类型不能声明字段（ctor param-property + body property + body var field）。
                if is_intrinsic_type {
                    let owner_fqn_text = {
                        let name_text = env.interner.resolve(d.name.symbol);
                        if package_prefix.is_empty() {
                            name_text.to_string()
                        } else {
                            format!("{package_prefix}.{name_text}")
                        }
                    };
                    // 主构造 param-property。
                    if let Some(ctor) = &d.primary_ctor {
                        for cp in &ctor.params {
                            if cp.property.is_some() {
                                let fname = env.interner.resolve(cp.name.symbol);
                                diags.push(diagnostics::intrinsic_type_field_not_supported(
                                    &format!("{owner_fqn_text}.{fname}"),
                                    cp.name.span,
                                ));
                            }
                        }
                    }
                    // 类型体字段。
                    if let Some(body) = &d.body {
                        for m in &body.members {
                            if let crate::syntax::ast::TypeMemberKind::Property(pd) = &m.kind {
                                let fname = env.interner.resolve(pd.name.symbol);
                                diags.push(diagnostics::intrinsic_type_field_not_supported(
                                    &format!("{owner_fqn_text}.{fname}"),
                                    pd.name.span,
                                ));
                            }
                        }
                        // interface override 必须是带 body 的普通 method：
                        // `@Extern`/`@Intrinsic` override → 专用码；普通 override 缺 body → fun_must_have_body。
                        for m in &body.members {
                            if let crate::syntax::ast::TypeMemberKind::Fun(fd) = &m.kind
                                && fd
                                    .modifiers
                                    .iter()
                                    .any(|x| x.kind == ModifierKind::Override)
                            {
                                let is_native =
                                    has_annotation(&fd.annotations, "Extern", env.interner)
                                        || has_annotation(
                                            &fd.annotations,
                                            "Intrinsic",
                                            env.interner,
                                        );
                                if is_native {
                                    diags.push(
                                        diagnostics::intrinsic_type_interface_override_must_be_bodied_regular_method(
                                            fd.name.span,
                                        ),
                                    );
                                } else if fd.body.is_none() {
                                    diags.push(diagnostics::fun_must_have_body_detail(
                                        "普通成员函数必须提供函数体",
                                        fd.name.span,
                                    ));
                                }
                            }
                        }
                    }
                }
                let this_ty = make_nominal(env, package_prefix, d.name.symbol);
                // 虚方法（open/abstract/override/interface 方法）不能引入方法级类型参数（spec P3 §4.5）。
                if let Some(body) = &d.body {
                    for m in &body.members {
                        if let crate::syntax::ast::TypeMemberKind::Fun(fd) = &m.kind
                            && fd.type_params.is_some()
                        {
                            let is_virtual = d.kind == crate::syntax::ast::TypeKind::Interface
                                || fd.modifiers.iter().any(|m| {
                                    matches!(
                                        m.kind,
                                        ModifierKind::Open
                                            | ModifierKind::Abstract
                                            | ModifierKind::Override
                                    )
                                });
                            if is_virtual {
                                diags.push(diagnostics::virtual_method_cannot_be_generic(
                                    fd.name.span,
                                ));
                            }
                        }
                    }
                }
                if let Some(body) = &d.body {
                    // enum variant 字段重名检查。
                    if d.kind == crate::syntax::ast::TypeKind::Enum {
                        for m in &body.members {
                            if let crate::syntax::ast::TypeMemberKind::EnumVariant(ev) = &m.kind {
                                let mut seen: HashSet<scoop2_base::Symbol> = HashSet::new();
                                for fld in &ev.fields {
                                    if !seen.insert(fld.name.symbol) {
                                        diags.push(diagnostics::duplicate_enum_variant_field(
                                            fld.name.span,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    let mut tp_map = std::collections::HashMap::new();
                    merge_type_params(&mut tp_map, d.type_params.as_ref());
                    check_member_funs(
                        &body.members,
                        this_ty,
                        env,
                        imports,
                        resolution,
                        diags,
                        package_prefix,
                        &tp_map,
                    );
                }
            }
            ItemKind::Object(d) => {
                if let Some(name) = &d.name {
                    let this_ty = make_nominal(env, package_prefix, name.symbol);
                    if let Some(body) = &d.body {
                        check_member_funs(
                            &body.members,
                            this_ty,
                            env,
                            imports,
                            resolution,
                            diags,
                            package_prefix,
                            &empty_tp,
                        );
                    }
                }
            }
            ItemKind::Val(d) => {
                let is_extern_var = has_annotation(&d.annotations, "Extern", env.interner);
                // @Extern 顶层变量声明必须省略 initializer（外部符号由链接提供）。
                if is_extern_var && let Some(init) = &d.init {
                    extern_fn::check_extern_var(diags, init.span);
                }
                // 顶层 `var` 存储策略校验（@Extern var 豁免——外部符号）。
                if d.kind == crate::syntax::ast::ValKind::Var && !is_extern_var {
                    let name_span = match &d.binding {
                        crate::syntax::ast::ValBinding::Name(id) => id.span,
                        // invariant: 顶层 `var` 解构是 parse error，binding 必为 Name。
                        _ => {
                            d.ty.as_ref()
                                .map_or(scoop2_base::Span::default(), |t| t.span)
                        }
                    };
                    let has_tl = has_annotation(&d.annotations, "ThreadLocal", env.interner);
                    let has_global = has_annotation(&d.annotations, "Global", env.interner);
                    if has_tl && has_global {
                        diags.push(diagnostics::top_level_var_storage_policy_conflict(
                            name_span,
                        ));
                    } else if !has_tl && !has_global {
                        diags.push(diagnostics::top_level_var_requires_threadlocal_or_global(
                            name_span,
                        ));
                    } else if has_global && let Some(ty_ref) = &d.ty {
                        let ty = {
                            let mut lower = crate::typecheck::lower::TypeLowering::new(
                                env,
                                imports,
                                empty_tp.clone(),
                                package_prefix.to_string(),
                                diags,
                            );
                            lower.lower(ty_ref)
                        };
                        if !release_hook::is_gc_free_value_type(env, ty) {
                            diags
                                .push(diagnostics::top_level_var_type_must_be_gc_free(ty_ref.span));
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// 检查 entry-point `main` 的签名（spec P4 §13）。
/// 合法形式：`fun main()` 或 `fun main(args: Array<String>)`；effect row 必须是闭合 Pure。
fn check_main_signature(
    d: &crate::syntax::ast::FunDecl,
    env: &mut TypeEnv,
    imports: &crate::resolve::imports::ImportTable,
    package_prefix: &str,
    diags: &mut DiagnosticSink,
) {
    // effect row：必须是 Pure；若显式给出则必须是闭合的（`Pure!`）。
    if let Some(eff) = &d.effect {
        let is_pure = eff.terms.iter().all(|t| {
            t.path
                .segments
                .last()
                .is_some_and(|s| env.interner.resolve(s.symbol) == "Pure")
        });
        if !is_pure {
            diags.push(diagnostics::entry_point_must_be_pure(eff.span));
            return;
        }
        if eff.closed.is_none() {
            diags.push(diagnostics::entry_point_must_be_closed_pure(eff.span));
            return;
        }
    }
    // 参数数量：0 或 1。
    if d.params.len() > 1 {
        diags.push(diagnostics::entry_point_main_invalid_signature(
            &format!("不允许带 {} 个参数", d.params.len()),
            d.name.span,
        ));
        return;
    }
    // 单参数必须是 Array<String>。
    if d.params.len() == 1
        && let Some(param) = d.params.first()
    {
        let mut lower = crate::typecheck::lower::TypeLowering::new(
            env,
            imports,
            std::collections::HashMap::new(),
            package_prefix.to_string(),
            diags,
        );
        let param_ty = param
            .ty
            .as_ref()
            .map(|t| lower.lower(t))
            .unwrap_or_else(|| env.store.nothing());
        let string_fqn = env
            .interner
            .get("scoop.core.String")
            .or_else(|| env.interner.get("String"));
        let array_fqn = env
            .interner
            .get("scoop.core.Array")
            .or_else(|| env.interner.get("Array"));
        if let (Some(sfqn), Some(afqn)) = (string_fqn, array_fqn) {
            let string_nominal = crate::ty::NominalType {
                fqn: sfqn,
                args: vec![],
                eff: None,
            };
            let string_ty = env.store.ref_nominal(string_nominal);
            let array_nominal = crate::ty::NominalType {
                fqn: afqn,
                args: vec![string_ty],
                eff: None,
            };
            let expected = env.store.ref_nominal(array_nominal);
            if param_ty != expected {
                let ty_desc = match env.store.kind(param_ty) {
                    crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n)) => {
                        let fqn_text = env.interner.resolve(n.fqn);
                        let inner: Vec<String> = n
                            .args
                            .iter()
                            .map(|a| match env.store.kind(*a) {
                                crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Nominal(an)) => {
                                    env.interner.resolve(an.fqn).to_string()
                                }
                                crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::String) => {
                                    "String".to_string()
                                }
                                crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Any) => {
                                    "Any".to_string()
                                }
                                crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::Int) => {
                                    "Int".to_string()
                                }
                                crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::UInt) => {
                                    "UInt".to_string()
                                }
                                crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::Bool) => {
                                    "Bool".to_string()
                                }
                                crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::Char) => {
                                    "Char".to_string()
                                }
                                crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::Nominal(
                                    an,
                                )) => env.interner.resolve(an.fqn).to_string(),
                                other => format!("{other:?}"),
                            })
                            .collect();
                        let args_text = if inner.is_empty() {
                            String::new()
                        } else {
                            format!("<{}>", inner.join(", "))
                        };
                        format!("`{fqn_text}{args_text}`")
                    }
                    _ => format!("{:?}", env.store.kind(param_ty)),
                };
                diags.push(diagnostics::entry_point_main_invalid_signature(
                    &format!("参数类型为 {ty_desc}，必须是 Array<String>"),
                    param.name.span,
                ));
            }
        }
    }
}

/// annotation class 声明头校验（spec §15.2）。与 legacy 一致：按固定顺序**短路**，
/// 只返回首个错误（`must_be_class` → 修饰符 → eff 参数 → where → 类型参数 → 超类型 →
/// 类型体 → 主构造参数必须为 `val`）。
fn annotation_class_error(
    d: &crate::syntax::ast::TypeDecl,
    interner: &scoop2_base::Interner,
) -> Option<scoop2_base::diag::Diagnostic> {
    use crate::syntax::ast::{ModifierKind, TypeKind, ValKind};
    // annotation 修饰符只能用于 class。
    if d.kind != TypeKind::Class {
        return Some(diagnostics::annotation_class_must_be_class(d.name.span));
    }
    // 仅允许 public/internal/private/annotation 修饰符。
    for m in &d.modifiers {
        if !matches!(
            m.kind,
            ModifierKind::Annotation
                | ModifierKind::Public
                | ModifierKind::Internal
                | ModifierKind::Private
        ) {
            let mod_name = match m.kind {
                ModifierKind::Open => "open",
                ModifierKind::Sealed => "sealed",
                ModifierKind::Abstract => "abstract",
                ModifierKind::Override => "override",
                ModifierKind::Operator => "operator",
                // invariant: 上面 matches 已排除其余种类。
                _ => "modifier",
            };
            return Some(diagnostics::annotation_class_modifier_not_supported_detail(
                mod_name,
                d.name.span,
            ));
        }
    }
    // compile-time marker 不引入 effect 参数。
    if let Some(tp) = &d.type_params
        && let Some(eff) = &tp.effect_row
    {
        return Some(diagnostics::annotation_class_eff_param_not_supported(
            eff.span,
        ));
    }
    // compile-time marker 不引入 where 约束。
    if let Some(wc) = &d.where_clause {
        return Some(diagnostics::annotation_class_where_clause_not_supported(
            wc.span,
        ));
    }
    // compile-time marker 不引入泛型实例化面。
    if let Some(tp) = &d.type_params
        && let Some(first) = tp.params.first()
    {
        return Some(diagnostics::annotation_class_type_param_not_supported(
            first.span,
        ));
    }
    // 不支持超类型。
    if let Some(st) = d.supertypes.first() {
        return Some(diagnostics::annotation_class_supertypes_not_supported(
            st.span,
        ));
    }
    // 不支持类型体成员。
    if let Some(body) = &d.body {
        return Some(diagnostics::annotation_class_body_not_supported(body.span));
    }
    // 所有主构造参数必须是 `val`。
    if let Some(ctor) = &d.primary_ctor {
        for cp in &ctor.params {
            if cp.property != Some(ValKind::Val) {
                let name_text = interner.resolve(cp.name.symbol).to_string();
                return Some(diagnostics::annotation_class_param_must_be_val(
                    &name_text,
                    cp.name.span,
                ));
            }
        }
    }
    None
}

/// 校验 where 子句：约束目标必须在当前声明的类型参数中；同一 (目标, 约束) 不得重复。
fn check_where_clause(
    where_clause: Option<&crate::syntax::ast::WhereClause>,
    type_params: Option<&crate::syntax::ast::TypeParamList>,
    interner: &scoop2_base::Interner,
    diags: &mut DiagnosticSink,
) {
    let Some(wc) = where_clause else {
        return;
    };
    let param_names: std::collections::HashSet<scoop2_base::Symbol> = type_params
        .map(|tp| tp.params.iter().map(|p| p.name.symbol).collect())
        .unwrap_or_default();
    let mut seen: std::collections::HashMap<(scoop2_base::Symbol, String), scoop2_base::Span> =
        std::collections::HashMap::new();
    for c in &wc.constraints {
        // 目标必须在当前声明的类型参数中。
        if !param_names.contains(&c.name.symbol) {
            diags.push(diagnostics::where_target_not_in_current_decl(c.name.span));
            return;
        }
        let key = (c.name.symbol, bound_key(&c.bound, interner));
        if let Some(first_span) = seen.get(&key) {
            // 指向首次声明（与 legacy 一致）。
            diags.push(diagnostics::duplicate_where_constraint(*first_span));
            return;
        }
        seen.insert(key, c.span);
    }
}

/// where 约束的判重键（类型约束用 path 文本；ref/value 用固定标记）。
fn bound_key(bound: &crate::syntax::ast::GenericBound, interner: &scoop2_base::Interner) -> String {
    use crate::syntax::ast::GenericBound;
    match bound {
        GenericBound::Ref(_) => "ref".to_string(),
        GenericBound::Value(_) => "value".to_string(),
        GenericBound::Type(t) => type_ref_text(t, interner),
    }
}

/// TypeRef 的路径文本（用于 where 约束判重）。
fn type_ref_text(t: &crate::syntax::ast::TypeRef, interner: &scoop2_base::Interner) -> String {
    use crate::syntax::ast::TypeRefKind;
    match &t.kind {
        TypeRefKind::Path { path, .. } => path
            .segments
            .iter()
            .map(|s| interner.resolve(s.symbol))
            .collect::<Vec<_>>()
            .join("."),
        _ => format!("{:?}", t.kind),
    }
}

/// 提取 item 的注解（各 decl 都把注解放在顶层字段）。
fn item_annotations(item: &crate::syntax::ast::Item) -> &[crate::syntax::ast::AnnotationUse] {
    use crate::syntax::ast::ItemKind;
    match &item.kind {
        ItemKind::Fun(d) => &d.annotations,
        ItemKind::Type(d) => &d.annotations,
        ItemKind::Val(d) => &d.annotations,
        ItemKind::Object(d) => &d.annotations,
        ItemKind::TypeAlias(d) => &d.annotations,
        ItemKind::ExtensionProperty(d) => &d.annotations,
    }
}

/// 检查注解使用路径末段文本是否匹配给定名称。
fn has_annotation(
    anns: &[crate::syntax::ast::AnnotationUse],
    name: &str,
    interner: &scoop2_base::Interner,
) -> bool {
    anns.iter().any(|a| {
        a.path
            .segments
            .last()
            .is_some_and(|s| interner.resolve(s.symbol) == name)
    })
}

/// 校验 `@Experimental(feature = "x")` 注解（spec §15.x）。`is_expr_target` 表示用于
/// 表达式前缀（非法目标）。
pub(crate) fn check_experimental_annotation(
    ann: &crate::syntax::ast::AnnotationUse,
    is_expr_target: bool,
    interner: &scoop2_base::Interner,
    diags: &mut DiagnosticSink,
) {
    let name_span = ann
        .path
        .segments
        .last()
        .map(|s| s.span)
        .unwrap_or(ann.path.span);
    if is_expr_target {
        diags.push(diagnostics::builtin_annotation_invalid_target(
            "@Experimental",
            "函数 / 类型 / 属性 / 文件",
            name_span,
        ));
        return;
    }
    use crate::syntax::ast::ExprKind;
    if ann.args.is_empty() {
        diags.push(diagnostics::annotation_arg_missing_required(
            "@Experimental",
            "feature",
            name_span,
        ));
        return;
    }
    let mut feature_count = 0u32;
    let mut feature_value: Option<&ExprKind> = None;
    let mut has_positional = false;
    let mut has_other_named = false;
    for arg in &ann.args {
        match &arg.name {
            Some(n) if interner.resolve(n.symbol) == "feature" => {
                feature_count += 1;
                feature_value = Some(&arg.value.kind);
            }
            Some(_) => has_other_named = true,
            None => has_positional = true,
        }
    }
    if has_positional || feature_count != 1 || has_other_named {
        diags.push(diagnostics::experimental_annotation_invalid_arg_shape(
            name_span,
        ));
        return;
    }
    if !matches!(feature_value, Some(ExprKind::StringLit(_))) {
        diags.push(diagnostics::experimental_annotation_arg_must_be_string(
            name_span,
        ));
    }
}

/// 扫描一组注解中的 `@Experimental` / `@Suppress`，逐个校验。
pub(crate) fn check_experimental_annotations(
    anns: &[crate::syntax::ast::AnnotationUse],
    is_expr_target: bool,
    interner: &scoop2_base::Interner,
    diags: &mut DiagnosticSink,
) {
    for ann in anns {
        let Some(last) = ann.path.segments.last() else {
            continue;
        };
        match interner.resolve(last.symbol) {
            "Experimental" => {
                check_experimental_annotation(ann, is_expr_target, interner, diags);
            }
            "Suppress" => {
                check_suppress_annotation(ann, interner, diags);
            }
            _ => {}
        }
    }
}

/// 校验 `@Suppress("code", ...)` 注解（spec §15.x）。
fn check_suppress_annotation(
    ann: &crate::syntax::ast::AnnotationUse,
    interner: &scoop2_base::Interner,
    diags: &mut DiagnosticSink,
) {
    use crate::syntax::ast::ExprKind;
    let name_span = ann
        .path
        .segments
        .last()
        .map(|s| s.span)
        .unwrap_or(ann.path.span);
    if ann.args.is_empty() {
        diags.push(diagnostics::suppress_annotation_requires_warning_codes(
            name_span,
        ));
        return;
    }
    for arg in &ann.args {
        let span = arg.value.span;
        if arg.name.is_some() {
            diags.push(diagnostics::suppress_annotation_named_args_not_supported(
                span,
            ));
            return;
        }
        let ExprKind::StringLit(s) = &arg.value.kind else {
            diags.push(diagnostics::suppress_annotation_arg_must_be_string(span));
            return;
        };
        if !is_known_warning_code(&s.value) {
            diags.push(diagnostics::unknown_suppress_warning_code(&s.value, span));
            return;
        }
    }
    let _ = interner;
}

/// 已知 warning code（复刻 legacy `is_known_warning_code`）。
fn is_known_warning_code(code: &str) -> bool {
    matches!(
        code,
        "deprecated" | "enum-size-disparity" | "redundant-when-else"
    )
}

#[allow(clippy::too_many_arguments)]
fn check_one_fun(
    d: &crate::syntax::ast::FunDecl,
    env: &mut TypeEnv,
    imports: &crate::resolve::imports::ImportTable,
    resolution: &crate::resolve::Resolution,
    diags: &mut DiagnosticSink,
    package_prefix: &str,
    enclosing_type_params: &std::collections::HashMap<
        scoop2_base::Symbol,
        crate::ty::TypeParamType,
    >,
    this_ty: Option<crate::ty::TypeId>,
) {
    use scoop2_base::FileId;
    let Some(body) = &d.body else {
        // 即便无 body，where 子句仍需校验（header 检查）。
        check_where_clause(
            d.where_clause.as_ref(),
            d.type_params.as_ref(),
            env.interner,
            diags,
        );
        return;
    };
    // where 子句校验（目标在当前声明 / 无重复）。
    check_where_clause(
        d.where_clause.as_ref(),
        d.type_params.as_ref(),
        env.interner,
        diags,
    );
    // @Intrinsic 成员函数不能有 body。
    if has_annotation(&d.annotations, "Intrinsic", env.interner) {
        diags.push(diagnostics::intrinsic_fun_must_have_no_body(d.name.span));
    }
    // 构建类型参数作用域：外层类型参数 + 本函数自身的类型参数。
    let mut tp = enclosing_type_params.clone();
    if let Some(type_params) = &d.type_params {
        for p in &type_params.params {
            tp.insert(
                p.name.symbol,
                crate::ty::TypeParamType {
                    name: p.name.symbol,
                    file: FileId(0),
                    span: p.name.span,
                },
            );
        }
    }
    expr::check_function(
        &d.params,
        d.return_ty.as_ref(),
        body,
        env,
        imports,
        resolution,
        diags,
        package_prefix,
        tp,
        this_ty,
    );
}

#[allow(clippy::too_many_arguments)]
fn check_member_funs(
    members: &[crate::syntax::ast::TypeMember],
    this_ty: Option<crate::ty::TypeId>,
    env: &mut TypeEnv,
    imports: &crate::resolve::imports::ImportTable,
    resolution: &crate::resolve::Resolution,
    diags: &mut DiagnosticSink,
    package_prefix: &str,
    enclosing_type_params: &std::collections::HashMap<
        scoop2_base::Symbol,
        crate::ty::TypeParamType,
    >,
) {
    use crate::syntax::ast::TypeMemberKind;
    for m in members {
        match &m.kind {
            TypeMemberKind::Fun(d) => {
                check_one_fun(
                    d,
                    env,
                    imports,
                    resolution,
                    diags,
                    package_prefix,
                    enclosing_type_params,
                    this_ty,
                );
            }
            TypeMemberKind::Object(d) => {
                if let Some(name) = &d.name
                    && let Some(b) = &d.body
                {
                    let nested = make_nominal_under(env, this_ty, name.symbol);
                    check_member_funs(
                        &b.members,
                        nested,
                        env,
                        imports,
                        resolution,
                        diags,
                        package_prefix,
                        enclosing_type_params,
                    );
                }
            }
            TypeMemberKind::Type(d) => {
                if let Some(b) = &d.body {
                    let nested = make_nominal_under(env, this_ty, d.name.symbol);
                    // 嵌套类型：合并外层 + 自身类型参数。
                    let mut merged = enclosing_type_params.clone();
                    merge_type_params(&mut merged, d.type_params.as_ref());
                    check_member_funs(
                        &b.members,
                        nested,
                        env,
                        imports,
                        resolution,
                        diags,
                        package_prefix,
                        &merged,
                    );
                }
            }
            _ => {}
        }
    }
}

/// 把类型参数列表合并进已有 map（用于嵌套类型累积外层 + 自身类型参数）。
fn merge_type_params(
    map: &mut std::collections::HashMap<scoop2_base::Symbol, crate::ty::TypeParamType>,
    tp: Option<&crate::syntax::ast::TypeParamList>,
) {
    use scoop2_base::FileId;
    if let Some(tp) = tp {
        for p in &tp.params {
            map.insert(
                p.name.symbol,
                crate::ty::TypeParamType {
                    name: p.name.symbol,
                    file: FileId(0),
                    span: p.name.span,
                },
            );
        }
    }
}

fn make_nominal(
    env: &mut TypeEnv,
    prefix: &str,
    name: scoop2_base::Symbol,
) -> Option<crate::ty::TypeId> {
    let name_text = env.interner.resolve(name);
    let fqn_text = if prefix.is_empty() {
        name_text.to_string()
    } else {
        format!("{prefix}.{name_text}")
    };
    let fqn = env.interner.get(&fqn_text)?;
    let nominal = crate::ty::NominalType {
        fqn,
        args: vec![],
        eff: None,
    };
    Some(if env.is_reference_nominal(fqn) {
        env.store.ref_nominal(nominal)
    } else {
        env.store.value_nominal(nominal)
    })
}

fn make_nominal_under(
    env: &mut TypeEnv,
    parent: Option<crate::ty::TypeId>,
    name: scoop2_base::Symbol,
) -> Option<crate::ty::TypeId> {
    let pfqn = parent.and_then(|tid| match env.store.kind(tid) {
        crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n))
        | crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n)) => Some(n.fqn),
        _ => None,
    })?;
    let pt = env.interner.resolve(pfqn);
    let nt = env.interner.resolve(name);
    let fqn = env.interner.get(&format!("{pt}.{nt}"))?;
    let nominal = crate::ty::NominalType {
        fqn,
        args: vec![],
        eff: None,
    };
    Some(if env.is_reference_nominal(fqn) {
        env.store.ref_nominal(nominal)
    } else {
        env.store.value_nominal(nominal)
    })
}
