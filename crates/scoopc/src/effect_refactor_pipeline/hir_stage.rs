use std::collections::HashMap;
use std::path::Path;

use crate::hir::{
    CallArg, CallSite, Expr, ExprKind, FunDecl, HirLowerError, Item, LoweredHir, Stmt, StmtKind,
    ValueRef,
};
use crate::session::Session;
use crate::source::SourceFile;
use crate::ty::{EffectRow, RefTypeKind, TypeId, TypeKind, TypeStore};

/// 单个 `Continuation.resume(...)` 调用点的 typed contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuationResumeSiteContract {
    receiver_ty: TypeId,
    resume_ty: TypeId,
    answer_ty: TypeId,
    return_ty: TypeId,
    out_effects: EffectRow,
    includes_runtime_error_effect: bool,
}

impl ContinuationResumeSiteContract {
    fn new(
        receiver_ty: TypeId,
        resume_ty: TypeId,
        answer_ty: TypeId,
        return_ty: TypeId,
        out_effects: EffectRow,
    ) -> Self {
        Self {
            receiver_ty,
            resume_ty,
            answer_ty,
            return_ty,
            out_effects,
            includes_runtime_error_effect: true,
        }
    }

    pub fn receiver_ty(&self) -> TypeId {
        self.receiver_ty
    }

    pub fn resume_ty(&self) -> TypeId {
        self.resume_ty
    }

    pub fn answer_ty(&self) -> TypeId {
        self.answer_ty
    }

    pub fn return_ty(&self) -> TypeId {
        self.return_ty
    }

    pub fn out_effects(&self) -> &EffectRow {
        &self.out_effects
    }

    pub fn required_effects_include_runtime_error(&self) -> bool {
        self.includes_runtime_error_effect
    }
}

/// refactor typed HIR stage 显式输出的 effect / continuation contract side tables。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TypedHirEffectContracts {
    continuation_resume_sites: HashMap<CallSite, ContinuationResumeSiteContract>,
}

impl TypedHirEffectContracts {
    fn from_lowered_hir(lowered_hir: &LoweredHir, source_path: &Path) -> Self {
        let mut continuation_resume_sites = HashMap::new();

        for item in &lowered_hir.file.items {
            collect_continuation_resume_contracts_in_item(
                source_path,
                item,
                &lowered_hir.types,
                &mut continuation_resume_sites,
            );
        }

        for member_fun in &lowered_hir.member_funs {
            collect_continuation_resume_contracts_in_fun(
                member_fun,
                &lowered_hir.types,
                &mut continuation_resume_sites,
            );
        }

        Self {
            continuation_resume_sites,
        }
    }

    pub const fn is_placeholder(&self) -> bool {
        false
    }

    pub fn is_empty(&self) -> bool {
        self.continuation_resume_sites.is_empty()
    }

    pub fn continuation_resume_sites(&self) -> &HashMap<CallSite, ContinuationResumeSiteContract> {
        &self.continuation_resume_sites
    }

    pub fn continuation_resume_site(
        &self,
        call_site: &CallSite,
    ) -> Option<&ContinuationResumeSiteContract> {
        self.continuation_resume_sites.get(call_site)
    }
}

/// refactor typed HIR stage 的稳定输出形状。
///
/// 本阶段固定如下 invariants，供 P2/P3 及后续阶段直接消费：
/// - 输出已经过 resolver + typecheck，可直接视为 typed HIR handoff；
/// - `Continuation` / `resume` / `perform` / `handle` 的 typed contract 应在此阶段显式化，
///   下游不应再回 AST 猜测 surface 语义；
/// - `dump-hir` 的 refactor 路径必须优先消费这一 stage 输出，而不是 legacy
///   `hir::lower_for_dump(...)`；
/// - `effect_contracts` 现在显式输出 `Continuation.resume(...)` 的 typed contract，至少固定
///   `ResumeTuple` / `Answer` / `Out` 与 `Raise<RuntimeError>` ordinary effect 约束，供后续阶段
///   直接消费。
#[derive(Debug)]
pub struct TypedHirStageOutput {
    lowered_hir: LoweredHir,
    effect_contracts: TypedHirEffectContracts,
}

impl TypedHirStageOutput {
    pub(crate) fn new(lowered_hir: LoweredHir, source_path: &Path) -> Self {
        let effect_contracts = TypedHirEffectContracts::from_lowered_hir(&lowered_hir, source_path);
        Self {
            lowered_hir,
            effect_contracts,
        }
    }

    pub fn hir_file(&self) -> &crate::hir::File {
        &self.lowered_hir.file
    }

    pub fn types(&self) -> &TypeStore {
        &self.lowered_hir.types
    }

    pub fn lowered_hir(&self) -> &LoweredHir {
        &self.lowered_hir
    }

    pub fn effect_contracts(&self) -> &TypedHirEffectContracts {
        &self.effect_contracts
    }

    pub fn into_lowered_hir(self) -> LoweredHir {
        self.lowered_hir
    }
}

