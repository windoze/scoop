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
                check_one_fun(
                    d,
                    env,
                    imports,
                    resolution,
                    diags,
                    package_prefix,
                    &empty_tp,
                    None,
                );
            }
            ItemKind::Type(d) => {
                // annotation class 限制（spec P5 §9）。
                let is_annotation = d
                    .modifiers
                    .iter()
                    .any(|m| m.kind == ModifierKind::Annotation);
                if is_annotation {
                    if d.body.is_some() {
                        diags.push(diagnostics::annotation_class_body_not_supported(
                            d.name.span,
                        ));
                    }
                    if d.type_params.is_some() {
                        diags.push(diagnostics::annotation_class_type_param_not_supported(
                            d.name.span,
                        ));
                    }
                    if d.type_params
                        .as_ref()
                        .is_some_and(|tp| tp.effect_row.is_some())
                    {
                        diags.push(diagnostics::annotation_class_eff_param_not_supported(
                            d.name.span,
                        ));
                    }
                }
                let this_ty = make_nominal(env, package_prefix, d.name.symbol);
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
