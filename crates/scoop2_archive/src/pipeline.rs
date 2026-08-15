//! 源码 → 解析 → typecheck 的可复用管线胶水（自 `scoop2c` main.rs 抽取）。
//!
//! CLI 子命令（dump-* / build / run / hir-build / mir-build）与 oracle 测试共用
//! 同一份胶水，保证「一次成型管线」与「分阶段落地管线」走完全相同的前端路径。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// 定位 sysroot 目录：优先环境变量 `SCOOP_SYSROOT`；否则沿可执行文件祖先向上
/// 找包含 `sysroot/` 子目录的节点（兼容 `target/debug/scoop2c` 与
/// `target/debug/deps/<test-bin>` 两种布局）。找不到返回 `None`（前端仍可对
/// 不依赖内置类型的程序解析）。
pub fn locate_sysroot() -> Option<PathBuf> {
    if let Ok(s) = std::env::var("SCOOP_SYSROOT") {
        let p = PathBuf::from(s);
        if p.is_dir() {
            return Some(p);
        }
    }
    let exe = std::env::current_exe().ok()?;
    for ancestor in exe.ancestors().skip(1) {
        let candidate = ancestor.join("sysroot");
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    None
}

/// 读取通过环境变量 `SCOOP_SYSROOT_DEPS` 声明的显式依赖（逗号分隔的包名）。
///
/// 这些是用户显式声明的 sysroot 依赖（如 `scoop.thread`），允许通过 wildcard
/// 导入。未声明的非 auto-dependency 包（如 `scoop.sync`）不能隐式导入。
pub fn read_declared_deps() -> HashSet<String> {
    let mut deps = HashSet::new();
    if let Ok(s) = std::env::var("SCOOP_SYSROOT_DEPS") {
        for d in s.split(',') {
            let d = d.trim();
            if !d.is_empty() {
                deps.insert(d.to_string());
            }
        }
    }
    deps
}

/// 收集 fixture 的 `.sysroot` overlay 目录（`SCOOP_SYSROOT_OVERLAY`）中的
/// `.scoop` 文件（按路径排序，保证确定性）。
pub fn collect_overlay_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(overlay) = std::env::var("SCOOP_SYSROOT_OVERLAY") {
        let p = PathBuf::from(&overlay);
        if p.is_dir() {
            walk_inner(&p, &mut out);
            out.sort();
        }
    }
    out
}

/// 递归收集 `dir` 下的所有 `*.scoop` 文件路径（按路径排序，保证确定性）。
pub fn walk_scoop_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk_inner(dir, &mut out);
    out.sort();
    out
}

fn walk_inner(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_inner(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "scoop") {
            out.push(path);
        }
    }
}

/// 解析主文件 + sysroot + `.sysroot` overlay 后的程序集合。
pub struct BuiltProgram {
    pub parsed: Vec<scoop2_syntax::parser::ParsedFile>,
    /// 所有文件的 SourceFile（与 parsed 同序），供诊断渲染跨文件 label。
    pub sources: Vec<scoop2_base::SourceFile>,
    /// 用户文件在 `parsed` 中的下标（主文件 0 始终是 user；overlay 文件也算 user）。
    pub user_indices: Vec<usize>,
    pub interner: scoop2_base::Interner,
    pub diags: scoop2_base::diag::DiagnosticSink,
}