pub(crate) fn run(
    session: &Session,
    source: &SourceFile,
) -> Result<TypedHirStageOutput, HirLowerError> {
    let lowered_hir = crate::hir::lower_typed_for_dump(session, source)?;
    Ok(TypedHirStageOutput::new(lowered_hir, source.path()))
}

fn collect_continuation_resume_contracts_in_item(
    source_path: &Path,
    item: &Item,
    types: &TypeStore,
    out: &mut HashMap<CallSite, ContinuationResumeSiteContract>,
) {
    match item {
        Item::Fun(fun) => collect_continuation_resume_contracts_in_fun(fun, types, out),
        Item::Val(val) => {
            if let Some(init) = &val.init {
                collect_continuation_resume_contracts_in_expr(source_path, init, types, out);
            }
        }
        Item::Todo { .. } => {}
    }
}

fn collect_continuation_resume_contracts_in_fun(
    fun: &FunDecl,
    types: &TypeStore,
    out: &mut HashMap<CallSite, ContinuationResumeSiteContract>,
) {
    if let Some(body) = &fun.body {
        collect_continuation_resume_contracts_in_block(&fun.source_path, body, types, out);
    }
}

fn collect_continuation_resume_contracts_in_block(
    source_path: &Path,
    block: &crate::hir::Block,
    types: &TypeStore,
    out: &mut HashMap<CallSite, ContinuationResumeSiteContract>,
) {
    for stmt in &block.stmts {
        collect_continuation_resume_contracts_in_stmt(source_path, stmt, types, out);
    }
}

fn collect_continuation_resume_contracts_in_stmt(
    source_path: &Path,
    stmt: &Stmt,
    types: &TypeStore,
    out: &mut HashMap<CallSite, ContinuationResumeSiteContract>,
) {
    match &stmt.kind {
        StmtKind::Empty
        | StmtKind::Break { .. }
        | StmtKind::Continue { .. }
        | StmtKind::Todo(_) => {}
        StmtKind::Expr(expr) => {
            collect_continuation_resume_contracts_in_expr(source_path, expr, types, out);
        }
        StmtKind::Val(val) => {
            if let Some(init) = &val.init {
                collect_continuation_resume_contracts_in_expr(source_path, init, types, out);
            }
        }
        StmtKind::Assign { lhs, rhs, .. } => {
            collect_continuation_resume_contracts_in_expr(source_path, lhs, types, out);
            collect_continuation_resume_contracts_in_expr(source_path, rhs, types, out);
        }
        StmtKind::While { cond, body } => {
            collect_continuation_resume_contracts_in_expr(source_path, cond, types, out);
            collect_continuation_resume_contracts_in_block(source_path, body, types, out);
        }
        StmtKind::Return { value } => {
            if let Some(value) = value {
                collect_continuation_resume_contracts_in_expr(source_path, value, types, out);
            }
        }
    }
}

fn collect_continuation_resume_contracts_in_expr(
    source_path: &Path,
    expr: &Expr,
    types: &TypeStore,
    out: &mut HashMap<CallSite, ContinuationResumeSiteContract>,
) {
    maybe_record_continuation_resume_contract(source_path, expr, types, out);

    match &expr.kind {
        ExprKind::Missing
        | ExprKind::Literal(_)
        | ExprKind::VarRef(_)
        | ExprKind::UnresolvedIdent { .. }
        | ExprKind::Todo(_) => {}
        ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_continuation_resume_contracts_in_expr(
                    source_path,
                    &field.value,
                    types,
                    out,
                );
            }
        }
        ExprKind::TupleLit { elements } => {
            for element in elements {
                collect_continuation_resume_contracts_in_expr(source_path, element, types, out);
            }
        }
        ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let crate::hir::InterpolatedStringPart::Expr { expr } = part {
                    collect_continuation_resume_contracts_in_expr(source_path, expr, types, out);
                }
            }
        }
        ExprKind::Unary { expr, .. }
        | ExprKind::TypeCheck { expr, .. }
        | ExprKind::Cast { expr, .. }
        | ExprKind::MemberAccess { receiver: expr, .. } => {
            collect_continuation_resume_contracts_in_expr(source_path, expr, types, out);
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            collect_continuation_resume_contracts_in_expr(source_path, lhs, types, out);
            collect_continuation_resume_contracts_in_expr(source_path, rhs, types, out);
        }
        ExprKind::Block(block) => {
            collect_continuation_resume_contracts_in_block(source_path, block, types, out);
        }
        ExprKind::Closure(closure) => {
            collect_continuation_resume_contracts_in_expr(source_path, &closure.body, types, out);
        }
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_continuation_resume_contracts_in_expr(source_path, cond, types, out);
            collect_continuation_resume_contracts_in_expr(source_path, then_branch, types, out);
            if let Some(else_branch) = else_branch {
                collect_continuation_resume_contracts_in_expr(source_path, else_branch, types, out);
            }
        }
        ExprKind::When { subject, arms } => {
            collect_continuation_resume_contracts_in_expr(source_path, subject, types, out);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_continuation_resume_contracts_in_expr(source_path, guard, types, out);
                }
                collect_continuation_resume_contracts_in_expr(source_path, &arm.body, types, out);
            }
        }
        ExprKind::Call { callee, args } => {
            collect_continuation_resume_contracts_in_expr(source_path, callee, types, out);
            for arg in args {
                match arg {
                    CallArg::Positional(expr) => {
                        collect_continuation_resume_contracts_in_expr(
                            source_path,
                            expr,
                            types,
                            out,
                        );
                    }
                    CallArg::Named { value, .. } => {
                        collect_continuation_resume_contracts_in_expr(
                            source_path,
                            value,
                            types,
                            out,
                        );
                    }
                }
            }
        }
        ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    CallArg::Positional(expr) => {
                        collect_continuation_resume_contracts_in_expr(
                            source_path,
                            expr,
                            types,
                            out,
                        );
                    }
                    CallArg::Named { value, .. } => {
                        collect_continuation_resume_contracts_in_expr(
                            source_path,
                            value,
                            types,
                            out,
                        );
                    }
                }
            }
        }
        ExprKind::Handle(handle) => {
            collect_continuation_resume_contracts_in_block(source_path, &handle.body, types, out);
            for arm in &handle.arms {
                collect_continuation_resume_contracts_in_expr(source_path, &arm.body, types, out);
            }
            if let Some(finally) = &handle.finally {
                collect_continuation_resume_contracts_in_block(source_path, finally, types, out);
            }
        }
    }
}

