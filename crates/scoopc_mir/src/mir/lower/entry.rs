//! MirLowerError, LoweredMir, lower_for_dump entry, file-level MirLowering.

#![allow(dead_code)]

use miette::Diagnostic;
use thiserror::Error;

use super::*;

/// MIR lowering 错误（当前阶段仅包装 HIR lowering 错误）。
#[derive(Debug, Error, Diagnostic)]
pub enum MirLowerError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Hir(#[from] hir::HirLowerError),
    #[error("direct-style MIR validation failed for `{fqn}`: {error}")]
    InvalidMir {
        fqn: String,
        #[source]
        error: Box<MirValidationError>,
    },
}

/// 一次 lowering 的产物：MIR + 对应的 `TypeStore`。
///
/// 说明：MIR 节点里的 `TypeId` 仅在同一个 `TypeStore` 里可解码/展示。
#[derive(Debug)]
pub struct LoweredMir {
    pub file: File,
    pub types: TypeStore,
}

/// 新建 basic block 时使用的默认 terminator 标记。
///
/// 说明：builder 在 block 完成后应当覆盖该 terminator；若最终仍保留该值，说明 lowering 未覆盖到
/// 某条控制流路径（对 dump/fixtures 来说仍可接受，但在后续阶段应当更严格约束）。
pub(in crate::mir::lower) const UNTERMINATED: &str = "unterminated";

pub(in crate::mir::lower) fn intrinsic_base_fqn(fqn: &str) -> &str {
    let base = fqn.rsplit_once("::<").map(|(base, _)| base).unwrap_or(fqn);
    base.split_once("$overload")
        .map(|(base, _)| base)
        .unwrap_or(base)
}

pub(in crate::mir::lower) fn top_level_callee_fqn(callee: &hir::Expr) -> Option<&str> {
    match &callee.kind {
        hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => Some(fqn.as_str()),
        _ => None,
    }
}

pub(in crate::mir::lower) fn top_level_binding_matches_callee(
    binding_fqn: &str,
    callee: &hir::Expr,
) -> bool {
    top_level_callee_fqn(callee)
        .is_none_or(|callee_fqn| intrinsic_base_fqn(binding_fqn) == intrinsic_base_fqn(callee_fqn))
}

/// 为 `scoop dump-mir` / mir fixtures 生成 MIR（最小实现）。
///
/// 当前阶段 pipeline：
/// 1) parse/resolve 源文件并降到正式 HIR stage output；
/// 2) 从 `HirFacts` 构造 MIR lowering facts，并生成显式 CFG。
pub fn lower_for_dump(session: &Session, source: &SourceFile) -> Result<LoweredMir, MirLowerError> {
    let hir_output = scoopc_hir::stage::run(session, source)?;
    let facts = MirLoweringFacts::from_hir_facts(hir_output.lowered_hir(), hir_output.hir_facts());
    let mut lowered_hir = hir_output.into_lowered_hir();
    let builtins = lowered_hir.types.intern_builtins();

    let file = lower_hir_file_for_dump_with_facts(
        builtins,
        &mut lowered_hir.types,
        &lowered_hir.file,
        &lowered_hir.member_funs,
        &facts,
    );
    Ok(LoweredMir {
        file,
        types: lowered_hir.types,
    })
}

/// 将一份已构造的 HIR 文件降低为 MIR，并显式接入 typed/shared facts。
///
/// 说明：
/// - 调用方需要确保 `hir_file` 中的 `TypeId` 与 `types` 来自同一个 `TypeStore`；
/// - `facts` 负责把 `Continuation.resume`、virtual/interface dispatch 等已确认语义
///   从 HIR/typecheck side table 收口为 MIR lowering 可直接消费的最小输入。
pub fn lower_hir_file_for_dump_with_facts(
    builtins: BuiltinTypes,
    types: &mut TypeStore,
    hir_file: &hir::File,
    member_funs: &[hir::FunDecl],
    facts: &MirLoweringFacts,
) -> File {
    let mut lowering = MirLowering::new(builtins, types, facts);
    lowering.lower_file(hir_file, member_funs)
}