/// 解析主文件 + sysroot + `.sysroot` overlay。
///
/// 主文件（index 0）始终是 user；sysroot 文件按路径序追加；overlay 文件最后
/// 追加并计入 user（当作用户代码检查）。
pub fn build_program(source: &scoop2_base::SourceFile) -> BuiltProgram {
    let mut interner = scoop2_base::Interner::new();
    let diags = scoop2_base::diag::DiagnosticSink::new();
    let mut parsed: Vec<scoop2_syntax::parser::ParsedFile> = Vec::with_capacity(1 + 32);
    let mut sources: Vec<scoop2_base::SourceFile> = Vec::with_capacity(1 + 32);
    let mut user_indices: Vec<usize> = vec![0];
    parsed.push(scoop2_syntax::parser::parse_file_with(
        source,
        &mut interner,
    ));
    sources.push(source.clone());
    if let Some(sysroot) = locate_sysroot() {
        for path in walk_scoop_files(&sysroot.join("lib")) {
            if let Ok(src) = scoop2_base::SourceFile::load_sysroot(&path) {
                parsed.push(scoop2_syntax::parser::parse_file_with(&src, &mut interner));
                sources.push(src);
            }
        }
    }
    // Fixture sysroot overlay（`.sysroot` 目录中的 `.scoop`）→ 当作用户代码检查。
    for path in collect_overlay_files() {
        if let Ok(src) = scoop2_base::SourceFile::load_sysroot(&path) {
            user_indices.push(parsed.len());
            parsed.push(scoop2_syntax::parser::parse_file_with(&src, &mut interner));
            sources.push(src);
        }
    }
    BuiltProgram {
        parsed,
        sources,
        user_indices,
        interner,
        diags,
    }
}

/// 把解析好的文件集合构造为 resolve/typecheck 的 `InputFile` 列表。
pub fn make_inputs<'a>(
    parsed: &'a mut [scoop2_syntax::parser::ParsedFile],
    user_indices: &[usize],
) -> Vec<scoop2_hir::resolve::InputFile<'a>> {
    parsed
        .iter_mut()
        .enumerate()
        .map(|(i, pf)| scoop2_hir::resolve::InputFile {
            file: &mut pf.file,
            file_id: scoop2_base::FileId(i as u32),
            origin: if user_indices.contains(&i) {
                scoop2_hir::resolve::InputOrigin::User
            } else {
                scoop2_hir::resolve::InputOrigin::Sysroot
            },
            // 主文件（i==0）非受信任；sysroot + `.sysroot` overlay 文件受信任。
            trusted: i != 0,
        })
        .collect()
}

/// 汇总解析诊断并运行完整 typecheck；返回 typed HIR。
///
/// 有解析 / typecheck 错误时返回 `Err(diags)`（全有或全无——错误程序不产出
/// 可消费的 HIR，PLAN.md C5）。
pub fn typecheck_program(
    program: &mut BuiltProgram,
    target_platform: Option<&str>,
) -> Result<scoop2_hir::hir::TypedHir, scoop2_base::diag::DiagnosticSink> {
    for pf in &program.parsed {
        program.diags.extend(pf.diagnostics.iter().cloned());
    }
    if program.parsed[0].diagnostics.has_errors() {
        return Err(std::mem::take(&mut program.diags));
    }
    let declared_deps: Vec<String> = read_declared_deps().into_iter().collect();
    let mut inputs = make_inputs(&mut program.parsed, &program.user_indices);
    let mut hir = scoop2_hir::typecheck::run_typecheck(
        &mut inputs,
        &mut program.interner,
        &mut program.diags,
        target_platform,
        &declared_deps,
    );
    if program.diags.has_errors() {
        return Err(std::mem::take(&mut program.diags));
    }
    build_trees(&mut hir, program);
    Ok(hir)
}

