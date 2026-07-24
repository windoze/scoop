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
pub mod overloads;
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
        env::register_type_aliases(&mut env, inp.file, prefix);
        env::register_type_constraints(&mut env, inp.file, prefix);
        env::register_top_level_vals(&mut env, inp.file, imports, prefix, diags);
        env::register_enum_variants(&mut env, inp.file, prefix);
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
    // 顶层函数重载冲突检测（pre-pass）。
    let top_funs: Vec<&crate::syntax::ast::FunDecl> = file
        .items
        .iter()
        .filter_map(|it| {
            if let ItemKind::Fun(d) = &it.kind {
                Some(d)
            } else {
                None
            }
        })
        .collect();
    overloads::check_top_level_overload_conflicts(env, imports, diags, package_prefix, &top_funs);
    // 文件级注解目标检查（`@file:...`）。
    check_file_annotation_targets(file, env.interner, diags);
    for item in &file.items {
        // @Experimental / @Suppress 注解校验（item 级目标是合法的）。
        check_experimental_annotations(item_annotations(item), false, env.interner, diags);
        // 未知注解类型检查。
        check_annotation_uses(item_annotations(item), env, package_prefix, diags);
        // `@Deprecated` 实参校验（位置/命名/类型）。
        check_deprecated_annotation_args(item_annotations(item), env.interner, diags);
        // 内建注解目标检查。
        check_builtin_annotation_targets(item, env.interner, diags);
        match &item.kind {
            ItemKind::Fun(d) => {
                // `annotation` 修饰符只能用于 annotation class，不能用于函数。
                if d.modifiers
                    .iter()
                    .any(|m| m.kind == ModifierKind::Annotation)
                {
                    diags.push(diagnostics::annotation_modifier_invalid_target(d.name.span));
                }
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
                // `@NoGC` 函数必须声明 Pure effect row（不得声明非 Pure effect）。
                if has_annotation(&d.annotations, "NoGC", env.interner)
                    && !expr::effect_row_expr_is_pure(d.effect.as_ref(), env.interner)
                    && let Some(eff) = &d.effect
                {
                    diags.push(diagnostics::nogc_fun_effects_not_allowed(eff.span));
                }
                // `@NoGC` 函数不允许声明 effect row 参数（`<eff E>`）。
                if has_annotation(&d.annotations, "NoGC", env.interner)
                    && let Some(tp) = &d.type_params
                    && let Some(er) = &tp.effect_row
                {
                    diags.push(diagnostics::nogc_fun_eff_param_not_allowed(er.span));
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
                // annotation class 上的 `@Target(...)` 实参校验。
                if is_annotation {
                    check_target_annotation_args(&d.annotations, env.interner, diags);
                }
                // compiler-owned interface 限制：`Continuation` 不能被用户实现/继承。
                for st in &d.supertypes {
                    if let crate::syntax::ast::TypeRefKind::Path { path, .. } = &st.ty.kind
                        && let Some(seg) = path.segments.last()
                    {
                        let name = env.interner.resolve(seg.symbol);
                        let stripped = name.strip_prefix("scoop.core.").unwrap_or(name);
                        if stripped == "Continuation" {
                            diags.push(diagnostics::continuation_impl_not_allowed(st.span));
                        }
                    }
                }
                // 只能继承 `open`/`abstract` 类（class 超类必须 open）。
                if d.kind == crate::syntax::ast::TypeKind::Class {
                    check_superclass_open(d, d.name.symbol, env, package_prefix, diags);
                    check_overrides(d, env, imports, diags, package_prefix);
                    check_interface_impl_complete(d, env, diags, package_prefix);
                    if let Some(body) = &d.body {
                        overloads::check_ctor_overload_conflicts(
                            env,
                            imports,
                            diags,
                            package_prefix,
                            d.name.symbol,
                            &body.members,
                        );
                        // 有主构造器时，次构造器必须 `: this(...)` 委托（不能省略 / 不能 super）。
                        if d.primary_ctor.is_some() {
                            for m in &body.members {
                                if let crate::syntax::ast::TypeMemberKind::SecondaryCtor(c) =
                                    &m.kind
                                {
                                    match &c.delegation {
                                        None => {
                                            diags.push(
                                                diagnostics::secondary_ctor_delegation_required(
                                                    c.span,
                                                ),
                                            );
                                        }
                                        Some(del)
                                            if matches!(
                                                del.kind,
                                                crate::syntax::ast::CtorDelegationKind::Super
                                            ) =>
                                        {
                                            diags.push(
                                                diagnostics::secondary_ctor_delegation_must_be_this(
                                                    del.span,
                                                ),
                                            );
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
                // where 子句校验（目标在当前声明 / 无重复）。
                check_where_clause(
                    d.where_clause.as_ref(),
                    d.type_params.as_ref(),
                    env,
                    package_prefix,
                    diags,
                );
                // 具体类型的成员函数必须提供函数体（interface / effect / abstract / @Intrinsic / @Extern 除外）。
                check_member_funs_have_body(d, env.interner, diags);
                // 主构造 param-property 重名检查。
                let is_intrinsic_type_early =
                    has_annotation(&d.annotations, "Intrinsic", env.interner);
                if let Some(ctor) = &d.primary_ctor {
                    let mut seen: HashSet<scoop2_base::Symbol> = HashSet::new();
                    for cp in &ctor.params {
                        if cp.property.is_some() && !seen.insert(cp.name.symbol) {
                            diags.push(diagnostics::duplicate_struct_field(cp.name.span));
                        }
                    }
                    // struct 的主构造 param-property 也必须是 val（@Intrinsic 豁免）。
                    if d.kind == crate::syntax::ast::TypeKind::Struct && !is_intrinsic_type_early {
                        for cp in &ctor.params {
                            if cp.property == Some(crate::syntax::ast::ValKind::Var) {
                                diags.push(diagnostics::struct_field_must_be_val(cp.name.span));
                            }
                        }
                    }
                }
                // 值类型（struct/enum）属性校验（@Intrinsic 豁免）。
                if matches!(
                    d.kind,
                    crate::syntax::ast::TypeKind::Struct | crate::syntax::ast::TypeKind::Enum
                ) && !is_intrinsic_type_early
                    && let Some(body) = &d.body
                {
                    use crate::syntax::ast::{AccessorKind, TypeMemberKind};
                    for m in &body.members {
                        let TypeMemberKind::Property(pd) = &m.kind else {
                            continue;
                        };
                        let getter_span = pd
                            .accessors
                            .iter()
                            .find(|a| a.kind == AccessorKind::Get)
                            .map(|a| a.span);
                        let setter_span = pd
                            .accessors
                            .iter()
                            .find(|a| a.kind == AccessorKind::Set)
                            .map(|a| a.span);
                        let has_getter = getter_span.is_some();
                        // struct `var` 属性用专用码；enum `var` 属性用 value_type 码。
                        if pd.kind == crate::syntax::ast::ValKind::Var {
                            if d.kind == crate::syntax::ast::TypeKind::Struct {
                                diags.push(diagnostics::struct_field_must_be_val(pd.name.span));
                            } else {
                                diags.push(diagnostics::value_type_property_must_be_val(
                                    pd.name.span,
                                ));
                            }
                        }
                        // `val` 属性不允许 setter（指向 setter）。
                        if pd.kind == crate::syntax::ast::ValKind::Val
                            && let Some(sspan) = setter_span
                        {
                            diags.push(diagnostics::val_property_setter_not_allowed(sspan));
                        }
                        // computed 属性（带 getter）不允许 initializer（指向 init）。
                        if has_getter && let Some(init) = &pd.init {
                            diags.push(diagnostics::value_type_property_initializer_not_allowed(
                                init.span,
                            ));
                        }
                    }
                }
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
                // 委托属性（`by`）：值类型（struct/enum）不允许。
                if matches!(
                    d.kind,
                    crate::syntax::ast::TypeKind::Struct | crate::syntax::ast::TypeKind::Enum
                ) && let Some(body) = &d.body
                {
                    for m in &body.members {
                        if let crate::syntax::ast::TypeMemberKind::Property(pd) = &m.kind
                            && pd.delegate.is_some()
                        {
                            diags.push(diagnostics::delegated_property_not_allowed_in_value_type(
                                pd.name.span,
                            ));
                        }
                    }
                }
                // 委托属性（class owner）：delegate 的 getValue/setValue 校验。
                if d.kind == crate::syntax::ast::TypeKind::Class
                    && let Some(body) = &d.body
                {
                    for m in &body.members {
                        if let crate::syntax::ast::TypeMemberKind::Property(pd) = &m.kind
                            && pd.delegate.is_some()
                        {
                            check_delegated_property(env, imports, diags, package_prefix, pd);
                        }
                    }
                }
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
                    // enum variant 字段重名检查 + variant 重名检查。
                    if d.kind == crate::syntax::ast::TypeKind::Enum {
                        let mut seen_variants: HashSet<scoop2_base::Symbol> = HashSet::new();
                        for m in &body.members {
                            if let crate::syntax::ast::TypeMemberKind::EnumVariant(ev) = &m.kind {
                                if !seen_variants.insert(ev.name.symbol) {
                                    diags.push(diagnostics::duplicate_enum_variant(ev.name.span));
                                }
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
                    // computed property 引用 `field` 检查。
                    check_computed_property_field_ref(&body.members, env.interner, diags);
                }
            }
            ItemKind::Object(d) => {
                if let Some(name) = &d.name {
                    let this_ty = make_nominal(env, package_prefix, name.symbol);
                    let name_text = env.interner.resolve(name.symbol);
                    let obj_fqn = if package_prefix.is_empty() {
                        name_text.to_string()
                    } else {
                        format!("{package_prefix}.{name_text}")
                    };
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
                        // object 的 init 块与属性初始化器必须 `Pure!`（静态初始化器）。
                        for m in &body.members {
                            use crate::syntax::ast::TypeMemberKind;
                            match &m.kind {
                                TypeMemberKind::InitBlock(ib) => {
                                    let what = format!("object `{obj_fqn}` init block");
                                    expr::check_pure_static_init(
                                        env,
                                        imports,
                                        resolution,
                                        diags,
                                        package_prefix,
                                        this_ty,
                                        what,
                                        expr::PureInitSite::Block(&ib.body),
                                    );
                                }
                                TypeMemberKind::Property(pd) if pd.init.is_some() => {
                                    let pname = env.interner.resolve(pd.name.symbol);
                                    let what = format!("object `{obj_fqn}` 属性 `{pname}`");
                                    if let Some(init) = &pd.init {
                                        expr::check_pure_static_init(
                                            env,
                                            imports,
                                            resolution,
                                            diags,
                                            package_prefix,
                                            this_ty,
                                            what,
                                            expr::PureInitSite::Expr(init),
                                        );
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            ItemKind::Val(d) => {
                let is_extern_var = has_annotation(&d.annotations, "Extern", env.interner);
                // 顶层 `val`/`var` 必须显式标注类型（顶层不做类型推断）。
                if d.ty.is_none() {
                    let name_span = match &d.binding {
                        crate::syntax::ast::ValBinding::Name(id) => id.span,
                        _ => scoop2_base::Span::default(),
                    };
                    diags.push(diagnostics::missing_type_annotation(name_span));
                }
                // 降级类型注解 + 检查 initializer。
                if d.init.is_some() {
                    expr::check_top_level_val(d, env, imports, resolution, diags, package_prefix);
                } else if let Some(ty_ref) = &d.ty {
                    let mut lower = crate::typecheck::lower::TypeLowering::new(
                        env,
                        imports,
                        empty_tp.clone(),
                        package_prefix.to_string(),
                        diags,
                    );
                    lower.lower(ty_ref);
                }
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

/// 校验委托属性（`by`）的 delegate：必须有 `getValue`（`var` 还需 `setValue`），且签名匹配。
fn check_delegated_property(
    env: &mut TypeEnv,
    imports: &crate::resolve::imports::ImportTable,
    diags: &mut DiagnosticSink,
    package_prefix: &str,
    pd: &crate::syntax::ast::PropertyDecl,
) {
    use crate::syntax::ast::{ExprKind, ValKind};
    // 仅处理 `Type()` 构造调用形式的 delegate。
    let delegate_expr = match &pd.delegate {
        Some(e) => e,
        None => return,
    };
    // 诊断指向 delegate 表达式（与 legacy 一致）。
    let delegate_span = delegate_expr.span;
    let callee_name = match &delegate_expr.kind {
        ExprKind::Call { callee, .. } => match &callee.kind {
            ExprKind::Ident(id) => Some(id.symbol),
            _ => None,
        },
        _ => None,
    };
    let Some(callee) = callee_name else {
        return;
    };
    let fqn_text = {
        let n = env.interner.resolve(callee);
        if package_prefix.is_empty() {
            n.to_string()
        } else {
            format!("{package_prefix}.{n}")
        }
    };
    let Some(delegate_fqn) = env.interner.get(&fqn_text) else {
        return;
    };
    let get_value = env.interner.get("getValue").unwrap_or(callee);
    let set_value = env.interner.get("setValue").unwrap_or(callee);

    // getValue 必须存在。
    let Some(get_sigs) = env.member_signatures(delegate_fqn, get_value) else {
        diags.push(diagnostics::delegated_property_missing_get_value(
            delegate_span,
        ));
        return;
    };
    let Some(get_sig) = get_sigs.first() else {
        diags.push(diagnostics::delegated_property_missing_get_value(
            delegate_span,
        ));
        return;
    };
    // getValue 的 `property` 参数（第 2 个）必须是 `PropertyMeta`。
    if get_sig.params.len() >= 2 {
        let prop_ty = get_sig.params[1];
        if !is_property_meta_type(env, prop_ty) {
            diags.push(
                diagnostics::delegated_property_get_value_signature_mismatch(
                    &fmt_type_short(env, prop_ty),
                    delegate_span,
                ),
            );
            return;
        }
    }
    // var：setValue 必须存在，且 value 参数类型匹配属性类型。
    if pd.kind == ValKind::Var {
        let Some(set_sigs) = env.member_signatures(delegate_fqn, set_value) else {
            diags.push(diagnostics::delegated_property_missing_set_value(
                delegate_span,
            ));
            return;
        };
        let Some(set_sig) = set_sigs.first() else {
            diags.push(diagnostics::delegated_property_missing_set_value(
                delegate_span,
            ));
            return;
        };
        if set_sig.params.len() >= 3
            && let Some(prop_ty_ref) = &pd.ty
        {
            let value_ty = set_sig.params[2];
            let prop_ty = {
                let mut lower = crate::typecheck::lower::TypeLowering::new(
                    env,
                    imports,
                    std::collections::HashMap::new(),
                    package_prefix.to_string(),
                    diags,
                );
                lower.lower(prop_ty_ref)
            };
            if value_ty != prop_ty {
                diags.push(
                    diagnostics::delegated_property_set_value_signature_mismatch(
                        &fmt_type_fqn(env, prop_ty),
                        delegate_span,
                    ),
                );
            }
        }
    }
}

/// 是否为 `PropertyMeta` 类型。
fn is_property_meta_type(env: &TypeEnv, id: crate::ty::TypeId) -> bool {
    let fqn = match env.store.kind(id) {
        crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n))
        | crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n)) => Some(n.fqn),
        _ => None,
    };
    fqn.map(|f| env.interner.resolve(f).ends_with(".PropertyMeta"))
        .unwrap_or(false)
}

/// 类型短名（诊断用）。
fn fmt_type_short(env: &TypeEnv, id: crate::ty::TypeId) -> String {
    match env.store.kind(id) {
        crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Any) => "Any".into(),
        crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::String) => "String".into(),
        crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n))
        | crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n)) => {
            env.interner.resolve(n.fqn).to_string()
        }
        other => format!("{other:?}"),
    }
}

/// 类型 FQN（诊断用；标量映射到 scoop.core 短名，nominal 用全限定）。
fn fmt_type_fqn(env: &TypeEnv, id: crate::ty::TypeId) -> String {
    match env.store.kind(id) {
        crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Any) => "scoop.core.Any".into(),
        crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::String) => "scoop.core.String".into(),
        crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n))
        | crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n)) => {
            env.interner.resolve(n.fqn).to_string()
        }
        crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::Int) => "scoop.core.Int".into(),
        crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::UInt) => "scoop.core.UInt".into(),
        crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::Bool) => "scoop.core.Bool".into(),
        crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::Unit) => "scoop.core.Unit".into(),
        other => format!("{other:?}"),
    }
}