fn maybe_record_continuation_resume_contract(
    source_path: &Path,
    expr: &Expr,
    types: &TypeStore,
    out: &mut HashMap<CallSite, ContinuationResumeSiteContract>,
) {
    let ExprKind::Call { callee, args } = &expr.kind else {
        return;
    };
    let ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) = &callee.kind else {
        return;
    };
    if fqn != "scoop.core.Continuation.resume" {
        return;
    }

    let Some(CallArg::Positional(receiver)) = args.first() else {
        return;
    };
    let Some((resume_ty, answer_ty, out_effects)) =
        continuation_receiver_contract(types, receiver.ty)
    else {
        return;
    };

    out.insert(
        CallSite::new(source_path.to_path_buf(), expr.span),
        ContinuationResumeSiteContract::new(
            receiver.ty,
            resume_ty,
            answer_ty,
            expr.ty,
            out_effects,
        ),
    );
}

fn continuation_receiver_contract(
    types: &TypeStore,
    receiver_ty: TypeId,
) -> Option<(TypeId, TypeId, EffectRow)> {
    let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = types.kind(receiver_ty) else {
        return None;
    };
    if nominal.fqn != "scoop.core.Continuation" || nominal.args.len() < 2 {
        return None;
    }

    Some((
        nominal.args[0],
        nominal.args[1],
        nominal.eff.clone().unwrap_or_else(EffectRow::pure),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::session::{EffectPipelineMode, SessionOptions};

    fn refactor_session() -> Session {
        Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor)).unwrap()
    }

    #[test]
    fn refactor_typed_hir_stage_output_is_constructible() {
        let session = refactor_session();
        let source = SourceFile::new_virtual("<mem>", "package sample\nfun main() {}\n");

        let output = run(&session, &source).unwrap();

        assert_eq!(output.hir_file().items.len(), 1);
        assert!(!output.effect_contracts().is_placeholder());
        assert!(output.effect_contracts().is_empty());
    }

    #[test]
    fn refactor_typed_hir_stage_builds_explicit_contract_tables() {
        let session = refactor_session();
        let source = SourceFile::new_virtual("<mem>", "package sample\nfun main() {}\n");

        let output = run(&session, &source).unwrap();

        assert!(!output.types().is_empty());
        assert!(!output.effect_contracts().is_placeholder());
    }

    #[test]
    fn refactor_continuation_typecheck_records_resume_contracts_in_typed_hir_stage() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_continuation_contracts.scoop",
            r#"
package fixtures.hirstage

import scoop.core.*

effect Boom {
    fun next(): Int
}

fun resumeWithEffects(k: Continuation<Int, Int, eff Boom>): Int / (Boom + Raise<RuntimeError>) {
    return k.resume(1)
}
"#,
        );

        let output = run(&session, &source).unwrap();
        let contracts = output.effect_contracts();

        assert_eq!(contracts.continuation_resume_sites().len(), 1);
        let (call_site, contract) = contracts
            .continuation_resume_sites()
            .iter()
            .next()
            .expect("应收集到唯一的 continuation resume contract");

        assert_eq!(call_site.source_path, source.path());
        assert_eq!(
            output.types().display(contract.receiver_ty()).to_string(),
            "scoop.core.Continuation<Int, Int, eff fixtures.hirstage.Boom>"
        );
        assert_eq!(
            output.types().display(contract.resume_ty()).to_string(),
            "Int"
        );
        assert_eq!(
            output.types().display(contract.answer_ty()).to_string(),
            "Int"
        );
        assert_eq!(
            output.types().display(contract.return_ty()).to_string(),
            "Int"
        );
        assert_eq!(contract.out_effects().terms.len(), 1);
        assert_eq!(
            output
                .types()
                .display(contract.out_effects().terms[0])
                .to_string(),
            "fixtures.hirstage.Boom"
        );
        assert!(contract.required_effects_include_runtime_error());
    }
}