/// M2 第一刀：为每个用户文件的顶层函数构造 HIR body 树。
///
/// gaps 记录未覆盖构造（不阻塞管线——树尚无消费者；MIR 翻转前必须清零）。
/// 签名缺失（重载匹配失败等）的函数暂不构树。
fn build_trees(hir: &mut scoop2_hir::hir::TypedHir, program: &BuiltProgram) {
    use scoop2_hir::hir::element::TypeCategory;
    use scoop2_hir::hir::tree;
    use scoop2_syntax::ast::ItemKind;

    let unit_ty = hir.store.unit();
    let mut new_files: Vec<(usize, Vec<tree::FnTree>, Vec<tree::FileItem>)> = Vec::new();
    for (i, tf) in hir.files.iter().enumerate() {
        let Some(pf) = program.parsed.get(tf.file_id.0 as usize) else {
            continue;
        };
        let mut trees = Vec::new();
        let mut skeleton = Vec::new();
        let fqn_of = |simple: scoop2_base::Symbol| -> String {
            let name = program.interner.resolve(simple);
            if tf.package_prefix.is_empty() {
                name.to_string()
            } else {
                format!("{}.{}", tf.package_prefix, name)
            }
        };
        for item in &pf.file.items {
            let range_start = trees.len() as u32;
            match &item.kind {
                ItemKind::Fun(d) => {
                    let fqn_text = fqn_of(d.name.symbol);
                    let mut fun_sig: Option<(scoop2_hir::ty::TypeId, scoop2_hir::ty::EffectRow)> =
                        None;
                    if let Some(body) = &d.body
                        && let Some(sig) = lookup_sig(hir, &fqn_text, d.name.span)
                    {
                        fun_sig = Some((sig.return_ty, sig.effect_row.clone()));
                        let params: Vec<(scoop2_base::Symbol, scoop2_hir::ty::TypeId)> = sig
                            .param_names
                            .iter()
                            .copied()
                            .zip(sig.param_types.iter().copied())
                            .collect();
                        trees.push(tree::build_fn_tree(
                            fqn_text.clone(),
                            body,
                            &params,
                            None,
                            unit_ty,
                            &tf.expr_types,
                            &tf.facts,
                            &program.interner,
                            &hir.store,
                        ));
                    } else if let Some(body) = &d.body
                        && let Some(sig) = lookup_extension_sig(hir, d.name.symbol, d.params.len())
                    {
                        // 扩展函数（`fun Receiver.name`）：fqn 仍是简名（AST quirk），
                        // 签名按 owner 全集解析（镜像 lookup_member_sig 的
                        // None-owner 回退——含确定性排序）。
                        fun_sig = Some((sig.return_ty, sig.effect_row.clone()));
                        let params: Vec<(scoop2_base::Symbol, scoop2_hir::ty::TypeId)> = sig
                            .param_names
                            .iter()
                            .copied()
                            .zip(sig.param_types.iter().copied())
                            .collect();
                        trees.push(tree::build_fn_tree(
                            fqn_text.clone(),
                            body,
                            &params,
                            None,
                            unit_ty,
                            &tf.expr_types,
                            &tf.facts,
                            &program.interner,
                            &hir.store,
                        ));
                    }
                    // 无 body / 签名缺失：空树区间（模块 lowering 发签名-only
                    // FunDecl——extern/abstract/intrinsic 形态）。
                    skeleton.push(tree::FileItem {
                        kind: tree::FileItemKind::Fun,
                        fqn: fqn_text,
                        tree_range: (range_start, trees.len() as u32),
                        members: Vec::new(),
                        fun_sig,
                    });
                }
                ItemKind::Val(d) => {
                    let fqn_text = match &d.binding {
                        scoop2_syntax::ast::ValBinding::Name(name) => fqn_of(name.symbol),
                        // 顶层解构是 parse error。
                        scoop2_syntax::ast::ValBinding::Pattern(_) => continue,
                    };
                    // 顶层 val/var 初始化器树（fqn = <prefix>.<name>；根块为尾
                    // 表达式——lower 为 InitializerRoot）。无初始化器（@Extern）：
                    // 空区间 → 模块 lowering 发 ExternGlobal。
                    if let Some(init) = &d.init {
                        trees.push(tree::build_val_init_tree(
                            fqn_text.clone(),
                            init,
                            d.kind == scoop2_syntax::ast::ValKind::Var,
                            unit_ty,
                            &tf.expr_types,
                            &tf.facts,
                            &program.interner,
                            &hir.store,
                        ));
                    }
                    skeleton.push(tree::FileItem {
                        kind: tree::FileItemKind::Val,
                        fqn: fqn_text,
                        tree_range: (range_start, trees.len() as u32),
                        members: Vec::new(),
                        fun_sig: None,
                    });
                }
                ItemKind::Type(d) => {
                    let owner_fqn = fqn_of(d.name.symbol);
                    let category = match d.kind {
                        scoop2_syntax::ast::TypeKind::Class => TypeCategory::Class,
                        scoop2_syntax::ast::TypeKind::Interface => TypeCategory::Interface,
                        scoop2_syntax::ast::TypeKind::Struct => TypeCategory::Struct,
                        scoop2_syntax::ast::TypeKind::Enum => TypeCategory::Enum,
                        scoop2_syntax::ast::TypeKind::Effect => TypeCategory::Effect,
                    };
                    let mut members = Vec::new();
                    if let Some(body) = &d.body {
                        members = build_member_trees(
                            &mut trees,
                            hir,
                            tf,
                            &program.interner,
                            unit_ty,
                            &owner_fqn,
                            &body.members,
                        );
                        // class `<Fqn>.$init` 合成（M2-3：从 MIR 上移）。
                        if let Some(owner_sym) = hir.interner.get(&owner_fqn) {
                            if let Some(init_tree) = tree::synthesize_class_init_tree(
                                hir,
                                tf,
                                &program.interner,
                                unit_ty,
                                &owner_fqn,
                                owner_sym,
                                d,
                            ) {
                                trees.push(init_tree);
                            }
                        }
                    }
                    skeleton.push(tree::FileItem {
                        kind: tree::FileItemKind::Type(category),
                        fqn: owner_fqn,
                        tree_range: (range_start, trees.len() as u32),
                        members,
                        fun_sig: None,
                    });
                }
                ItemKind::Object(d) => {
                    let name_sym = d.name.as_ref().map(|n| n.symbol).unwrap_or_default();
                    let owner_fqn = fqn_of(name_sym);
                    let mut members = Vec::new();
                    if let Some(body) = &d.body {
                        members = build_member_trees(
                            &mut trees,
                            hir,
                            tf,
                            &program.interner,
                            unit_ty,
                            &owner_fqn,
                            &body.members,
                        );
                    }
                    skeleton.push(tree::FileItem {
                        kind: tree::FileItemKind::Object,
                        fqn: owner_fqn,
                        tree_range: (range_start, trees.len() as u32),
                        members,
                        fun_sig: None,
                    });
                }
                // Extension/TypeAlias：MIR 不单独建模（无 item 无树）。
                _ => {}
            }
        }
        new_files.push((i, trees, skeleton));
    }
    for (i, trees, skeleton) in new_files {
        hir.files[i].trees = trees;
        hir.files[i].item_skeleton = skeleton;
    }
}