/// 校验 `@Target(AnnotationTarget.X, ...)` 的实参：每个 X 必须是合法的 target variant。
fn check_target_annotation_args(
    anns: &[crate::syntax::ast::AnnotationUse],
    interner: &scoop2_base::Interner,
    diags: &mut DiagnosticSink,
) {
    use crate::syntax::ast::{ExprKind, MemberName};
    for ann in anns {
        if !ann
            .path
            .segments
            .last()
            .is_some_and(|s| interner.resolve(s.symbol) == "Target")
        {
            continue;
        }
        for arg in &ann.args {
            if let ExprKind::MemberAccess { member, .. } = &arg.value.kind
                && let MemberName::Named(name) = member
            {
                let variant = interner.resolve(name.symbol);
                if !is_valid_annotation_target(variant) {
                    diags.push(diagnostics::invalid_annotation_target_name(
                        variant, name.span,
                    ));
                    return;
                }
            }
        }
    }
}

/// 合法的 `AnnotationTarget` variant 名。
fn is_valid_annotation_target(name: &str) -> bool {
    matches!(
        name,
        "Function"
            | "Property"
            | "Field"
            | "Param"
            | "Type"
            | "Constructor"
            | "LocalVariable"
            | "Expression"
            | "Module"
            | "TypeParam"
            | "EnumVariant"
    )
}