/// 文件级 lowering：负责遍历顶层 item 并为每个函数构造 MIR body。
pub(in crate::mir::lower) struct MirLowering<'a> {
    pub(in crate::mir::lower) builtins: BuiltinTypes,
    pub(in crate::mir::lower) types: &'a mut TypeStore,
    pub(in crate::mir::lower) facts: &'a MirLoweringFacts,
}

impl<'a> MirLowering<'a> {
    /// 创建一个 MIR lowering 上下文（仅保存 builtin type ids）。
    pub(in crate::mir::lower) fn new(
        builtins: BuiltinTypes,
        types: &'a mut TypeStore,
        facts: &'a MirLoweringFacts,
    ) -> Self {
        Self {
            builtins,
            types,
            facts,
        }
    }

    /// 把 HIR 文件降到 MIR 文件。
    pub(in crate::mir::lower) fn lower_file(
        &mut self,
        file: &hir::File,
        member_funs: &[hir::FunDecl],
    ) -> File {
        let top_level_fun_return_tys = collect_top_level_fun_return_tys(file, member_funs);
        let top_level_fun_param_tys = collect_top_level_fun_param_tys(file, member_funs);
        let mut items = Vec::with_capacity(
            file.items.len()
                + member_funs.len()
                + file.decls.len()
                + self.facts.top_level_init_roots().len()
                + self.facts.extern_global_contracts().len(),
        );
        items.extend(
            file.decls
                .iter()
                .map(|decl| Item::Metadata(lower_decl_metadata(decl))),
        );
        items.extend(
            self.facts
                .top_level_init_roots()
                .iter()
                .map(|root| Item::InitializerRoot(self.lower_initializer_root(root))),
        );
        items.extend(
            self.facts
                .extern_global_contracts()
                .iter()
                .map(|contract| Item::ExternGlobal(lower_extern_global_root(contract))),
        );
        for item in &file.items {
            match item {
                hir::Item::Fun(fun) => {
                    let (primary, nested) =
                        self.lower_fun(fun, &top_level_fun_return_tys, &top_level_fun_param_tys);
                    items.push(Item::Fun(primary));
                    items.extend(nested.into_iter().map(Item::Fun));
                }
                hir::Item::Val(_) => {}
                hir::Item::Todo { span, kind } => items.push(Item::Todo {
                    span: *span,
                    kind: kind.clone(),
                }),
            }
        }

        // type/object body 中可 codegen 的 member fun 在 HIR 中以 side table 形式保存；
        // dump-mir / dump-ir 需要把它们也作为真正的 generic MIR root 发射出来。
        for fun in member_funs {
            let (primary, nested) =
                self.lower_fun(fun, &top_level_fun_return_tys, &top_level_fun_param_tys);
            items.push(Item::Fun(primary));
            items.extend(nested.into_iter().map(Item::Fun));
        }

        File { items }
    }

    pub(in crate::mir::lower) fn lower_initializer_root(
        &self,
        root: &TopLevelInitRootContract,
    ) -> InitializerRoot {
        InitializerRoot {
            span: root.span(),
            fqn: root.fqn().to_string(),
            source_path: root.source_path().to_path_buf(),
            kind: lower_initializer_root_kind(root.kind()),
            ty: root.ty(),
            initializer_transport: root.initializer_ty().and_then(|source_ty| {
                root.ty().and_then(|target_ty| {
                    value_erasure_transport(
                        self.builtins,
                        self.types,
                        self.facts,
                        source_ty,
                        target_ty,
                    )
                })
            }),
            has_initializer: root.has_initializer(),
            dependencies: root
                .dependencies()
                .iter()
                .map(lower_initializer_dependency)
                .collect(),
            hidden_effects: self.facts.top_level_ref_hidden_effects(root.fqn()),
        }
    }

    /// 把一个函数降到 MIR。
    pub(in crate::mir::lower) fn lower_fun(
        &mut self,
        fun: &hir::FunDecl,
        top_level_fun_return_tys: &HashMap<String, TypeId>,
        top_level_fun_param_tys: &HashMap<String, Vec<TypeId>>,
    ) -> (FunDecl, Vec<FunDecl>) {
        FnLowering::new(
            self.builtins,
            self.types,
            self.facts,
            top_level_fun_return_tys.clone(),
            top_level_fun_param_tys.clone(),
            fun.fqn.clone(),
            fun.source_path.clone(),
        )
        .lower_fun(fun)
    }
}