/// 顶层函数签名查找：FQN symbol + 声明 span 匹配（多重载消歧；单重载兜底）。
fn lookup_sig<'a>(
    hir: &'a scoop2_hir::hir::TypedHir,
    fqn_text: &str,
    decl_span: scoop2_base::Span,
) -> Option<&'a scoop2_hir::hir::TypedSignature> {
    let sym = hir.interner.get(fqn_text)?;
    let sigs = hir.top_level_funs.get(&sym)?;
    sigs.iter()
        .find(|s| s.decl_span == decl_span)
        .or_else(|| (sigs.len() == 1).then(|| &sigs[0]))
}

/// 扩展函数签名查找：owner 全集按名搜索（镜像 MIR `lookup_member_sig` 的
/// None-owner 回退——owner 按文本排序保证确定性，arity 匹配优先）。
fn lookup_extension_sig<'a>(
    hir: &'a scoop2_hir::hir::TypedHir,
    method_sym: scoop2_base::Symbol,
    arity: usize,
) -> Option<&'a scoop2_hir::hir::TypedSignature> {
    let pick = |sigs: &'a [scoop2_hir::hir::TypedSignature]| {
        sigs.iter()
            .find(|s| s.param_types.len() == arity)
            .or_else(|| sigs.first())
    };
    let mut owners: Vec<scoop2_base::Symbol> = hir.member_funs.keys().copied().collect();
    owners.sort_by(|a, b| hir.interner.resolve(*a).cmp(hir.interner.resolve(*b)));
    owners
        .into_iter()
        .filter_map(|o| {
            hir.member_funs
                .get(&o)
                .and_then(|ms| ms.get(&method_sym))
                .and_then(|sigs| pick(sigs))
        })
        .next()
}