/// 校验 where 子句：约束目标必须在当前声明的类型参数中；同一 (目标, 约束) 不得重复。
fn check_where_clause(
    where_clause: Option<&crate::syntax::ast::WhereClause>,
    type_params: Option<&crate::syntax::ast::TypeParamList>,
    env: &mut TypeEnv,
    package_prefix: &str,
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
    // 类型参数 → (has_ref_span, has_value_span)：检测 ref/value 互斥。
    let mut ref_bounds: std::collections::HashMap<scoop2_base::Symbol, scoop2_base::Span> =
        std::collections::HashMap::new();
    let mut value_bounds: std::collections::HashMap<scoop2_base::Symbol, scoop2_base::Span> =
        std::collections::HashMap::new();
    for c in &wc.constraints {
        // 目标必须在当前声明的类型参数中。
        if !param_names.contains(&c.name.symbol) {
            diags.push(diagnostics::where_target_not_in_current_decl(c.name.span));
            return;
        }
        // ref/value 互斥。
        match &c.bound {
            crate::syntax::ast::GenericBound::Ref(s) => {
                ref_bounds.insert(c.name.symbol, *s);
            }
            crate::syntax::ast::GenericBound::Value(s) => {
                value_bounds.insert(c.name.symbol, *s);
            }
            _ => {}
        }
        let key = (c.name.symbol, bound_key(&c.bound, env.interner));
        if let Some(first_span) = seen.get(&key) {
            // 指向首次声明（与 legacy 一致）。
            diags.push(diagnostics::duplicate_where_constraint(*first_span));
            return;
        }
        seen.insert(key, c.span);
    }
    // ref 与 value 互斥（任一类型参数同时带两者）。
    for (name, ref_span) in &ref_bounds {
        if let Some(value_span) = value_bounds.get(name) {
            // 指向较后者（第二个出现的约束）。
            let span = if ref_span.start > value_span.start {
                *ref_span
            } else {
                *value_span
            };
            diags.push(diagnostics::ref_value_bound_mutually_exclusive(span));
            return;
        }
    }
    // 冲突的 class bound：同一类型参数约束到两个以上 class。
    let mut class_bounds: std::collections::HashMap<scoop2_base::Symbol, Vec<scoop2_base::Span>> =
        std::collections::HashMap::new();
    for c in &wc.constraints {
        if let crate::syntax::ast::GenericBound::Type(t) = &c.bound
            && is_class_bound(t, env, package_prefix)
        {
            class_bounds.entry(c.name.symbol).or_default().push(c.span);
        }
    }
    for spans in class_bounds.values() {
        if spans.len() >= 2 {
            diags.push(diagnostics::conflicting_where_constraints(spans[0]));
            return;
        }
    }
}

