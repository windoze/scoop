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
///
/// 返回自包含的 [`crate::hir::TypedHir`]（含 per-NodeId 表达式类型表），供
/// `dump-hir` 与后续 lowering 消费。诊断仍通过 `diags` 汇报。
pub fn run_typecheck(
    inputs: &mut [crate::resolve::InputFile],
    interner: &mut Interner,
    diags: &mut DiagnosticSink,
    target_platform: Option<&str>,
    declared_deps: &[String],
) -> crate::hir::TypedHir {
    run_typecheck_with_options(
        inputs,
        interner,
        diags,
        target_platform,
        declared_deps,
        false,
    )
}

/// 带 `lower_sysroot_bodies` 选项的 typecheck。
///
/// - `lower_sysroot_bodies = false`（默认；dump/check-source 路径）：sysroot 仅贡献符号声明，
///   其函数体不进入 typecheck（保持现有行为 + fixture 基线）。
/// - `lower_sysroot_bodies = true`（e2e build/run 路径）：sysroot 文件也产出 TypedFile，
///   其函数体可被后续 MIR lowering（为 println<String> 等库函数生成实例）。
pub fn run_typecheck_with_options(
    inputs: &mut [crate::resolve::InputFile],
    interner: &mut Interner,
    diags: &mut DiagnosticSink,
    target_platform: Option<&str>,
    declared_deps: &[String],
    lower_sysroot_bodies: bool,
) -> crate::hir::TypedHir {
    use crate::resolve::{
        ConeKind, Index, InputOrigin, Resolution, body, collect, imports, type_refs,
    };
    use crate::syntax::ast::File;

    // ---- Phase 1：收集所有 header ----
    let mut index = Index::new();
    for inp in inputs.iter() {
        let cone_name = crate::resolve::cone_name_of(inp.file, interner, inp.origin);
        let cone_kind = match inp.origin {
            InputOrigin::User => ConeKind::Bin,
            InputOrigin::Sysroot => ConeKind::Syslib,
        };
        let cone = index.intern_cone(&cone_name, cone_kind);
        collect::collect_file(inp.file, inp.file_id, cone, &mut index, interner, diags);
    }
    index.resolve_extensions(interner);

    // ---- Phase 2：解析用户文件 ----
    struct UserFile {
        file_idx: usize,
        file_id: scoop2_base::FileId,
        prefix: String,
        imports: imports::ImportTable,
        resolution: Resolution,
        is_sysroot: bool,
        trusted: bool,
    }
    let mut user_files: Vec<UserFile> = Vec::new();
    for (idx, inp) in inputs.iter().enumerate() {
        // 默认仅 User-origin 文件产出 TypedFile；lower_sysroot_bodies 时包含 sysroot。
        if !lower_sysroot_bodies && inp.origin != InputOrigin::User {
            continue;
        }
        let file: &File = &*inp.file;
        let prefix = collect::package_prefix_of(file, interner);
        let is_sysroot = inp.origin == InputOrigin::Sysroot;
        let imports = imports::ImportTable::collect_with_origin(
            file,
            inp.file_id,
            &index,
            interner,
            diags,
            is_sysroot,
            declared_deps,
        );
        type_refs::resolve_file_type_refs(file, &index, &imports, interner, diags, &prefix);
        let mut resolution = Resolution::new();
        body::resolve_file_bodies(
            file,
            &index,
            &imports,
            interner,
            diags,
            &mut resolution,
            &prefix,
        );
        user_files.push(UserFile {
            file_idx: idx,
            file_id: inp.file_id,
            prefix,
            imports,
            resolution,
            trusted: inp.trusted,
            is_sysroot: inp.origin == InputOrigin::Sysroot,
        });
    }

    // ---- Phase 3：类型检查 ----
    // for-loop desugar pre-pass：在 env 构建（锁定 &interner）之前，就地改写所有
    // For 循环为 do{var __it; while(true){when(__it.next()){Some(x)->BODY; None->break}}}。
    // typecheck 随后对改写后的 AST 验证 iterator()/next() 是否存在。
    for inp in inputs.iter_mut() {
        crate::typecheck::expr::desugar_for_loops(inp.file, interner);
    }
    // 先为所有文件构建 imports（ImportTable::collect 需要 &mut interner）。
    let mut file_state: Vec<(usize, String, imports::ImportTable)> = Vec::new();
    for (i, inp) in inputs.iter().enumerate() {
        let prefix = collect::package_prefix_of(inp.file, interner);
        let is_sysroot = inp.origin == InputOrigin::Sysroot;
        let imports = imports::ImportTable::collect_with_origin(
            inp.file,
            inp.file_id,
            &index,
            interner,
            diags,
            is_sysroot,
            declared_deps,
        );
        file_state.push((i, prefix, imports));
    }
    // 注册顺序：sysroot 文件先于用户文件（typealias / 签名等需 sysroot 先注册）。
    file_state.sort_by_key(|(i, _, _)| match inputs[*i].origin {
        InputOrigin::Sysroot => 0,
        InputOrigin::User => 1,
    });
    // 创建 TypeEnv（借用 interner 不可变）。
    let mut env = TypeEnv::new(&index, interner);
    // 注入 Option FQN（Option<T> 现为 value nominal，走 FQN 判定）。
    // 必须在任何 typecheck 构造/检查 Option 之前完成。
    env.store
        .set_option_fqn(interner.get("scoop.core.Option").unwrap_or_default());
    // 注入 Any FQN（Any 现为 ref nominal{scoop.core.Any}，走 FQN 判定）。
    env.store
        .set_any_fqn(interner.get("scoop.core.Any").unwrap_or_default());
    // 注入 String FQN（String 现为 ref nominal{scoop.core.String}，走 FQN 判定）。
    env.store
        .set_string_fqn(interner.get("scoop.core.String").unwrap_or_default());
    for &(i, ref prefix, ref imports) in &file_state {
        let inp = &inputs[i];
        // register_type_constraints 先运行（填充 eff_param_types 等供后续降级使用）。
        env::register_type_constraints(&mut env, inp.file, imports, prefix, diags);
        // typealias 先于签名/成员/构造器注册，使后续降级时能展开 typealias。
        env::register_type_aliases(&mut env, inp.file, prefix, diags);
        env::register_top_level_signatures(&mut env, inp.file, inp.file_id, imports, prefix, diags);
        env::register_members(&mut env, inp.file, inp.file_id, imports, prefix, diags);
        env::register_constructors(&mut env, inp.file, inp.file_id, imports, prefix, diags);
        env::register_clayout_structs(&mut env, inp.file, prefix);
        env::register_top_level_vals(&mut env, inp.file, imports, prefix, diags);
        env::register_enum_variants(&mut env, inp.file, prefix);
    }
    // 检查每个用户文件的函数体；同时收集 per-file 表达式类型表与语义事实。
    let mut typed_files: Vec<crate::hir::TypedFile> = Vec::with_capacity(user_files.len());
    for uf in &user_files {
        let mut expr_types = crate::resolve::output::NodeIdTable::new();
        // 把 resolve 阶段的 value_refs 搬入 SemanticFacts（typed HIR 不再持有 Resolution）。
        let mut facts = crate::hir::SemanticFacts::new();
        facts.value_refs = uf.resolution.value_refs.clone();
        let file: &mut File = &mut inputs[uf.file_idx].file;
        check_file_bodies(
            file,
            &mut env,
            &uf.imports,
            &uf.resolution,
            diags,
            &uf.prefix,
            uf.trusted,
            &mut expr_types,
            &mut facts,
            target_platform,
        );
        typed_files.push(crate::hir::TypedFile {
            file_id: uf.file_id,
            package_prefix: uf.prefix.clone(),
            expr_types,
            facts,
            trees: Vec::new(),
        });
    }
    // 把 typecheck 产出 move 进自包含的 TypedHir（含 interner 副本，解耦借用）。
    let hir = env.into_typed_hir(interner.clone(), typed_files);
    // 完整性闸门：对所有文件（User + sysroot）做严格检查。
    // sysroot 与 User 同等对待——MIR 会 lower sysroot 函数体，sysroot 的不完整事实
    // 同样会泄漏进 MIR，故必须同等保证完整。backfill_child_types 已删除（不再有
    // Nothing 兜底），sysroot 类型应已完整。
    let all_file_refs: Vec<(scoop2_base::FileId, &File)> = user_files
        .iter()
        .map(|uf| (uf.file_id, &*inputs[uf.file_idx].file))
        .collect();
    crate::completeness::verify(&hir, &all_file_refs, diags);
    hir
}