/// 类型 / object 体的成员函数树（`<OwnerFqn>.<method>`；`this` 绑定为隐式参数）。
#[allow(clippy::too_many_arguments)]
fn build_member_trees(
    trees: &mut Vec<scoop2_hir::hir::tree::FnTree>,
    hir: &scoop2_hir::hir::TypedHir,
    tf: &scoop2_hir::hir::TypedFile,
    interner: &scoop2_base::Interner,
    unit_ty: scoop2_hir::ty::TypeId,
    owner_fqn: &str,
    members: &[scoop2_syntax::ast::TypeMember],
) -> Vec<scoop2_hir::hir::tree::MemberSlot> {
    use scoop2_syntax::ast::TypeMemberKind;
    let mut slots = Vec::new();
    let owner_sym = hir.interner.get(owner_fqn).unwrap_or_default();
    let this_ty = this_ty_of(hir, owner_sym);
    for m in members {
        match &m.kind {
            TypeMemberKind::Fun(d) => {
                let method_fqn = format!("{}.{}", owner_fqn, interner.resolve(d.name.symbol));
                let Some(body) = &d.body else {
                    // 无 body 方法（接口 / 效应 op / abstract）：签名-only FunDecl。
                    slots.push(scoop2_hir::hir::tree::MemberSlot::Bodyless { fqn: method_fqn });
                    continue;
                };
                let Some(sig) = hir
                    .member_funs
                    .get(&owner_sym)
                    .and_then(|ms| ms.get(&d.name.symbol))
                    .and_then(|sigs| {
                        sigs.iter()
                            .find(|s| s.decl_span == d.name.span)
                            .or_else(|| (sigs.len() == 1).then(|| &sigs[0]))
                    })
                else {
                    continue;
                };
                let params: Vec<(scoop2_base::Symbol, scoop2_hir::ty::TypeId)> = sig
                    .param_names
                    .iter()
                    .copied()
                    .zip(sig.param_types.iter().copied())
                    .collect();
                trees.push(scoop2_hir::hir::tree::build_fn_tree(
                    method_fqn,
                    body,
                    &params,
                    this_ty,
                    unit_ty,
                    &tf.expr_types,
                    &tf.facts,
                    interner,
                    &hir.store,
                ));
                slots.push(scoop2_hir::hir::tree::MemberSlot::Tree);
            }
            TypeMemberKind::Type(d) => {
                if let Some(b) = &d.body {
                    let nested = build_member_trees(
                        trees,
                        hir,
                        tf,
                        interner,
                        unit_ty,
                        &format!("{}.{}", owner_fqn, interner.resolve(d.name.symbol)),
                        &b.members,
                    );
                    slots.extend(nested);
                }
            }
            _ => {}
        }
    }
    slots
}

/// `this` 的类型：owner 的声明态 nominal（ref class/interface / value struct/enum）。
fn this_ty_of(
    hir: &scoop2_hir::hir::TypedHir,
    owner_sym: scoop2_base::Symbol,
) -> Option<scoop2_hir::ty::TypeId> {
    hir.type_infos
        .iter()
        .find(|(ty, _)| {
            matches!(
                hir.store.kind(**ty),
                scoop2_hir::ty::TypeKind::Ref(scoop2_hir::ty::RefTypeKind::Nominal(n))
                    | scoop2_hir::ty::TypeKind::Value(scoop2_hir::ty::ValueTypeKind::Nominal(n))
                    if n.fqn == owner_sym && n.args.is_empty()
            )
        })
        .map(|(&ty, _)| ty)
}
