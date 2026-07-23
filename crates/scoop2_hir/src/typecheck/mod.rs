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
    let mut env = TypeEnv::new(&index, interner);
    // 登记所有用户文件的签名 / 成员 / 构造器。
    for uf in &user_files {
        env::register_top_level_signatures(&mut env, uf.file, &uf.imports, &uf.prefix, diags);
        env::register_members(&mut env, uf.file, &uf.imports, &uf.prefix, diags);
        env::register_constructors(&mut env, uf.file, &uf.imports, &uf.prefix, diags);
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

/// 检查一个文件的**顶层**函数体（含 `= expr` 表达式体）。
fn check_file_bodies(
    file: &crate::syntax::ast::File,
    env: &mut TypeEnv,
    imports: &crate::resolve::imports::ImportTable,
    resolution: &crate::resolve::Resolution,
    diags: &mut DiagnosticSink,
    package_prefix: &str,
) {
    use crate::syntax::ast::ItemKind;
    use std::collections::HashMap;
    for item in &file.items {
        if let ItemKind::Fun(d) = &item.kind
            && let Some(body) = &d.body
        {
            let type_params: HashMap<scoop2_base::Symbol, crate::ty::TypeParamType> =
                HashMap::new(); // 泛型函数实例化推迟
            expr::check_function(
                &d.params,
                d.return_ty.as_ref(),
                body,
                env,
                imports,
                resolution,
                diags,
                package_prefix,
                type_params,
                None,
            );
        }
    }
}