/// Type bound 是否解析为 class（用于检测冲突的 class bound）。
fn is_class_bound(t: &crate::syntax::ast::TypeRef, env: &TypeEnv, package_prefix: &str) -> bool {
    use crate::syntax::ast::TypeRefKind;
    let path = match &t.kind {
        TypeRefKind::Path { path, .. } => path,
        _ => return false,
    };
    let fqn_text = if path.segments.len() == 1 {
        let n = env.interner.resolve(path.segments[0].symbol);
        if package_prefix.is_empty() {
            n.to_string()
        } else {
            format!("{package_prefix}.{n}")
        }
    } else {
        path.segments
            .iter()
            .map(|s| env.interner.resolve(s.symbol))
            .collect::<Vec<_>>()
            .join(".")
    };
    env.interner
        .get(&fqn_text)
        .and_then(|f| env.index.category(f))
        .is_some_and(|c| matches!(c, crate::resolve::symbol::NominalCategory::Class))
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

/// computed property 引用 `field` 检查：有自定义 getter 的属性不能引用 `field`。
fn check_computed_property_field_ref(
    members: &[crate::syntax::ast::TypeMember],
    interner: &scoop2_base::Interner,
    diags: &mut DiagnosticSink,
) {
    use crate::syntax::ast::{AccessorKind, TypeMemberKind};
    let field_sym = interner.get("field");
    let Some(field_sym) = field_sym else {
        return;
    };
    for m in members {
        let TypeMemberKind::Property(pd) = &m.kind else {
            continue;
        };
        for acc in &pd.accessors {
            if acc.kind != AccessorKind::Get {
                continue;
            }
            // 在 getter body 中查找 `field` 引用。
            let found = match &acc.body {
                crate::syntax::ast::AccessorBody::Block(b) => block_contains_ident(b, field_sym),
                crate::syntax::ast::AccessorBody::Expr(e) => expr_contains_ident(e, field_sym),
            };
            if found {
                // 找到 `field` 标识符的精确 span。
                let field_span = find_ident_span_in_accessor(&acc.body, field_sym);
                diags.push(diagnostics::field_used_without_backing_field(field_span));
            }
        }
    }
}

/// 块中是否包含对指定标识符的引用。
fn block_contains_ident(block: &crate::syntax::ast::Block, sym: scoop2_base::Symbol) -> bool {
    for stmt in &block.stmts {
        match &stmt.kind {
            crate::syntax::ast::StmtKind::Return { value } => {
                if let Some(v) = value
                    && expr_contains_ident(v, sym)
                {
                    return true;
                }
            }
            crate::syntax::ast::StmtKind::Expr(e) if expr_contains_ident(e, sym) => {
                return true;
            }
            _ => {}
        }
    }
    false
}

/// 表达式中是否包含对指定标识符的引用。
fn expr_contains_ident(e: &crate::syntax::ast::Expr, sym: scoop2_base::Symbol) -> bool {
    match &e.kind {
        crate::syntax::ast::ExprKind::Ident(id) => id.symbol == sym,
        crate::syntax::ast::ExprKind::Binary { lhs, rhs, .. } => {
            expr_contains_ident(lhs, sym) || expr_contains_ident(rhs, sym)
        }
        crate::syntax::ast::ExprKind::Unary { expr, .. } => expr_contains_ident(expr, sym),
        _ => false,
    }
}

/// 在访问器体中查找 `field` 标识符的 span。
fn find_ident_span_in_accessor(
    body: &crate::syntax::ast::AccessorBody,
    sym: scoop2_base::Symbol,
) -> scoop2_base::Span {
    match body {
        crate::syntax::ast::AccessorBody::Block(b) => {
            for stmt in &b.stmts {
                match &stmt.kind {
                    crate::syntax::ast::StmtKind::Return { value } => {
                        if let Some(v) = value
                            && let Some(s) = find_ident_span(v, sym)
                        {
                            return s;
                        }
                    }
                    crate::syntax::ast::StmtKind::Expr(e) => {
                        if let Some(s) = find_ident_span(e, sym) {
                            return s;
                        }
                    }
                    _ => {}
                }
            }
            scoop2_base::Span::default()
        }
        crate::syntax::ast::AccessorBody::Expr(e) => find_ident_span(e, sym).unwrap_or_default(),
    }
}

/// 递归查找标识符的 span。
fn find_ident_span(
    e: &crate::syntax::ast::Expr,
    sym: scoop2_base::Symbol,
) -> Option<scoop2_base::Span> {
    match &e.kind {
        crate::syntax::ast::ExprKind::Ident(id) if id.symbol == sym => Some(id.span),
        crate::syntax::ast::ExprKind::Binary { lhs, rhs, .. } => {
            find_ident_span(lhs, sym).or_else(|| find_ident_span(rhs, sym))
        }
        crate::syntax::ast::ExprKind::Unary { expr, .. } => find_ident_span(expr, sym),
        _ => None,
    }
}

/// `@Deprecated` 实参校验：
/// - 至多一个位置实参（第一个=`message`），其余必须命名；
/// - 命名实参 `message` 必须是字符串字面量。
fn check_deprecated_annotation_args(
    anns: &[crate::syntax::ast::AnnotationUse],
    interner: &scoop2_base::Interner,
    diags: &mut DiagnosticSink,
) {
    use crate::syntax::ast::ExprKind;
    for ann in anns {
        let Some(last) = ann.path.segments.last() else {
            continue;
        };
        if interner.resolve(last.symbol) != "Deprecated" {
            continue;
        }
        let mut positional = 0usize;
        for arg in &ann.args {
            match &arg.name {
                None => {
                    positional += 1;
                    if positional > 1 {
                        diags.push(
                            diagnostics::deprecated_annotation_only_first_arg_positional(arg.span),
                        );
                    }
                }
                Some(name) => {
                    let pname = interner.resolve(name.symbol);
                    if pname == "message" && !matches!(&arg.value.kind, ExprKind::StringLit(_)) {
                        let found = match &arg.value.kind {
                            ExprKind::IntLit(_) => "Int",
                            ExprKind::FloatLit(_) => "Float",
                            _ => "非字符串",
                        };
                        diags.push(diagnostics::annotation_arg_type_mismatch(
                            "message", "String", found, arg.span,
                        ));
                    }
                }
            }
        }
    }
}

/// 文件级注解目标检查（`@file:...`）：内建注解 `@Deprecated` 只能用于 函数/类型/属性，不能用于文件。
fn check_file_annotation_targets(
    file: &crate::syntax::ast::File,
    interner: &scoop2_base::Interner,
    diags: &mut DiagnosticSink,
) {
    for ann in &file.file_annotations {
        let Some(last) = ann.path.segments.last() else {
            continue;
        };
        let name = interner.resolve(last.symbol);
        if name == "Deprecated" {
            diags.push(diagnostics::builtin_annotation_invalid_target(
                "@Deprecated",
                "函数 / 类型 / 属性",
                last.span,
            ));
        }
    }
}

/// 内建注解目标检查：`@InteriorMutable` 只能用于 class/struct，不能用于 typealias/val/fun。
fn check_builtin_annotation_targets(
    item: &crate::syntax::ast::Item,
    interner: &scoop2_base::Interner,
    diags: &mut DiagnosticSink,
) {
    use crate::syntax::ast::ItemKind;
    let anns = item_annotations(item);
    for ann in anns {
        let Some(last) = ann.path.segments.last() else {
            continue;
        };
        let name = interner.resolve(last.symbol);
        let span = last.span;
        if name == "InteriorMutable"
            && !matches!(&item.kind, ItemKind::Type(d) if matches!(d.kind, crate::syntax::ast::TypeKind::Class | crate::syntax::ast::TypeKind::Struct))
        {
            diags.push(diagnostics::builtin_annotation_invalid_target(
                "@InteriorMutable",
                "struct / class 类型声明",
                span,
            ));
        }
        // `@AllowIntrinsic` 只能用于 文件/模块（任何 item 级使用均非法）。
        if name == "AllowIntrinsic" {
            diags.push(diagnostics::builtin_annotation_invalid_target(
                "@AllowIntrinsic",
                "文件 / 模块",
                span,
            ));
        }
        // `@Deprecated` 只能用于 函数/类型/属性。
        if name == "Deprecated"
            && !matches!(
                &item.kind,
                ItemKind::Fun(_) | ItemKind::Type(_) | ItemKind::Val(_)
            )
        {
            diags.push(diagnostics::builtin_annotation_invalid_target(
                "@Deprecated",
                "函数 / 类型 / 属性",
                span,
            ));
        }
    }
}

/// 已知内建注解名（不需解析为 annotation class）。
const BUILTIN_ANNOTATIONS: &[&str] = &[
    "Intrinsic",
    "Extern",
    "Unsafe",
    "Safe",
    "NoGC",
    "Global",
    "ThreadLocal",
    "CLayout",
    "TailRec",
    "CallingConvention",
    "Deprecated",
    "Experimental",
    "Suppress",
    "Target",
    "Retention",
    "ReleaseHook",
    "InteriorMutable",
    "ReplaceWith",
];

/// 扫描所有注解使用：对非内建注解，检查是否解析为 annotation class；
/// 若不解析为任何已知类型 → unresolved_annotation_type。
pub(crate) fn check_annotation_uses(
    anns: &[crate::syntax::ast::AnnotationUse],
    env: &TypeEnv,
    package_prefix: &str,
    diags: &mut DiagnosticSink,
) {
    for ann in anns {
        let Some(last) = ann.path.segments.last() else {
            continue;
        };
        let name = env.interner.resolve(last.symbol);
        // 内建注解跳过。
        if BUILTIN_ANNOTATIONS.contains(&name) {
            continue;
        }
        let last_text = name.to_string();
        let name_text: String = ann
            .path
            .segments
            .iter()
            .map(|s| env.interner.resolve(s.symbol))
            .collect::<Vec<_>>()
            .join(".");
        // FQN 候选：全路径 / 短名 + package prefix / 短名 + scoop.core。
        let mut candidates = vec![name_text.clone()];
        if !name_text.contains('.') {
            if !package_prefix.is_empty() {
                candidates.push(format!("{package_prefix}.{last_text}"));
            }
            candidates.push(format!("scoop.core.{last_text}"));
        }
        let mut resolved = false;
        for fqn_text in &candidates {
            if let Some(fqn) = env.interner.get(fqn_text)
                && let Some(sym) = env.index.lookup_type(fqn)
            {
                if !sym
                    .modifiers
                    .contains(crate::syntax::ast::ModifierKind::Annotation)
                {
                    diags.push(diagnostics::annotation_type_is_not_annotation_class(
                        &name_text, last.span,
                    ));
                }
                resolved = true;
                break;
            }
        }
        if !resolved {
            diags.push(diagnostics::unresolved_annotation_type(
                &name_text, last.span,
            ));
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

/// 闭合 effect row（`...!`）不允许引用 effect row 变量：闭合行必须是完全已知的 effect 集合。
fn check_closed_effect_row_no_row_var(d: &crate::syntax::ast::FunDecl, diags: &mut DiagnosticSink) {
    use crate::syntax::ast::EffectRowExpr;
    let Some(eff): Option<&EffectRowExpr> = d.effect.as_ref() else {
        return;
    };
    if eff.closed.is_none() {
        return;
    }
    // 该声明的 eff row 变量名（`<eff E>`）。
    let Some(row_var) = d
        .type_params
        .as_ref()
        .and_then(|tp| tp.effect_row.as_ref())
        .map(|er| er.name.symbol)
    else {
        return;
    };
    for term in &eff.terms {
        if term.path.segments.last().map(|s| s.symbol) == Some(row_var) {
            diags.push(diagnostics::closed_effect_row_contains_row_var(eff.span));
            return;
        }
    }
}

/// class 的 class 超类必须是 `open` 或 `abstract`（接口超类不受限）。
fn check_superclass_open(
    d: &crate::syntax::ast::TypeDecl,
    name_sym: scoop2_base::Symbol,
    env: &TypeEnv,
    package_prefix: &str,
    diags: &mut DiagnosticSink,
) {
    use crate::resolve::symbol::NominalCategory;
    use crate::syntax::ast::ModifierKind;
    let name_text = env.interner.resolve(name_sym);
    let fqn_text = if package_prefix.is_empty() {
        name_text.to_string()
    } else {
        format!("{package_prefix}.{name_text}")
    };
    let Some(derived_fqn) = env.interner.get(&fqn_text) else {
        return;
    };
    let bases = env.index.supertypes_of(derived_fqn);
    for (i, st) in d.supertypes.iter().enumerate() {
        let Some(&base_fqn) = bases.get(i) else {
            continue;
        };
        let is_class = matches!(env.index.category(base_fqn), Some(NominalCategory::Class));
        if !is_class {
            continue;
        }
        let is_open = env
            .index
            .lookup_type(base_fqn)
            .map(|b| {
                b.modifiers.contains(ModifierKind::Open)
                    || b.modifiers.contains(ModifierKind::Abstract)
            })
            .unwrap_or(false);
        if !is_open {
            diags.push(diagnostics::superclass_not_open(st.span));
        }
    }
}

/// class 成员的 override 校验（M6）：
/// - `override` 必须命中签名匹配的超类/接口方法（否则 override_target_not_found）；
/// - 命中的 base 方法必须是 open/abstract/interface（否则 override_non_open_method）；
/// - 命中 open base 但未声明 override → missing_override；
/// - 非_override 同签名且 base 非 open → override_non_open_method；
/// - 覆盖方法 effect row ⊄ base（class 具体行）→ override_effect_row_not_contained。
fn check_overrides(
    d: &crate::syntax::ast::TypeDecl,
    env: &mut TypeEnv,
    imports: &crate::resolve::imports::ImportTable,
    diags: &mut DiagnosticSink,
    package_prefix: &str,
) {
    use crate::resolve::symbol::{ModifierSet, NominalCategory};
    use crate::syntax::ast::{ModifierKind, TypeMemberKind};
    let name_text = env.interner.resolve(d.name.symbol);
    let fqn_text = if package_prefix.is_empty() {
        name_text.to_string()
    } else {
        format!("{package_prefix}.{name_text}")
    };
    let Some(derived_fqn) = env.interner.get(&fqn_text) else {
        return;
    };
    let bases: Vec<scoop2_base::Symbol> = env.index.supertypes_of(derived_fqn).to_vec();
    if bases.is_empty() {
        return;
    }
    let Some(body) = &d.body else {
        return;
    };
    let tp_map = env::build_tp_map(d.type_params.as_ref());
    let unit_ty = env.store.unit();
    // 接口超类的 use-site effect 行实参（`Disposable<eff Pure>` 的 `Pure`），按 base FQN 索引。
    // 用于接口实现方法的 effect 行代入（eff 形参 → 实参）。
    let eff_args: std::collections::HashMap<
        scoop2_base::Symbol,
        Option<crate::syntax::ast::EffectRowExpr>,
    > = d
        .supertypes
        .iter()
        .enumerate()
        .filter_map(|(i, st)| {
            let base = *bases.get(i)?;
            use crate::syntax::ast::{TypeArgKind, TypeRefKind};
            let arg = match &st.ty.kind {
                TypeRefKind::Path { args, .. } => args.iter().find_map(|a| match &a.kind {
                    TypeArgKind::Effect(e) => Some(e.clone()),
                    _ => None,
                }),
                _ => None,
            };
            Some((base, arg))
        })
        .collect();
    for m in &body.members {
        let TypeMemberKind::Fun(f) = &m.kind else {
            continue;
        };
        // 方法级类型参数的虚方法由 virtual_method_cannot_be_generic 单独检查。
        if f.type_params.is_some() {
            continue;
        }
        // 覆盖方法的参数类型（降级）。
        let m_params: Vec<crate::ty::TypeId> = {
            let mut lower = crate::typecheck::lower::TypeLowering::new(
                env,
                imports,
                tp_map.clone(),
                package_prefix.to_string(),
                diags,
            );
            f.params
                .iter()
                .map(|p| match &p.ty {
                    Some(t) => lower.lower(t),
                    None => unit_ty,
                })
                .collect()
        };
        let has_override = f.modifiers.iter().any(|x| x.kind == ModifierKind::Override);
        // 在超类型链中查找签名匹配的 base 方法。
        // matched = (modifiers, base 方法 effect 行, base 是否接口, base FQN)。
        let mut matched: Option<(
            ModifierSet,
            Option<crate::syntax::ast::EffectRowExpr>,
            bool,
            scoop2_base::Symbol,
        )> = None;
        for &base in &bases {
            let base_is_interface =
                matches!(env.index.category(base), Some(NominalCategory::Interface));
            if let Some(sigs) = env.member_signatures(base, f.name.symbol) {
                for bs in sigs {
                    if params_match(&env.store, &bs.params, &m_params) {
                        matched = Some((bs.modifiers, bs.effect.clone(), base_is_interface, base));
                        break;
                    }
                }
            }
            if matched.is_some() {
                break;
            }
        }
        match (has_override, matched) {
            (true, None) => {
                diags.push(diagnostics::override_target_not_found(f.name.span));
            }
            (true, Some((bmods, beff, base_iface, base_fqn))) => {
                let base_open = bmods.contains(ModifierKind::Open)
                    || bmods.contains(ModifierKind::Abstract)
                    || base_iface;
                if !base_open {
                    diags.push(diagnostics::override_non_open_method(f.name.span));
                } else {
                    check_override_effect_containment(
                        f,
                        beff.as_ref(),
                        base_iface,
                        base_fqn,
                        &eff_args,
                        env,
                        diags,
                    );
                }
            }
            (false, Some((bmods, beff, base_iface, base_fqn))) => {
                if base_iface {
                    // 实现接口方法不需要 override；仅校验 effect containment（代入 eff 形参）。
                    check_override_effect_containment(
                        f,
                        beff.as_ref(),
                        base_iface,
                        base_fqn,
                        &eff_args,
                        env,
                        diags,
                    );
                } else {
                    let base_open = bmods.contains(ModifierKind::Open)
                        || bmods.contains(ModifierKind::Abstract);
                    if base_open {
                        diags.push(diagnostics::missing_override(f.name.span));
                    } else {
                        diags.push(diagnostics::override_non_open_method(f.name.span));
                    }
                }
            }
            (false, None) => {}
        }
    }
}

/// 覆盖/实现方法的 effect containment：R_over ⊆ R_base。
/// - class 具体 base 行：直接比较；
/// - interface base：把 base 方法 effect 行中的 eff 形参（非已知 effect 的项）
///   用超类的 use-site eff 实参代入后再比较。
fn check_override_effect_containment(
    f: &crate::syntax::ast::FunDecl,
    base_eff: Option<&crate::syntax::ast::EffectRowExpr>,
    base_iface: bool,
    base_fqn: scoop2_base::Symbol,
    eff_args: &std::collections::HashMap<
        scoop2_base::Symbol,
        Option<crate::syntax::ast::EffectRowExpr>,
    >,
    env: &TypeEnv,
    diags: &mut DiagnosticSink,
) {
    let over_effs = expr::extract_effect_row_names(f.effect.as_ref(), env.interner);
    let base_effs = if base_iface {
        let eff_arg = eff_args.get(&base_fqn).cloned().flatten();
        substituted_effect_names(base_eff, eff_arg.as_ref(), env)
    } else {
        expr::extract_effect_row_names(base_eff, env.interner)
    };
    if !over_effs.is_subset(&base_effs) {
        let span = f.effect.as_ref().map(|e| e.span).unwrap_or(f.name.span);
        diags.push(diagnostics::override_effect_row_not_contained(span));
    }
}

/// 接口 base 方法 effect 行代入：非已知 effect 的项（eff 形参）用 use-site eff 实参替换。
fn substituted_effect_names(
    base_eff: Option<&crate::syntax::ast::EffectRowExpr>,
    eff_arg: Option<&crate::syntax::ast::EffectRowExpr>,
    env: &TypeEnv,
) -> std::collections::HashSet<String> {
    use crate::resolve::symbol::NominalCategory;
    let mut set = std::collections::HashSet::new();
    let Some(base_eff) = base_eff else {
        return set;
    };
    for term in &base_eff.terms {
        let Some(seg) = term.path.segments.last() else {
            continue;
        };
        let n = env.interner.resolve(seg.symbol);
        let s = n.strip_prefix("scoop.core.").unwrap_or(n);
        if s == "Pure" {
            continue;
        }
        // 已知 effect 类型（prelude Raise/RuntimeError 或 Index Effect 类别）→ 保留。
        let known = matches!(s, "Raise" | "RuntimeError")
            || env
                .interner
                .get(s)
                .and_then(|sym| env.index.category(sym))
                .is_some_and(|c| matches!(c, NominalCategory::Effect));
        if known {
            set.insert(s.to_string());
        } else if let Some(arg) = eff_arg {
            // eff 形参 → 用 use-site eff 实参代入。
            set.extend(expr::extract_effect_row_names(Some(arg), env.interner));
        }
    }
    set
}

/// 具体类型（class/struct/object/enum）的成员函数必须提供函数体；
/// interface / effect 操作 / `abstract` / `@Intrinsic` / `@Extern` 方法可省略。
fn check_member_funs_have_body(
    d: &crate::syntax::ast::TypeDecl,
    interner: &scoop2_base::Interner,
    diags: &mut DiagnosticSink,
) {
    use crate::syntax::ast::{ModifierKind, TypeMemberKind};
    let Some(body) = &d.body else {
        return;
    };
    let exempt_kind = matches!(
        d.kind,
        crate::syntax::ast::TypeKind::Interface | crate::syntax::ast::TypeKind::Effect
    );
    for m in &body.members {
        if let TypeMemberKind::Fun(fd) = &m.kind
            && fd.body.is_none()
            && !exempt_kind
            && !fd
                .modifiers
                .iter()
                .any(|x| x.kind == ModifierKind::Abstract)
            && !has_annotation(&fd.annotations, "Intrinsic", interner)
            && !has_annotation(&fd.annotations, "Extern", interner)
        {
            diags.push(diagnostics::fun_must_have_body_detail(
                "普通成员函数必须提供函数体",
                fd.name.span,
            ));
        }
    }
}

/// 签名参数匹配：数量相等；若 base 含 owner 类型参数（跨泛型边界），仅按数量匹配。
fn params_match(
    store: &crate::ty::TypeStore,
    base_params: &[crate::ty::TypeId],
    other_params: &[crate::ty::TypeId],
) -> bool {
    if base_params.len() != other_params.len() {
        return false;
    }
    let base_has_tp = base_params
        .iter()
        .any(|p| matches!(store.kind(*p), crate::ty::TypeKind::Param(_)));
    if base_has_tp {
        return true;
    }
    base_params == other_params
}

/// class 必须实现所有接口超类要求的成员方法（签名匹配）。
fn check_interface_impl_complete(
    d: &crate::syntax::ast::TypeDecl,
    env: &TypeEnv,
    diags: &mut DiagnosticSink,
    package_prefix: &str,
) {
    use crate::resolve::symbol::NominalCategory;
    let name_text = env.interner.resolve(d.name.symbol);
    let fqn_text = if package_prefix.is_empty() {
        name_text.to_string()
    } else {
        format!("{package_prefix}.{name_text}")
    };
    let Some(derived_fqn) = env.interner.get(&fqn_text) else {
        return;
    };
    let bases = env.index.supertypes_of(derived_fqn);
    for (i, st) in d.supertypes.iter().enumerate() {
        let Some(&base_fqn) = bases.get(i) else {
            continue;
        };
        if !matches!(
            env.index.category(base_fqn),
            Some(NominalCategory::Interface)
        ) {
            continue;
        }
        let Some(iface_methods) = env.member_method_table(base_fqn) else {
            continue;
        };
        // 该接口要求的方法是否在类中实现（含类自身的重载签名匹配）。
        let mut missing = false;
        'outer: for (&method_name, iface_sigs) in iface_methods {
            let impl_sigs = env
                .member_signatures(derived_fqn, method_name)
                .unwrap_or(&[]);
            for isig in iface_sigs {
                // interface default 方法（带 body）不必由实现类提供。
                if isig.has_body {
                    continue;
                }
                let implemented = impl_sigs
                    .iter()
                    .any(|s| params_match(&env.store, &isig.params, &s.params));
                if !implemented {
                    missing = true;
                    break 'outer;
                }
            }
        }
        if missing {
            diags.push(diagnostics::missing_interface_member(st.span));
        }
    }
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
    // 闭合 effect row（`...!`）不允许引用 effect row 变量（`eff E`）—— header 级检查。
    check_closed_effect_row_no_row_var(d, diags);
    // 函数参数必须显式标注类型（无参数类型推断）。
    for p in &d.params {
        if p.ty.is_none() {
            diags.push(diagnostics::missing_type_annotation(p.name.span));
        }
    }
    let Some(body) = &d.body else {
        // 即便无 body，where 子句仍需校验（header 检查）。
        check_where_clause(
            d.where_clause.as_ref(),
            d.type_params.as_ref(),
            env,
            package_prefix,
            diags,
        );
        return;
    };
    // where 子句校验（目标在当前声明 / 无重复）。
    check_where_clause(
        d.where_clause.as_ref(),
        d.type_params.as_ref(),
        env,
        package_prefix,
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
        d.effect.as_ref(),
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
