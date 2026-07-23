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
pub mod lower;

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
) {
    use crate::syntax::ast::{ItemKind, ModifierKind};
    use std::collections::HashMap;
    let empty_tp: HashMap<scoop2_base::Symbol, crate::ty::TypeParamType> = HashMap::new();
    for item in &file.items {
        match &item.kind {
            ItemKind::Fun(d) => {
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
                // entry-point `main` 签名校验（spec P4 §13）。
                let name_text = env.interner.resolve(d.name.symbol);
                if name_text == "main" && d.receiver.is_none() {
                    check_main_signature(d, env, diags);
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
                let is_intrinsic_type = has_annotation(&d.annotations, "Intrinsic", env.interner);
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
                    check_member_funs(
                        &body.members,
                        this_ty,
                        env,
                        imports,
                        resolution,
                        diags,
                        package_prefix,
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
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

/// 检查 entry-point `main` 的签名（spec P4 §13）。
/// 合法形式：`fun main()` 或 `fun main(args: Array<String>)`。
fn check_main_signature(
    d: &crate::syntax::ast::FunDecl,
    env: &mut TypeEnv,
    diags: &mut DiagnosticSink,
) {
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
        let empty_imports = crate::resolve::imports::ImportTable::new();
        let mut lower = crate::typecheck::lower::TypeLowering::new(
            env,
            &empty_imports,
            std::collections::HashMap::new(),
            String::new(),
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
                                _ => format!("{:?}", env.store.kind(*a)),
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
    let Some(body) = &d.body else { return };
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

fn check_member_funs(
    members: &[crate::syntax::ast::TypeMember],
    this_ty: Option<crate::ty::TypeId>,
    env: &mut TypeEnv,
    imports: &crate::resolve::imports::ImportTable,
    resolution: &crate::resolve::Resolution,
    diags: &mut DiagnosticSink,
    package_prefix: &str,
) {
    use crate::syntax::ast::TypeMemberKind;
    use std::collections::HashMap;
    let empty_tp: HashMap<scoop2_base::Symbol, crate::ty::TypeParamType> = HashMap::new();
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
                    &empty_tp,
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
                    );
                }
            }
            TypeMemberKind::Type(d) => {
                if let Some(b) = &d.body {
                    let nested = make_nominal_under(env, this_ty, d.name.symbol);
                    check_member_funs(
                        &b.members,
                        nested,
                        env,
                        imports,
                        resolution,
                        diags,
                        package_prefix,
                    );
                }
            }
            _ => {}
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