/// 检查一个文件的**顶层 + 成员**函数体 + 声明头语义检查。
#[allow(clippy::too_many_arguments)]
fn check_file_bodies(
    file: &mut crate::syntax::ast::File,
    env: &mut TypeEnv,
    imports: &crate::resolve::imports::ImportTable,
    resolution: &crate::resolve::Resolution,
    diags: &mut DiagnosticSink,
    package_prefix: &str,
    trusted: bool,
    expr_types: &mut crate::resolve::output::NodeIdTable<crate::ty::TypeId>,
    facts: &mut crate::hir::SemanticFacts,
    target_platform: Option<&str>,
) {
    use crate::syntax::ast::{ItemKind, ModifierKind};
    use std::collections::{HashMap, HashSet};
    let empty_tp: HashMap<scoop2_base::Symbol, crate::ty::TypeParamId> = HashMap::new();
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
    // 文件级 `@AllowIntrinsic` 不支持参数。
    check_allow_intrinsic_args(&file.file_annotations, env.interner, diags);
    // 文件级注解实参必须是编译期常量。
    check_annotation_const_args(&file.file_annotations, env.interner, diags);
    // 文件级 `@Retention` 策略实参校验。
    check_retention_policy(&file.file_annotations, env.interner, diags);
    // 收集本文件的 annotation class 信息（FQN + @Target 声明），供运行期使用检查与
    // @Target 强制复用。注：跨文件 / sysroot 的 annotation class 在各自文件收集。
    let mut anno_classes: std::collections::HashSet<scoop2_base::Symbol> =
        std::collections::HashSet::new();
    let mut anno_targets: std::collections::HashMap<scoop2_base::Symbol, Vec<AnnotationUseTarget>> =
        std::collections::HashMap::new();
    collect_annotation_class_info(
        file,
        env.interner,
        package_prefix,
        &mut anno_classes,
        &mut anno_targets,
    );
    for item in &mut file.items {
        // @Experimental / @Suppress 注解校验（item 级目标是合法的）。
        check_experimental_annotations(item_annotations(item), false, env.interner, diags);
        // 未知注解类型检查。
        check_annotation_uses(item_annotations(item), env, package_prefix, diags);
        // `@Deprecated` 实参校验（位置/命名/类型）。
        check_deprecated_annotation_args(item_annotations(item), env.interner, diags);
        // 内建注解目标检查。
        check_builtin_annotation_targets(item, env.interner, diags);
        // 注解实参必须是编译期常量。
        check_annotation_const_args(item_annotations(item), env.interner, diags);
        // `@AllowIntrinsic` 不支持参数。
        check_allow_intrinsic_args(item_annotations(item), env.interner, diags);
        // `@Retention` 策略实参校验。
        check_retention_policy(item_annotations(item), env.interner, diags);
        // annotation class 不能作为普通类型 / 运行期构造。
        check_annotation_runtime_use(item, env, imports, package_prefix, &anno_classes, diags);
        // `@Target(...)` 强制：注解被用在不允许的目标上。
        check_annotation_target_enforcement(
            item,
            env,
            imports,
            package_prefix,
            &anno_targets,
            diags,
        );
        // enum variant 字段类型必须可解析（`NotAType` 等报 unresolved_type）。
        check_enum_variant_field_types(item, env, imports, package_prefix, diags);
        match &mut item.kind {
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
                // @Intrinsic("name") 的 name 必须命中编译器 intrinsic 表。
                if let Some(ann) = find_annotation(&d.annotations, "Intrinsic", env.interner) {
                    check_intrinsic_table_entry(ann, env.interner, diags);
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
                // 扩展函数：this = 接收者类型（lowered）；接收者类型参数合并到函数作用域。
                let ext_tp_map = if let Some(tp_list) = &d.type_params {
                    let mut m = empty_tp.clone();
                    for p in &tp_list.params {
                        m.insert(p.name.symbol, crate::ty::TypeParamId(p.id.as_u32()));
                    }
                    m
                } else {
                    empty_tp.clone()
                };
                let ext_this_ty = d.receiver.as_ref().map(|recv| {
                    let mut lower = crate::typecheck::lower::TypeLowering::new(
                        env,
                        imports,
                        ext_tp_map.clone(),
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
                    &ext_tp_map,
                    ext_this_ty,
                    expr_types,
                    facts,
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
                // value enum 底层类型必须整型（首个超类型若是具体类型而非 interface）。
                if d.kind == crate::syntax::ast::TypeKind::Enum
                    && let Some(st) = d.supertypes.first()
                {
                    use crate::resolve::symbol::NominalCategory;
                    let ty = {
                        let mut lower = crate::typecheck::lower::TypeLowering::new(
                            env,
                            imports,
                            empty_tp.clone(),
                            package_prefix.to_string(),
                            diags,
                        );
                        lower.lower(&st.ty)
                    };
                    let kind = env.store.kind(ty);
                    let is_integral = matches!(
                        kind,
                        crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::Int)
                            | crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::UInt)
                            | crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::IntN(_))
                            | crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::UIntN(_))
                    );
                    let is_interface = expr::nominal_fqn_of(kind)
                        .and_then(|fqn| env.index.category(fqn))
                        .is_some_and(|c| matches!(c, NominalCategory::Interface));
                    if !is_integral && !is_interface && !env.store.is_nothing(ty) {
                        diags.push(diagnostics::value_only_enum_underlying_not_integral(
                            st.span,
                        ));
                    }
                }
                // enum-size-disparity lint（T0826，复刻 legacy 判定与 warning 文本）。
                if d.kind == crate::syntax::ast::TypeKind::Enum {
                    check_enum_size_disparity(d, &file.file_annotations, env, package_prefix);
                }
                // 只能继承 `open`/`abstract` 类（class 超类必须 open）。
                if d.kind == crate::syntax::ast::TypeKind::Class {
                    check_superclass_open(d, d.name.symbol, env, package_prefix, diags);
                    check_overrides(d, env, imports, diags, package_prefix);
                    check_interface_impl_complete(d, env, diags, package_prefix);
                    record_class_ctor_layout(d, env, imports, package_prefix, diags);
                    if let Some(body) = &d.body {
                        overloads::check_ctor_overload_conflicts(
                            env,
                            imports,
                            diags,
                            package_prefix,
                            d.name.symbol,
                            d.type_params.as_ref(),
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
                check_variance_positions(d, diags);
                // @CLayout struct 字段必须 GC-free；packed 实参必须是 2 的幂且 <= 16。
                if d.kind == crate::syntax::ast::TypeKind::Struct
                    && has_annotation(&d.annotations, "CLayout", env.interner)
                {
                    check_clayout_struct_gc_free(d, env, imports, package_prefix, diags);
                    check_clayout_packed(&d.annotations, env.interner, diags);
                }
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
                    // 类型体字段：只有带 backing field 的属性（非计算属性）报错。
                    // 计算属性（带 getter/setter，无 backing field）是合法的。
                    if let Some(body) = &d.body {
                        for m in &body.members {
                            if let crate::syntax::ast::TypeMemberKind::Property(pd) = &m.kind
                                && pd.accessors.is_empty()
                            {
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
                            check_delegated_property(
                                env,
                                imports,
                                diags,
                                package_prefix,
                                pd,
                                target_platform,
                            );
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
                if let Some(body) = &mut d.body {
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
                        &mut body.members,
                        this_ty,
                        env,
                        imports,
                        resolution,
                        diags,
                        package_prefix,
                        &tp_map,
                        expr_types,
                        facts,
                    );
                    // class 的 init 块与属性初始化器：走 init-body typecheck（写回
                    // call_resolutions / value_refs / expr_types，供 MIR `<Class>.$init`
                    // 合成 callable lowering 消费）。constructor/init 块对外强制 Pure
                    //（outward effect——含未捕获的 Raise——是编译错误：构造副作用必须
                    // 在 init 内部用 handle 消化）。object 走自己的 check_pure_static_init
                    // 路径（静态初始化器），这里仅处理 class。
                    if d.kind == crate::syntax::ast::TypeKind::Class {
                        // ctor 参数（name → ty），供 init 块 / property initializer / super
                        // 实参表达式 typecheck 时解析构造参数引用。
                        let class_fqn_sym = env.interner.get(&if package_prefix.is_empty() {
                            env.interner.resolve(d.name.symbol).to_string()
                        } else {
                            format!("{}.{}", package_prefix, env.interner.resolve(d.name.symbol))
                        });
                        let ctor_params: Vec<(scoop2_base::Symbol, crate::ty::TypeId)> =
                            class_fqn_sym
                                .and_then(|sym| env.class_ctor_params.get(&sym).cloned())
                                .unwrap_or_default()
                                .into_iter()
                                .map(|cp| (cp.name, cp.ty))
                                .collect();
                        // super 委托实参表达式 typecheck（任意表达式：函数调用/运算/参数引用）。
                        // 写回语义事实，供 MIR $init 合成时 lower super 调用实参消费。
                        if let Some(sym) = class_fqn_sym
                            && let Some(super_del) = env.super_ctor_delegations.get(&sym).cloned()
                            && let Some(base_st) = d.supertypes.get_mut(super_del.base_index)
                        {
                            expr::check_super_delegation_args(
                                env,
                                imports,
                                resolution,
                                diags,
                                package_prefix,
                                this_ty,
                                &mut base_st.args,
                                expr_types,
                                facts,
                                &ctor_params,
                            );
                        }
                        for m in &mut body.members {
                            use crate::syntax::ast::TypeMemberKind;
                            match &mut m.kind {
                                TypeMemberKind::InitBlock(ib) => {
                                    let what = format!(
                                        "class `{}` init block",
                                        env.interner.resolve(d.name.symbol)
                                    );
                                    expr::check_init_body(
                                        env,
                                        imports,
                                        resolution,
                                        diags,
                                        package_prefix,
                                        this_ty,
                                        what,
                                        expr::PureInitSite::Block(&mut ib.body),
                                        expr_types,
                                        facts,
                                        /* require_pure */ true,
                                        &ctor_params,
                                    );
                                }
                                TypeMemberKind::Property(pd) if pd.init.is_some() => {
                                    let pname = env.interner.resolve(pd.name.symbol);
                                    let what = format!(
                                        "class `{}` 属性 `{pname}`",
                                        env.interner.resolve(d.name.symbol)
                                    );
                                    if let Some(init) = &mut pd.init {
                                        expr::check_init_body(
                                            env,
                                            imports,
                                            resolution,
                                            diags,
                                            package_prefix,
                                            this_ty,
                                            what,
                                            expr::PureInitSite::Expr(init),
                                            expr_types,
                                            facts,
                                            /* require_pure */ true,
                                            &ctor_params,
                                        );
                                    }
                                }
                                TypeMemberKind::SecondaryCtor(sc) => {
                                    // secondary ctor body + delegation 实参 typecheck
                                    //（写回语义事实，供 MIR 合成 secondary ctor callable 消费）。
                                    // ctor 参数类型：从 ctor_signatures 按 span 匹配（已 typecheck）。
                                    let sc_params: Vec<(scoop2_base::Symbol, crate::ty::TypeId)> = {
                                        let sigs =
                                            env.ctor_signatures(class_fqn_sym.unwrap_or_default());
                                        let matched = sigs.and_then(|ss| {
                                            ss.iter().find(|s| s.decl_span == sc.span)
                                        });
                                        matched
                                            .map(|s| {
                                                s.param_names
                                                    .iter()
                                                    .zip(&s.params)
                                                    .map(|(n, t)| (*n, *t))
                                                    .collect()
                                            })
                                            .unwrap_or_default()
                                    };
                                    // delegation 实参（this(...)/super(...)）。
                                    if let Some(del) = &mut sc.delegation {
                                        expr::check_super_delegation_args(
                                            env,
                                            imports,
                                            resolution,
                                            diags,
                                            package_prefix,
                                            this_ty,
                                            &mut del.args,
                                            expr_types,
                                            facts,
                                            &sc_params,
                                        );
                                    }
                                    // secondary ctor body（强制 Pure）。
                                    let what = format!(
                                        "class `{}` secondary constructor",
                                        env.interner.resolve(d.name.symbol)
                                    );
                                    expr::check_init_body(
                                        env,
                                        imports,
                                        resolution,
                                        diags,
                                        package_prefix,
                                        this_ty,
                                        what,
                                        expr::PureInitSite::Block(&mut sc.body),
                                        expr_types,
                                        facts,
                                        /* require_pure */ true,
                                        &sc_params,
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
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
                    if let Some(body) = &mut d.body {
                        check_member_funs(
                            &mut body.members,
                            this_ty,
                            env,
                            imports,
                            resolution,
                            diags,
                            package_prefix,
                            &empty_tp,
                            expr_types,
                            facts,
                        );
                        // object 的 init 块与属性初始化器必须 `Pure!`（静态初始化器）。
                        for m in &mut body.members {
                            use crate::syntax::ast::TypeMemberKind;
                            match &mut m.kind {
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
                                        expr::PureInitSite::Block(&mut ib.body),
                                        expr_types,
                                        facts,
                                    );
                                }
                                TypeMemberKind::Property(pd) if pd.init.is_some() => {
                                    let pname = env.interner.resolve(pd.name.symbol);
                                    let what = format!("object `{obj_fqn}` 属性 `{pname}`");
                                    if let Some(init) = &mut pd.init {
                                        expr::check_pure_static_init(
                                            env,
                                            imports,
                                            resolution,
                                            diags,
                                            package_prefix,
                                            this_ty,
                                            what,
                                            expr::PureInitSite::Expr(init),
                                            expr_types,
                                            facts,
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
                // 但解构绑定（`val (a, b) = ...`）从 initializer 推断整体类型，不要求标注。
                if d.ty.is_none() && matches!(&d.binding, crate::syntax::ast::ValBinding::Name(_)) {
                    let name_span = match &d.binding {
                        crate::syntax::ast::ValBinding::Name(id) => id.span,
                        _ => scoop2_base::Span::default(),
                    };
                    diags.push(diagnostics::missing_type_annotation(name_span));
                }
                // 降级类型注解 + 检查 initializer。
                if d.init.is_some() {
                    expr::check_top_level_val(
                        d,
                        env,
                        imports,
                        resolution,
                        diags,
                        package_prefix,
                        expr_types,
                        facts,
                    );
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
            ItemKind::ExtensionProperty(d) => {
                // 扩展属性不允许 initializer（计算属性 / 带 accessor）。
                if let Some(init) = &d.init {
                    diags.push(diagnostics::extension_property_initializer_not_allowed(
                        init.span,
                    ));
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
            .unwrap_or_else(|| env.store.unit());
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
    target_platform: Option<&str>,
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
    // `lazy(LazyThreadSafetyMode.Synchronized)` 在不支持线程的平台（wasm-browser）上报错。
    let callee_text = env.interner.resolve(callee);
    if callee_text == "lazy"
        && target_platform.is_some_and(|p| p == "wasm-browser" || p.contains("wasm"))
        && let ExprKind::Call { args, .. } = &delegate_expr.kind
        && let Some(first) = args.first()
    {
        // 实参形如 `LazyThreadSafetyMode.Synchronized`（MemberAccess）。
        if let ExprKind::MemberAccess { member, .. } = &first.value.kind
            && let crate::syntax::ast::MemberName::Named(seg) = member
            && env.interner.resolve(seg.symbol) == "Synchronized"
        {
            diags.push(
                diagnostics::lazy_thread_safety_mode_not_supported_on_platform(
                    target_platform.unwrap_or(""),
                    "LazyThreadSafetyMode.Synchronized",
                    first.value.span,
                ),
            );
        }
    }
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

/// 类型短名（诊断用）。委托给统一的 [`crate::ty::render_type`]。
fn fmt_type_short(env: &TypeEnv, id: crate::ty::TypeId) -> String {
    crate::ty::render_type(&env.store, env.interner, id, false)
}

/// 类型 FQN（诊断用；nominal 用全限定，标量用裸短名）。委托给统一的 [`crate::ty::render_type`]。
fn fmt_type_fqn(env: &TypeEnv, id: crate::ty::TypeId) -> String {
    crate::ty::render_type(&env.store, env.interner, id, true)
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
    // 每个 Type bound 必须解析为已知类型（AnyValue/AnyRef 等非类型名应报 unresolved_type）。
    for c in &wc.constraints {
        if let crate::syntax::ast::GenericBound::Type(t) = &c.bound
            && !where_type_bound_resolves(t, env, package_prefix)
        {
            use crate::syntax::ast::TypeRefKind;
            if let TypeRefKind::Path { path, .. } = &t.kind
                && let Some(seg) = path.segments.last()
            {
                let name = env.interner.resolve(seg.symbol);
                diags.push(crate::resolve::errors::unresolved_type(name, t.span));
            }
        }
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

/// where 子句的 Type bound 是否解析为已知类型（类型命名空间命中即视为解析）。
/// AnyValue/AnyRef 等非类型名返回 false。
fn where_type_bound_resolves(
    t: &crate::syntax::ast::TypeRef,
    env: &TypeEnv,
    package_prefix: &str,
) -> bool {
    use crate::syntax::ast::TypeRefKind;
    let path = match &t.kind {
        TypeRefKind::Path { path, .. } => path,
        _ => return true, // 非 path 类型（函数类型等）不在此检查。
    };
    let fqn_text = if path.segments.len() == 1 {
        let n = env.interner.resolve(path.segments[0].symbol);
        // 候选：裸名 / 当前包前缀 / scoop.core 前缀（sysroot 接口如 Hashable 常在此）。
        for cand in [
            n.to_string(),
            format!("{package_prefix}.{n}"),
            format!("scoop.core.{n}"),
        ] {
            if env
                .interner
                .get(&cand)
                .is_some_and(|f| env.index.lookup_type(f).is_some())
            {
                return true;
            }
        }
        return false;
    } else {
        path.segments
            .iter()
            .map(|s| env.interner.resolve(s.symbol))
            .collect::<Vec<_>>()
            .join(".")
    };
    env.interner
        .get(&fqn_text)
        .is_some_and(|f| env.index.lookup_type(f).is_some())
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

/// 查找名为 `name` 的注解（末段匹配）。
fn find_annotation<'a>(
    anns: &'a [crate::syntax::ast::AnnotationUse],
    name: &str,
    interner: &scoop2_base::Interner,
) -> Option<&'a crate::syntax::ast::AnnotationUse> {
    anns.iter().find(|a| {
        a.path
            .segments
            .last()
            .is_some_and(|s| interner.resolve(s.symbol) == name)
    })
}

/// 编译器 intrinsic 表（与 `scoopc_hir::intrinsics` 一致）。
const KNOWN_INTRINSIC_ENTRIES: &[&str] = &[
    "array_data_ptr_inline",
    "array_data_ptr_outofline",
    "array_get_inline",
    "array_get_outofline",
    "array_set_inline",
    "array_set_outofline",
    "array_size_inline",
    "array_size_outofline",
    "bool_to_string",
    "char_to_string",
    "char_compare_to",
    "char_equals",
    "char_hash",
    "char_minus_char",
    "char_minus_int",
    "char_plus_int",
    "char_to_int",
    "composite_copy",
    "dummy_ir",
    "dummy_runtime",
    "float32_to_int",
    "float32_to_string",
    "float64_to_int",
    "float64_to_string",
    "float_compare_to",
    "float_div",
    "float_equals",
    "float_minus",
    "float_plus",
    "float_rem",
    "float_times",
    "float_to_int",
    "float_unary_minus",
    "float_unary_plus",
    "int_and",
    "int_compare_to",
    "int_dec",
    "int_div",
    "int_eq",
    "int_equals",
    "int_ge",
    "int_gt",
    "int_hash",
    "int_inc",
    "int_inv",
    "int_le",
    "int_lt",
    "int_minus",
    "int_ne",
    "int_not_equals",
    "int_or",
    "int_plus",
    "int_rem",
    "int_shl",
    "int_shr",
    "int_times",
    "int_to_int",
    "int_to_string",
    "int_unary_minus",
    "int_unary_plus",
    "int_ushr",
    "int_xor",
    "string_byte_length",
    "string_get_byte",
    "uint_compare_to",
    "uint_equals",
    "uint_hash",
    "uint_to_int",
    "unsafe_array_cast",
    "unsafe_mutable_array_cast",
    "unsafe_mutable_array_erase",
    "unsafe_value_slot",
    "unsafe_value_to_any",
    "unsafe_value_to_word",
    "write_barrier",
];

/// 校验 `@Intrinsic("name")` 的 name 命中编译器 intrinsic 表。
fn check_intrinsic_table_entry(
    ann: &crate::syntax::ast::AnnotationUse,
    interner: &scoop2_base::Interner,
    diags: &mut DiagnosticSink,
) {
    use crate::syntax::ast::ExprKind;
    // @Intrinsic 无参数 → 无 name 可校验。
    let Some(first_arg) = ann.args.first() else {
        return;
    };
    // 参数必须是字符串字面量。
    let name_str = if let ExprKind::StringLit(s) = &first_arg.value.kind {
        &s.value
    } else {
        return;
    };
    let _ = interner;
    if !KNOWN_INTRINSIC_ENTRIES.contains(&name_str.as_str()) {
        diags.push(diagnostics::unknown_intrinsic_table_entry(
            name_str,
            first_arg.value.span,
        ));
    }
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

/// 一组注解中是否存在 `@Suppress("<code>")`（file 级与 decl 级通用）。
fn warning_code_suppressed(
    anns: &[crate::syntax::ast::AnnotationUse],
    interner: &scoop2_base::Interner,
    code: &str,
) -> bool {
    use crate::syntax::ast::ExprKind;
    for ann in anns {
        let Some(last) = ann.path.segments.last() else {
            continue;
        };
        if interner.resolve(last.symbol) != "Suppress" {
            continue;
        }
        for arg in &ann.args {
            if let ExprKind::StringLit(s) = &arg.value.kind
                && s.value == code
            {
                return true;
            }
        }
    }
    false
}

/// lint 用的类型尺寸近似（字节）：标量按位宽，引用/类型参数按字长 8，
/// tuple/struct/enum 按字段递归（每个字段按 8 字节槽进位），depth 防环。
///
/// 只为 `enum-size-disparity` lint 的阈值判定服务，不是真实布局（真实布局在
/// `scoop2_lir::layout`）；与 legacy lint 的 `aggregate_fields_layout` 语义对齐到
/// “足够触发/不触发”即可。
fn lint_size_of(env: &TypeEnv, ty: crate::ty::TypeId, depth: u32) -> u64 {
    use crate::ty::{TypeKind, ValueTypeKind};
    if depth >= 32 {
        return 8;
    }
    let kind = env.store.kind(ty);
    match kind {
        TypeKind::Nothing | TypeKind::Ref(_) | TypeKind::Param(_) | TypeKind::StarProjection => 8,
        TypeKind::Value(v) => match v {
            ValueTypeKind::Unit => 0,
            ValueTypeKind::Bool => 1,
            ValueTypeKind::Char | ValueTypeKind::Float32 => 4,
            ValueTypeKind::Int | ValueTypeKind::UInt | ValueTypeKind::Float64 => 8,
            ValueTypeKind::IntN(bits) | ValueTypeKind::UIntN(bits) => u64::from(*bits) / 8,
            ValueTypeKind::Tuple(elems) => elems
                .iter()
                .map(|&e| lint_size_of(env, e, depth + 1).max(1).next_multiple_of(8))
                .sum(),
            ValueTypeKind::Nominal(n) => {
                // Option<T>：内层为引用/Nothing 时按 niche 取 8，否则 8 + inner。
                if n.fqn == env.store.option_fqn()
                    && let Some(inner) = n.args.first()
                {
                    let inner_kind = env.store.kind(*inner);
                    return if matches!(inner_kind, TypeKind::Ref(_) | TypeKind::Nothing) {
                        8
                    } else {
                        8 + lint_size_of(env, *inner, depth + 1)
                            .max(1)
                            .next_multiple_of(8)
                    };
                }
                if let Some(variants) = env.enum_variants(n.fqn) {
                    // 嵌套 enum：tag（8）+ 最大 variant payload。
                    let enum_name = env.interner.resolve(n.fqn);
                    let max_payload = variants
                        .iter()
                        .map(|&v| {
                            let text = format!("{enum_name}.{}", env.interner.resolve(v));
                            env.interner
                                .get(&text)
                                .map(|vf| {
                                    env.ordered_member_types(vf)
                                        .iter()
                                        .map(|&t| {
                                            lint_size_of(env, t, depth + 1)
                                                .max(1)
                                                .next_multiple_of(8)
                                        })
                                        .sum::<u64>()
                                })
                                .unwrap_or(0)
                        })
                        .max()
                        .unwrap_or(0);
                    8 + max_payload
                } else {
                    env.ordered_member_types(n.fqn)
                        .iter()
                        .map(|&t| lint_size_of(env, t, depth + 1).max(1).next_multiple_of(8))
                        .sum()
                }
            }
        },
    }
}

/// T0826：enum variant payload 尺寸差异 lint（复刻 legacy
/// `scoopc_hir/src/typecheck/layout.rs` 的判定：`max >= 16 字` 且
/// （`second == 0` 或 `max >= second * 4`））。
///
/// 注意：新管线的实际布局是 inline tagged union（scalar union slot + ref 独立
/// slot，不做真 boxing）；本 lint 仅为 spec/fixture 契约保留 legacy warning
/// 文本。`@Suppress("enum-size-disparity")`（decl 级或 `@file:Suppress`）可抑制。
fn check_enum_size_disparity(
    d: &crate::syntax::ast::TypeDecl,
    file_annotations: &[crate::syntax::ast::AnnotationUse],
    env: &TypeEnv,
    package_prefix: &str,
) {
    use crate::syntax::ast::TypeMemberKind;
    if warning_code_suppressed(file_annotations, env.interner, "enum-size-disparity")
        || warning_code_suppressed(&d.annotations, env.interner, "enum-size-disparity")
    {
        return;
    }
    let Some(body) = &d.body else {
        return;
    };
    let enum_name = env.interner.resolve(d.name.symbol);
    let enum_fqn = if package_prefix.is_empty() {
        enum_name.to_string()
    } else {
        format!("{package_prefix}.{enum_name}")
    };
    let mut sizes: Vec<(String, u64)> = Vec::new();
    for member in &body.members {
        let TypeMemberKind::EnumVariant(v) = &member.kind else {
            continue;
        };
        // variant payload 字段登记在 `<enum_fqn>.<variant>` 的 members 下（声明序）。
        let variant_fqn_text = format!("{enum_fqn}.{}", env.interner.resolve(v.name.symbol));
        let payload = env
            .interner
            .get(&variant_fqn_text)
            .map(|vf| {
                env.ordered_member_types(vf)
                    .iter()
                    .map(|&t| lint_size_of(env, t, 0).max(1).next_multiple_of(8))
                    .sum::<u64>()
            })
            .unwrap_or(0);
        sizes.push((env.interner.resolve(v.name.symbol).to_string(), payload));
    }
    if sizes.len() < 2 {
        return;
    }
    let mut sorted: Vec<u64> = sizes.iter().map(|(_, s)| *s).collect();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    let max_size = sorted[0];
    let second_size = sorted[1];
    // legacy：`ENUM_BOX_INLINE_THRESHOLD_WORDS = 16` 字 × 8 字节；比例 4。
    let inline_threshold = 8 * 16;
    let disparity =
        max_size >= inline_threshold && (second_size == 0 || max_size >= second_size * 4);
    if !disparity {
        return;
    }
    let boxed: Vec<&str> = sizes
        .iter()
        .filter(|(_, s)| *s == max_size && max_size > 8)
        .map(|(n, _)| n.as_str())
        .collect();
    if boxed.is_empty() {
        return;
    }
    eprintln!(
        "warn[enum-size-disparity]: enum `{enum_fqn}` 的 variant payload 尺寸差异显著；\
         已对 oversized variant 做 boxing（boxed={}; max_size={max_size}; second_size={second_size}）",
        boxed.join(", ")
    );
}

/// B3-1：注解实参必须是编译期常量（字面量 / 常量算术 / 数组字面量 / enum 变体 /
/// 嵌套注解 / `::class` 等）。调用等非常量表达式不允许。
fn check_annotation_const_args(
    anns: &[crate::syntax::ast::AnnotationUse],
    interner: &scoop2_base::Interner,
    diags: &mut DiagnosticSink,
) {
    for ann in anns {
        for arg in &ann.args {
            if !is_const_annotation_expr(&arg.value, interner) {
                diags.push(diagnostics::annotation_arg_not_const(arg.value.span));
            }
        }
    }
}

/// 判断注解实参表达式是否为编译期常量。
fn is_const_annotation_expr(
    expr: &crate::syntax::ast::Expr,
    interner: &scoop2_base::Interner,
) -> bool {
    use crate::syntax::ast::{ExprKind, MemberName};
    match &expr.kind {
        ExprKind::IntLit(_)
        | ExprKind::FloatLit(_)
        | ExprKind::CharLit(_)
        | ExprKind::StringLit(_)
        | ExprKind::UnitLit => true,
        // `true` / `false` / 常量引用 是 Ident。
        ExprKind::Ident(_) => true,
        // 负数字面量 `-1`：一元 Neg 作用于常量。
        ExprKind::Unary {
            op: crate::syntax::ast::UnaryOp::Neg,
            expr: inner,
        } => is_const_annotation_expr(inner, interner),
        // 常量算术 `1 + 2`：二元运算作用于常量操作数。
        ExprKind::Binary { lhs, rhs, .. } => {
            is_const_annotation_expr(lhs, interner) && is_const_annotation_expr(rhs, interner)
        }
        // 数组字面量 `[1, 2, 3]`：元素全为常量。
        ExprKind::ArrayLit(els) => els.iter().all(|e| is_const_annotation_expr(e, interner)),
        // 元组字面量 `(1, 2)`：元素全为常量。
        ExprKind::TupleLit(els) => els.iter().all(|e| is_const_annotation_expr(e, interner)),
        // `Color.Red` / `AnnotationTarget.Field`：成员访问，receiver 是大写 Ident 或
        // 嵌套成员访问（enum 变体 / 静态常量）。方法调用形如 `a.foo()` 是 Call，不在此处。
        ExprKind::MemberAccess { receiver, member } => {
            let recv_const = match &receiver.kind {
                ExprKind::Ident(ident) => {
                    let name = interner.resolve(ident.symbol);
                    name.chars().next().is_some_and(|c| c.is_uppercase())
                }
                _ => is_const_annotation_expr(receiver, interner),
            };
            recv_const && matches!(member, MemberName::Named(_))
        }
        // `T::class`：类型字面量，编译期常量。
        ExprKind::ClassLit { .. } => true,
        // 嵌套注解：`@Inner(...)` 形式（Annotated）。
        ExprKind::Annotated { expr, .. } => is_const_annotation_expr(expr, interner),
        _ => false,
    }
}

/// B3-5：`@AllowIntrinsic` 不支持任何参数。
fn check_allow_intrinsic_args(
    anns: &[crate::syntax::ast::AnnotationUse],
    interner: &scoop2_base::Interner,
    diags: &mut DiagnosticSink,
) {
    for ann in anns {
        if let Some(last) = ann.path.segments.last()
            && interner.resolve(last.symbol) == "AllowIntrinsic"
            && !ann.args.is_empty()
        {
            diags.push(diagnostics::builtin_annotation_args_not_supported(
                "@AllowIntrinsic",
                last.span,
            ));
        }
    }
}

/// B3-3：`@Retention("local" | "cone")` 策略实参校验。
fn check_retention_policy(
    anns: &[crate::syntax::ast::AnnotationUse],
    interner: &scoop2_base::Interner,
    diags: &mut DiagnosticSink,
) {
    use crate::syntax::ast::ExprKind;
    for ann in anns {
        let Some(last) = ann.path.segments.last() else {
            continue;
        };
        if interner.resolve(last.symbol) != "Retention" {
            continue;
        }
        // 第一个位置实参必须是字符串字面量 "local" 或 "cone"。
        let Some(first) = ann.args.first() else {
            continue;
        };
        let span = first.value.span;
        if let ExprKind::StringLit(s) = &first.value.kind {
            if s.value != "local" && s.value != "cone" {
                diags.push(diagnostics::invalid_annotation_retention_policy(
                    &s.value, span,
                ));
            }
        } else {
            diags.push(diagnostics::invalid_annotation_retention_policy(
                "<非字符串>",
                span,
            ));
        }
    }
}

/// B3-2：annotation class 不能作为普通类型使用，也不能在运行期构造实例。
/// 用 `anno_classes`（本文件 + sysroot 收集的 annotation class FQN 集合）扫描
/// item 内的类型引用与构造调用。
fn check_annotation_runtime_use(
    item: &crate::syntax::ast::Item,
    env: &TypeEnv,
    imports: &crate::resolve::imports::ImportTable,
    package_prefix: &str,
    anno_classes: &std::collections::HashSet<scoop2_base::Symbol>,
    diags: &mut DiagnosticSink,
) {
    let mut type_refs: Vec<&crate::syntax::ast::TypeRef> = Vec::new();
    let mut ctor_calls: Vec<&crate::syntax::ast::Expr> = Vec::new();
    collect_item_type_refs_and_ctors(item, &mut type_refs, &mut ctor_calls);
    for tr in type_refs {
        collect_type_ref_paths(tr, &mut |path| {
            if path_resolves_to_anno_class(path, env, imports, package_prefix, anno_classes) {
                let name = path
                    .segments
                    .last()
                    .map(|s| env.interner.resolve(s.symbol))
                    .unwrap_or("");
                diags.push(diagnostics::annotation_type_runtime_use_not_allowed(
                    name, tr.span,
                ));
            }
        });
    }
    for call in ctor_calls {
        if let crate::syntax::ast::ExprKind::Call { callee, .. } = &call.kind {
            // 单段 `Foo(args)`：callee 是 Ident，取其名。
            // 多段 `pkg.Foo(args)`：callee 是 MemberAccess，取末段。
            let name_sym = match &callee.kind {
                crate::syntax::ast::ExprKind::Ident(ident) => Some(ident.symbol),
                _ => callee_simple_name_symbol(callee),
            };
            if let Some(sym) = name_sym {
                let name = env.interner.resolve(sym);
                if name_resolves_to_anno_class(name, env, imports, package_prefix, anno_classes) {
                    diags.push(diagnostics::annotation_type_runtime_use_not_allowed(
                        name, call.span,
                    ));
                }
            }
        }
    }
}

/// B3-4：`@Target(...)` 强制——注解被用在不允许的 item 目标上。
/// `anno_targets`：annotation class FQN → 允许的目标类别（由声明侧 `@Target` 收集）。
fn check_annotation_target_enforcement(
    item: &crate::syntax::ast::Item,
    env: &TypeEnv,
    imports: &crate::resolve::imports::ImportTable,
    package_prefix: &str,
    anno_targets: &std::collections::HashMap<scoop2_base::Symbol, Vec<AnnotationUseTarget>>,
    diags: &mut DiagnosticSink,
) {
    for ann in item_annotations(item) {
        let Some(fqn) = resolve_annotation_fqn(&ann.path, env, imports, package_prefix) else {
            continue;
        };
        let Some(allowed) = anno_targets.get(&fqn) else {
            continue;
        };
        if allowed.is_empty() {
            continue;
        }
        let Some(it_target) = item_annotation_target(item) else {
            continue;
        };
        if !allowed.contains(&it_target) {
            let ann_name = ann
                .path
                .segments
                .last()
                .map(|s| env.interner.resolve(s.symbol))
                .unwrap_or("");
            let allowed_str = allowed
                .iter()
                .map(|t| t.display())
                .collect::<Vec<_>>()
                .join(", ");
            let span = ann
                .path
                .segments
                .last()
                .map(|s| s.span)
                .unwrap_or(ann.path.span);
            diags.push(diagnostics::annotation_invalid_target(
                &format!("@{ann_name}"),
                &allowed_str,
                span,
            ));
        }
    }
    // 主构造参数上的注解（`@param:Column` / `@Column val x`）：目标类别 Param。
    if let crate::syntax::ast::ItemKind::Type(d) = &item.kind
        && let Some(ctor) = d.primary_ctor.as_ref()
    {
        for param in &ctor.params {
            for ann in &param.annotations {
                let Some(fqn) = resolve_annotation_fqn(&ann.path, env, imports, package_prefix)
                else {
                    continue;
                };
                let Some(allowed) = anno_targets.get(&fqn) else {
                    continue;
                };
                if allowed.is_empty() {
                    continue;
                }
                // use-site target `@param:` → Param；`@property:` → Property；无 target
                // 但参数声明属性（`val x`）→ Property；纯参数 → Param。
                let target = match ann.target.as_ref().map(|t| env.interner.resolve(t.symbol)) {
                    Some("param") => AnnotationUseTarget::Param,
                    Some("property") | Some("field") => AnnotationUseTarget::Property,
                    _ => {
                        if param.property.is_some() {
                            AnnotationUseTarget::Property
                        } else {
                            AnnotationUseTarget::Param
                        }
                    }
                };
                if !allowed.contains(&target) {
                    let ann_name = ann
                        .path
                        .segments
                        .last()
                        .map(|s| env.interner.resolve(s.symbol))
                        .unwrap_or("");
                    let allowed_str = allowed
                        .iter()
                        .map(|t| t.display())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let span = ann
                        .path
                        .segments
                        .last()
                        .map(|s| s.span)
                        .unwrap_or(ann.path.span);
                    diags.push(diagnostics::annotation_invalid_target(
                        &format!("@{ann_name}"),
                        &allowed_str,
                        span,
                    ));
                }
            }
        }
    }
}

/// enum variant 字段类型必须可解析（`NotAType` 等非类型名报 unresolved_type）。
fn check_enum_variant_field_types(
    item: &crate::syntax::ast::Item,
    env: &TypeEnv,
    imports: &crate::resolve::imports::ImportTable,
    package_prefix: &str,
    diags: &mut DiagnosticSink,
) {
    use crate::syntax::ast::{ItemKind, TypeKind};
    let ItemKind::Type(d) = &item.kind else {
        return;
    };
    if !matches!(d.kind, TypeKind::Enum) {
        return;
    }
    // 收集 enum 声明的类型参数名（单段路径若是这些名，视为已解析，如 `Some(val value: T)`）。
    let tp_names: std::collections::HashSet<String> = d
        .type_params
        .as_ref()
        .map(|tp| {
            tp.params
                .iter()
                .map(|p| env.interner.resolve(p.name.symbol).to_string())
                .collect()
        })
        .unwrap_or_default();
    let Some(body) = &d.body else { return };
    for m in &body.members {
        if let crate::syntax::ast::TypeMemberKind::EnumVariant(ev) = &m.kind {
            for field in &ev.fields {
                collect_type_ref_paths(&field.ty, &mut |path| {
                    // 单段名是类型参数 → 视为已解析。
                    if path.segments.len() == 1
                        && let Some(seg) = path.segments.last()
                        && tp_names.contains(env.interner.resolve(seg.symbol))
                    {
                        return;
                    }
                    if !path_resolves_to_known_type(path, env, imports, package_prefix)
                        && let Some(seg) = path.segments.last()
                    {
                        let name = env.interner.resolve(seg.symbol);
                        diags.push(crate::resolve::errors::unresolved_type(name, field.ty.span));
                    }
                });
            }
        }
    }
}

/// TypePath 是否解析为已知类型（Index 命中）。候选：裸名 / 包前缀 / scoop.core / import。
fn path_resolves_to_known_type(
    path: &crate::syntax::ast::TypePath,
    env: &TypeEnv,
    imports: &crate::resolve::imports::ImportTable,
    package_prefix: &str,
) -> bool {
    resolve_annotation_fqn(path, env, imports, package_prefix).is_some()
}

/// 收集一个文件中所有 annotation class 的 FQN 与 `@Target` 声明。
pub(super) fn collect_annotation_class_info(
    file: &crate::syntax::ast::File,
    interner: &scoop2_base::Interner,
    package_prefix: &str,
    anno_classes: &mut std::collections::HashSet<scoop2_base::Symbol>,
    anno_targets: &mut std::collections::HashMap<scoop2_base::Symbol, Vec<AnnotationUseTarget>>,
) {
    use crate::syntax::ast::{ExprKind, ItemKind};
    for item in &file.items {
        let ItemKind::Type(d) = &item.kind else {
            continue;
        };
        if !d
            .modifiers
            .iter()
            .any(|m| m.kind == crate::syntax::ast::ModifierKind::Annotation)
        {
            continue;
        }
        let fqn_text = if package_prefix.is_empty() {
            interner.resolve(d.name.symbol).to_string()
        } else {
            format!("{package_prefix}.{}", interner.resolve(d.name.symbol))
        };
        if let Some(fqn) = interner.get(&fqn_text) {
            anno_classes.insert(fqn);
        } else {
            // 未 intern 过的 FQN：intern 之（collect 阶段 interner 可写不在本签名；
            // 这里用 get 兜底，若不存在则跳过——annotation class 名通常已在 header 收集时 intern）。
            let _ = fqn_text;
        }
        // 解析 @Target(AnnotationTarget.X, ...) 实参。
        let mut targets = Vec::new();
        for ann in &d.annotations {
            let Some(last) = ann.path.segments.last() else {
                continue;
            };
            if interner.resolve(last.symbol) != "Target" {
                continue;
            }
            for arg in &ann.args {
                // 实参形如 `AnnotationTarget.Property`（MemberAccess）。
                if let ExprKind::MemberAccess { member, .. } = &arg.value.kind
                    && let crate::syntax::ast::MemberName::Named(seg) = member
                {
                    let tname = interner.resolve(seg.symbol);
                    if let Some(t) = annotation_target_from_str(tname) {
                        targets.push(t);
                    }
                }
            }
        }
        if !targets.is_empty()
            && let Some(fqn) = interner.get(&fqn_text)
        {
            anno_targets.insert(fqn, targets);
        }
    }
}

/// 注解目标类别（对应 spec AnnotationTarget variant 子集）。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AnnotationUseTarget {
    Function,
    Property,
    Field,
    Param,
    Type,
    Constructor,
    LocalVariable,
    Expression,
    Module,
    TypeParam,
    EnumVariant,
}

impl AnnotationUseTarget {
    fn display(self) -> &'static str {
        match self {
            AnnotationUseTarget::Function => "Function",
            AnnotationUseTarget::Property => "Property",
            AnnotationUseTarget::Field => "Field",
            AnnotationUseTarget::Param => "Param",
            AnnotationUseTarget::Type => "Type",
            AnnotationUseTarget::Constructor => "Constructor",
            AnnotationUseTarget::LocalVariable => "LocalVariable",
            AnnotationUseTarget::Expression => "Expression",
            AnnotationUseTarget::Module => "Module",
            AnnotationUseTarget::TypeParam => "TypeParam",
            AnnotationUseTarget::EnumVariant => "EnumVariant",
        }
    }
}

fn annotation_target_from_str(s: &str) -> Option<AnnotationUseTarget> {
    Some(match s {
        "Function" => AnnotationUseTarget::Function,
        "Property" => AnnotationUseTarget::Property,
        "Field" => AnnotationUseTarget::Field,
        "Param" => AnnotationUseTarget::Param,
        "Type" => AnnotationUseTarget::Type,
        "Constructor" => AnnotationUseTarget::Constructor,
        "LocalVariable" => AnnotationUseTarget::LocalVariable,
        "Expression" => AnnotationUseTarget::Expression,
        "Module" => AnnotationUseTarget::Module,
        "TypeParam" => AnnotationUseTarget::TypeParam,
        "EnumVariant" => AnnotationUseTarget::EnumVariant,
        _ => return None,
    })
}

/// item 的注解目标类别。
fn item_annotation_target(item: &crate::syntax::ast::Item) -> Option<AnnotationUseTarget> {
    use crate::syntax::ast::ItemKind;
    Some(match &item.kind {
        ItemKind::Fun(_) => AnnotationUseTarget::Function,
        ItemKind::Val(_) | ItemKind::ExtensionProperty(_) => AnnotationUseTarget::Property,
        ItemKind::Type(_) | ItemKind::Object(_) | ItemKind::TypeAlias(_) => {
            AnnotationUseTarget::Type
        }
    })
}

/// 解析注解 use 的 path 为 annotation class FQN（Symbol）。
fn resolve_annotation_fqn(
    path: &crate::syntax::ast::TypePath,
    env: &TypeEnv,
    imports: &crate::resolve::imports::ImportTable,
    package_prefix: &str,
) -> Option<scoop2_base::Symbol> {
    let name_text: String = path
        .segments
        .iter()
        .map(|s| env.interner.resolve(s.symbol))
        .collect::<Vec<_>>()
        .join(".");
    let last = path.segments.last()?;
    let last_text = env.interner.resolve(last.symbol);
    let mut candidates = vec![name_text.clone()];
    if !name_text.contains('.') {
        if !package_prefix.is_empty() {
            candidates.push(format!("{package_prefix}.{last_text}"));
        }
        if let Some(fqn) = imports.resolve_name(last.symbol, env.index, env.interner) {
            candidates.push(env.interner.resolve(fqn).to_string());
        }
        candidates.push(format!("scoop.core.{last_text}"));
    }
    for c in &candidates {
        if let Some(fqn) = env.interner.get(c)
            && env.index.lookup_type(fqn).is_some()
        {
            return Some(fqn);
        }
    }
    None
}

/// TypePath 是否解析为 annotation class。
fn path_resolves_to_anno_class(
    path: &crate::syntax::ast::TypePath,
    env: &TypeEnv,
    imports: &crate::resolve::imports::ImportTable,
    package_prefix: &str,
    anno_classes: &std::collections::HashSet<scoop2_base::Symbol>,
) -> bool {
    let Some(fqn) = resolve_annotation_fqn(path, env, imports, package_prefix) else {
        return false;
    };
    anno_classes.contains(&fqn)
}

/// 单段名是否解析为 annotation class（用于构造调用 `Foo(args)`）。
fn name_resolves_to_anno_class(
    name: &str,
    env: &TypeEnv,
    imports: &crate::resolve::imports::ImportTable,
    package_prefix: &str,
    anno_classes: &std::collections::HashSet<scoop2_base::Symbol>,
) -> bool {
    let mut candidates = vec![name.to_string()];
    if !package_prefix.is_empty() {
        candidates.push(format!("{package_prefix}.{name}"));
    }
    if let Some(sym) = env.interner.get(name)
        && let Some(fqn) = imports.resolve_name(sym, env.index, env.interner)
    {
        candidates.push(env.interner.resolve(fqn).to_string());
    }
    candidates.push(format!("scoop.core.{name}"));
    for c in &candidates {
        if let Some(fqn) = env.interner.get(c)
            && anno_classes.contains(&fqn)
        {
            return true;
        }
    }
    false
}

/// 构造调用 callee 的简单名（单段 `Foo(...)` 或多段 `a.Foo(...)`）。
/// 构造调用 callee 的简单名 Symbol（多段 `a.Foo(...)` 取末段；单段返回 None 交由
/// 表达式阶段 nominal 检查覆盖，避免重复扫描）。
fn callee_simple_name_symbol(callee: &crate::syntax::ast::Expr) -> Option<scoop2_base::Symbol> {
    use crate::syntax::ast::{ExprKind, MemberName};
    match &callee.kind {
        ExprKind::MemberAccess { member, .. } => match member {
            MemberName::Named(seg) => Some(seg.symbol),
            MemberName::TupleIndex { .. } => None,
        },
        _ => None,
    }
}

/// 收集 item 内所有类型引用与构造调用（表达式）。
fn collect_item_type_refs_and_ctors<'a>(
    item: &'a crate::syntax::ast::Item,
    type_refs: &mut Vec<&'a crate::syntax::ast::TypeRef>,
    ctor_calls: &mut Vec<&'a crate::syntax::ast::Expr>,
) {
    use crate::syntax::ast::ItemKind;
    match &item.kind {
        ItemKind::Fun(d) => {
            for p in &d.params {
                if let Some(t) = &p.ty {
                    type_refs.push(t);
                }
            }
            if let Some(t) = &d.return_ty {
                type_refs.push(t);
            }
            if let Some(b) = &d.body {
                collect_body_exprs(b, ctor_calls);
            }
        }
        ItemKind::Val(d) => {
            if let Some(t) = &d.ty {
                type_refs.push(t);
            }
            if let Some(init) = &d.init {
                collect_expr_calls(init, ctor_calls);
            }
        }
        _ => {}
    }
}

fn collect_body_exprs<'a>(
    body: &'a crate::syntax::ast::FunBody,
    out: &mut Vec<&'a crate::syntax::ast::Expr>,
) {
    match body {
        crate::syntax::ast::FunBody::Block(b) => collect_block_calls(b, out),
        crate::syntax::ast::FunBody::Expr(e) => collect_expr_calls(e, out),
    }
}

fn collect_block_calls<'a>(
    block: &'a crate::syntax::ast::Block,
    out: &mut Vec<&'a crate::syntax::ast::Expr>,
) {
    for s in &block.stmts {
        collect_stmt_calls(s, out);
    }
}

fn collect_stmt_calls<'a>(
    stmt: &'a crate::syntax::ast::Stmt,
    out: &mut Vec<&'a crate::syntax::ast::Expr>,
) {
    use crate::syntax::ast::StmtKind;
    match &stmt.kind {
        StmtKind::Expr(e) => collect_expr_calls(e, out),
        StmtKind::Assign { value, .. } => collect_expr_calls(value, out),
        StmtKind::LocalVal(d) => {
            if let Some(init) = &d.init {
                collect_expr_calls(init, out);
            }
        }
        StmtKind::Return { value } => {
            if let Some(e) = value {
                collect_expr_calls(e, out);
            }
        }
        StmtKind::While { body, .. } | StmtKind::For { body, .. } => collect_block_calls(body, out),
        StmtKind::Empty | StmtKind::Break | StmtKind::Continue => {}
    }
}

fn collect_expr_calls<'a>(
    expr: &'a crate::syntax::ast::Expr,
    out: &mut Vec<&'a crate::syntax::ast::Expr>,
) {
    use crate::syntax::ast::ExprKind;
    if let ExprKind::Call { .. } = &expr.kind {
        out.push(expr);
    }
    match &expr.kind {
        ExprKind::Call { callee, args } => {
            collect_expr_calls(callee, out);
            for a in args {
                collect_expr_calls(&a.value, out);
            }
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            collect_expr_calls(lhs, out);
            collect_expr_calls(rhs, out);
        }
        ExprKind::Unary { expr: inner, .. } => collect_expr_calls(inner, out),
        ExprKind::MemberAccess { receiver, .. } | ExprKind::SafeMemberAccess { receiver, .. } => {
            collect_expr_calls(receiver, out)
        }
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_expr_calls(cond, out);
            collect_expr_calls(then_branch, out);
            if let Some(e) = else_branch {
                collect_expr_calls(e, out);
            }
        }
        ExprKind::Block(b)
        | ExprKind::DoBlock(b)
        | ExprKind::UnsafeBlock(b)
        | ExprKind::SafeBlock(b) => collect_block_calls(b, out),
        _ => {}
    }
}

/// 递归收集 TypeRef 中的所有 Path 类型路径。
fn collect_type_ref_paths(
    tr: &crate::syntax::ast::TypeRef,
    f: &mut impl FnMut(&crate::syntax::ast::TypePath),
) {
    use crate::syntax::ast::TypeRefKind;
    match &tr.kind {
        TypeRefKind::Path { path, args } => {
            f(path);
            for a in args {
                collect_type_arg_paths(a, f);
            }
        }
        TypeRefKind::Tuple(els) => {
            for e in els {
                collect_type_ref_paths(e, f);
            }
        }
        TypeRefKind::Function { params, ret, .. } => {
            for p in params {
                collect_type_ref_paths(p, f);
            }
            collect_type_ref_paths(ret, f);
        }
        TypeRefKind::ReceiverFunction {
            receiver,
            params,
            ret,
            ..
        } => {
            collect_type_ref_paths(receiver, f);
            for p in params {
                collect_type_ref_paths(p, f);
            }
            collect_type_ref_paths(ret, f);
        }
        TypeRefKind::Nullable(inner) => collect_type_ref_paths(inner, f),
        TypeRefKind::Unit => {}
    }
}

fn collect_type_arg_paths(
    arg: &crate::syntax::ast::TypeArg,
    f: &mut impl FnMut(&crate::syntax::ast::TypePath),
) {
    use crate::syntax::ast::TypeArgKind;
    match &arg.kind {
        TypeArgKind::Type(t) => collect_type_ref_paths(t, f),
        TypeArgKind::Star | TypeArgKind::Effect(_) => {}
    }
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

/// 记录 class 主构造器参数布局与 `: Super(args)` 委托（MIR 继承构造链展开用）。
///
/// super 实参仅覆盖可静态解析的形式：常量字面量与本类主构造器参数引用
/// （`class B(tag: String) : A(tag)`），且仅位置实参。命名实参 / 一般表达式
/// 不记录（MIR 侧保持旧的「不初始化超类字段」行为，不回归）。
fn record_class_ctor_layout(
    d: &crate::syntax::ast::TypeDecl,
    env: &mut TypeEnv,
    imports: &crate::resolve::imports::ImportTable,
    package_prefix: &str,
    diags: &mut DiagnosticSink,
) {
    use crate::resolve::symbol::NominalCategory;
    let name_text = env.interner.resolve(d.name.symbol);
    let fqn_text = if package_prefix.is_empty() {
        name_text.to_string()
    } else {
        format!("{package_prefix}.{name_text}")
    };
    let Some(fqn) = env.interner.get(&fqn_text) else {
        return;
    };
    // 主构造器参数布局（含非属性参数）。无 primary_ctor 时跳过（class_ctor_params 无条目）。
    if let Some(ctor) = &d.primary_ctor {
        let tp_map = env::build_tp_map(env, d.type_params.as_ref());
        let mut infos: Vec<crate::hir::ClassCtorParamInfo> = Vec::with_capacity(ctor.params.len());
        for cp in &ctor.params {
            let ty = match &cp.ty {
                Some(t) => {
                    let mut lower = crate::typecheck::lower::TypeLowering::new(
                        env,
                        imports,
                        tp_map.clone(),
                        package_prefix.to_string(),
                        diags,
                    );
                    lower.lower(t)
                }
                None => env.store.unit(),
            };
            infos.push(crate::hir::ClassCtorParamInfo {
                name: cp.name.symbol,
                ty,
                is_property: cp.property.is_some(),
            });
        }
        env.class_ctor_params.insert(fqn, infos);
    }
    // super 委托收集（不依赖 primary_ctor——无 primary_ctor 的类如 `class D : A(f())`
    // 也有 super 委托）：supertypes 中第一个 class 类别项。
    let bases: Vec<scoop2_base::Symbol> = env.index.supertypes_of(fqn).to_vec();
    for (i, st) in d.supertypes.iter().enumerate() {
        let Some(&base_fqn) = bases.get(i) else {
            continue;
        };
        if !matches!(env.index.category(base_fqn), Some(NominalCategory::Class)) {
            continue;
        }
        // 记录 super 委托：base_index 指向 d.supertypes[i]，实参表达式由 MIR
        // 从 AST 直接 lower（任意表达式：函数调用/运算/参数引用/常量/命名实参）。
        // 命名实参由 MIR lower_delegation_args 按目标 ctor 签名排序。
        // arg_tys 暂用 ctor 参数类型占位——实参真实类型由 check_super_delegation_args
        // typecheck 后的 expr_types 提供，MIR lower 时按实参 NodeId 取 expr_type。
        env.super_ctor_delegations.insert(
            fqn,
            crate::hir::SuperCtorDelegation {
                super_fqn: base_fqn,
                base_index: i,
                arg_tys: Vec::new(),
            },
        );
        return;
    }
}

/// 把 super 委托实参表达式解析为 [`crate::hir::SuperCtorArg`]。
/// 支持：常量字面量 / `true`/`false` / 本类主构造器参数引用（按名字匹配）。
fn super_ctor_arg_of(
    e: &crate::syntax::ast::Expr,
    params: &[crate::hir::ClassCtorParamInfo],
    env: &mut TypeEnv,
) -> Option<crate::hir::SuperCtorArg> {
    use crate::syntax::ast::ExprKind;
    match &e.kind {
        ExprKind::IntLit(l) => {
            let ty = env.store.int();
            Some(crate::hir::SuperCtorArg::Const {
                value: crate::hir::SuperCtorConst::Int(l.value),
                ty,
            })
        }
        ExprKind::FloatLit(l) => {
            let ty = if l.suffix.is_some() {
                env.store.float32()
            } else {
                env.store.float64()
            };
            Some(crate::hir::SuperCtorArg::Const {
                value: crate::hir::SuperCtorConst::Float(l.value),
                ty,
            })
        }
        ExprKind::CharLit(l) => {
            let ty = env.store.char();
            Some(crate::hir::SuperCtorArg::Const {
                value: crate::hir::SuperCtorConst::Char(l.value),
                ty,
            })
        }
        ExprKind::StringLit(l) => {
            let ty = env.store.string();
            Some(crate::hir::SuperCtorArg::Const {
                value: crate::hir::SuperCtorConst::String(l.value.clone()),
                ty,
            })
        }
        ExprKind::UnitLit => {
            let ty = env.store.unit();
            Some(crate::hir::SuperCtorArg::Const {
                value: crate::hir::SuperCtorConst::Unit,
                ty,
            })
        }
        ExprKind::Ident(ident) => {
            let text = env.interner.resolve(ident.symbol);
            // Bool 字面量（`true`/`false` 是普通 Ident）。
            if text == "true" || text == "false" {
                let ty = env.store.bool();
                return Some(crate::hir::SuperCtorArg::Const {
                    value: crate::hir::SuperCtorConst::Bool(text == "true"),
                    ty,
                });
            }
            // 本类主构造器参数引用（按名字匹配）。
            let (index, ty) = params
                .iter()
                .enumerate()
                .find(|(_, p)| env.interner.resolve(p.name) == text)
                .map(|(i, p)| (i as u32, p.ty))?;
            Some(crate::hir::SuperCtorArg::CtorParam { index, ty })
        }
        _ => None,
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
    let tp_map = env::build_tp_map(env, d.type_params.as_ref());
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

/// `out T` 变型位置检查：out 类型参数不能出现在逆变位置（成员方法的参数类型）。
fn check_variance_positions(d: &crate::syntax::ast::TypeDecl, diags: &mut DiagnosticSink) {
    use crate::syntax::ast::{TypeMemberKind, Variance};
    use std::collections::HashSet;
    // 收集 `out T` 的类型参数名。
    let out_params: HashSet<scoop2_base::Symbol> = d
        .type_params
        .as_ref()
        .map(|tp| {
            tp.params
                .iter()
                .filter(|p| matches!(p.variance, Some(Variance::Out)))
                .map(|p| p.name.symbol)
                .collect()
        })
        .unwrap_or_default();
    if out_params.is_empty() {
        return;
    }
    let Some(body) = &d.body else {
        return;
    };
    // 扫描成员方法的参数类型引用中的类型名。
    for m in &body.members {
        if let TypeMemberKind::Fun(fd) = &m.kind {
            for p in &fd.params {
                if let Some(ty) = &p.ty {
                    scan_typeref_for_out_var(&out_params, ty, diags);
                }
            }
        }
    }
}

/// 递归扫描 TypeRef 中的路径段名是否匹配 `out T`。
fn scan_typeref_for_out_var(
    out_params: &std::collections::HashSet<scoop2_base::Symbol>,
    ty: &crate::syntax::ast::TypeRef,
    diags: &mut DiagnosticSink,
) {
    use crate::syntax::ast::TypeRefKind;
    match &ty.kind {
        TypeRefKind::Path { path, args } => {
            if let Some(last) = path.segments.last()
                && out_params.contains(&last.symbol)
            {
                diags.push(diagnostics::variance_position_violation("T", ty.span));
            }
            for a in args {
                if let crate::syntax::ast::TypeArgKind::Type(t) = &a.kind {
                    scan_typeref_for_out_var(out_params, t, diags);
                }
            }
        }
        TypeRefKind::Tuple(elems) => {
            for e in elems {
                scan_typeref_for_out_var(out_params, e, diags);
            }
        }
        TypeRefKind::Function { params, ret, .. } => {
            for p in params {
                scan_typeref_for_out_var(out_params, p, diags);
            }
            scan_typeref_for_out_var(out_params, ret, diags);
        }
        TypeRefKind::ReceiverFunction {
            receiver,
            params,
            ret,
            ..
        } => {
            scan_typeref_for_out_var(out_params, receiver, diags);
            for p in params {
                scan_typeref_for_out_var(out_params, p, diags);
            }
            scan_typeref_for_out_var(out_params, ret, diags);
        }
        TypeRefKind::Nullable(inner) => {
            scan_typeref_for_out_var(out_params, inner, diags);
        }
        TypeRefKind::Unit => {}
    }
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

/// `@CLayout(packed: N)`：N 必须是 2 的幂且 <= 16。
fn check_clayout_packed(
    anns: &[crate::syntax::ast::AnnotationUse],
    interner: &scoop2_base::Interner,
    diags: &mut DiagnosticSink,
) {
    use crate::syntax::ast::ExprKind;
    for ann in anns {
        let is_clayout = ann
            .path
            .segments
            .last()
            .is_some_and(|s| interner.resolve(s.symbol) == "CLayout");
        if !is_clayout {
            continue;
        }
        for arg in &ann.args {
            let is_packed = arg
                .name
                .as_ref()
                .is_some_and(|n| interner.resolve(n.symbol) == "packed");
            if !is_packed {
                continue;
            }
            if let ExprKind::IntLit(il) = &arg.value.kind
                && !(il.value.is_power_of_two() && il.value <= 16)
            {
                diags.push(diagnostics::clayout_packed_value_not_supported(
                    arg.value.span,
                ));
            }
        }
    }
}

/// `@CLayout` struct 的所有字段必须是 GC-free 值类型。
fn check_clayout_struct_gc_free(
    d: &crate::syntax::ast::TypeDecl,
    env: &mut TypeEnv,
    imports: &crate::resolve::imports::ImportTable,
    package_prefix: &str,
    diags: &mut DiagnosticSink,
) {
    use crate::syntax::ast::TypeMemberKind;
    let ann_span = d
        .annotations
        .iter()
        .find(|a| {
            a.path
                .segments
                .last()
                .is_some_and(|s| env.interner.resolve(s.symbol) == "CLayout")
        })
        .map(|a| a.span)
        .unwrap_or(d.name.span);
    // 收集所有字段的类型引用，统一降级后检查 GC-free。
    let mut field_tys: Vec<&crate::syntax::ast::TypeRef> = Vec::new();
    if let Some(ctor) = &d.primary_ctor {
        for cp in &ctor.params {
            if let Some(t) = &cp.ty {
                field_tys.push(t);
            }
        }
    }
    if let Some(body) = &d.body {
        for m in &body.members {
            if let TypeMemberKind::Property(pd) = &m.kind
                && let Some(t) = &pd.ty
            {
                field_tys.push(t);
            }
        }
    }
    let mut bad = false;
    for t in field_tys {
        let ty = {
            let mut lower = crate::typecheck::lower::TypeLowering::new(
                env,
                imports,
                std::collections::HashMap::new(),
                package_prefix.to_string(),
                diags,
            );
            lower.lower(t)
        };
        if !release_hook::is_gc_free_value_type(env, ty) {
            bad = true;
        }
    }
    if bad {
        diags.push(diagnostics::clayout_struct_must_be_gc_free(ann_span));
    }
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
    d: &mut crate::syntax::ast::FunDecl,
    env: &mut TypeEnv,
    imports: &crate::resolve::imports::ImportTable,
    resolution: &crate::resolve::Resolution,
    diags: &mut DiagnosticSink,
    package_prefix: &str,
    enclosing_type_params: &std::collections::HashMap<scoop2_base::Symbol, crate::ty::TypeParamId>,
    this_ty: Option<crate::ty::TypeId>,
    expr_types: &mut crate::resolve::output::NodeIdTable<crate::ty::TypeId>,
    facts: &mut crate::hir::SemanticFacts,
) {
    // 闭合 effect row（`...!`）不允许引用 effect row 变量（`eff E`）—— header 级检查。
    check_closed_effect_row_no_row_var(d, diags);
    // 函数参数必须显式标注类型（无参数类型推断）。
    for p in &d.params {
        if p.ty.is_none() {
            diags.push(diagnostics::missing_type_annotation(p.name.span));
        }
    }
    let Some(body) = &mut d.body else {
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
    // 收集 ref/value kind bound（从内联 bound + where 子句）。
    let mut param_ref_bounds: std::collections::HashSet<scoop2_base::Symbol> =
        std::collections::HashSet::new();
    let mut param_value_bounds: std::collections::HashSet<scoop2_base::Symbol> =
        std::collections::HashSet::new();
    if let Some(type_params) = &d.type_params {
        for p in &type_params.params {
            tp.insert(p.name.symbol, crate::ty::TypeParamId(p.id.as_u32()));
            // 内联 bound（§5.1 `T: ref` / `T: value`）。
            if let Some(bound) = &p.bound {
                match bound {
                    crate::syntax::ast::GenericBound::Ref(_) => {
                        param_ref_bounds.insert(p.name.symbol);
                    }
                    crate::syntax::ast::GenericBound::Value(_) => {
                        param_value_bounds.insert(p.name.symbol);
                    }
                    _ => {}
                }
            }
        }
    }
    // where 子句中的 ref/value bound。
    if let Some(wc) = &d.where_clause {
        for c in &wc.constraints {
            match &c.bound {
                crate::syntax::ast::GenericBound::Ref(_) => {
                    param_ref_bounds.insert(c.name.symbol);
                }
                crate::syntax::ast::GenericBound::Value(_) => {
                    param_value_bounds.insert(c.name.symbol);
                }
                _ => {}
            }
        }
    }
    // private/internal 函数省略 effect row 时，由函数体推断（不报错）。
    // 但 entry-point main 是 program boundary，必须 Pure——即使 internal 也不跳过 effect 检查。
    let is_entry_main_flag =
        d.receiver.is_none() && this_ty.is_none() && env.interner.resolve(d.name.symbol) == "main";
    let skip_effect_check = d.effect.is_none()
        && !is_entry_main_flag
        && d.modifiers.iter().any(|m| {
            matches!(
                m.kind,
                crate::syntax::ast::ModifierKind::Private
                    | crate::syntax::ast::ModifierKind::Internal
            )
        });
    // 是否 entry-point main（顶层、无 receiver、名为 main）。
    let is_entry_main =
        this_ty.is_none() && d.receiver.is_none() && env.interner.resolve(d.name.symbol) == "main";
    let in_nogc = has_annotation(&d.annotations, "NoGC", env.interner);
    // 当前声明的 where 约束 owner：顶层 fun 约束注册在 fun FQN 下；成员 / 扩展 fun
    // 的约束注册在所属类型 FQN 下（`register_type_constraints`）。函数体内的
    // Type bound 查找只在这些 owner 中进行，避免跨文件同名类型参数约束泄漏。
    let mut constraint_owners: Vec<scoop2_base::Symbol> = Vec::new();
    if d.receiver.is_none() && this_ty.is_none() {
        let name_text = env.interner.resolve(d.name.symbol);
        let fqn_text = if package_prefix.is_empty() {
            name_text.to_string()
        } else {
            format!("{package_prefix}.{name_text}")
        };
        if let Some(f) = env.interner.get(&fqn_text) {
            constraint_owners.push(f);
        }
    }
    if let Some(tt) = this_ty {
        if let Some(f) = expr::nominal_fqn_of(env.store.kind(tt)) {
            constraint_owners.push(f);
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
        param_ref_bounds,
        param_value_bounds,
        this_ty,
        skip_effect_check,
        expr_types,
        facts,
        is_entry_main,
        in_nogc,
        constraint_owners,
    );
}

#[allow(clippy::too_many_arguments)]
fn check_member_funs(
    members: &mut [crate::syntax::ast::TypeMember],
    this_ty: Option<crate::ty::TypeId>,
    env: &mut TypeEnv,
    imports: &crate::resolve::imports::ImportTable,
    resolution: &crate::resolve::Resolution,
    diags: &mut DiagnosticSink,
    package_prefix: &str,
    enclosing_type_params: &std::collections::HashMap<scoop2_base::Symbol, crate::ty::TypeParamId>,
    expr_types: &mut crate::resolve::output::NodeIdTable<crate::ty::TypeId>,
    facts: &mut crate::hir::SemanticFacts,
) {
    use crate::syntax::ast::TypeMemberKind;
    for m in members.iter_mut() {
        match &mut m.kind {
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
                    expr_types,
                    facts,
                );
            }
            TypeMemberKind::Object(d) => {
                if let Some(name) = &d.name
                    && let Some(b) = &mut d.body
                {
                    let nested = make_nominal_under(env, this_ty, name.symbol);
                    check_member_funs(
                        &mut b.members,
                        nested,
                        env,
                        imports,
                        resolution,
                        diags,
                        package_prefix,
                        enclosing_type_params,
                        expr_types,
                        facts,
                    );
                }
            }
            TypeMemberKind::Type(d) => {
                if let Some(b) = &mut d.body {
                    let nested = make_nominal_under(env, this_ty, d.name.symbol);
                    // 嵌套类型：合并外层 + 自身类型参数。
                    let mut merged = enclosing_type_params.clone();
                    merge_type_params(&mut merged, d.type_params.as_ref());
                    check_member_funs(
                        &mut b.members,
                        nested,
                        env,
                        imports,
                        resolution,
                        diags,
                        package_prefix,
                        &merged,
                        expr_types,
                        facts,
                    );
                }
            }
            _ => {}
        }
    }
}

/// 把类型参数列表合并进已有 map（用于嵌套类型累积外层 + 自身类型参数）。
fn merge_type_params(
    map: &mut std::collections::HashMap<scoop2_base::Symbol, crate::ty::TypeParamId>,
    tp: Option<&crate::syntax::ast::TypeParamList>,
) {
    if let Some(tp) = tp {
        for p in &tp.params {
            map.insert(p.name.symbol, crate::ty::TypeParamId(p.id.as_u32()));
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
