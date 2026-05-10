use std::collections::HashSet;
use std::path::Path;

use crate::hir::{self, LoweredHir};
use crate::llvm::LlvmEmitError;
use crate::opt::OptLevel;
use crate::session::Session;
use crate::source::{SourceFile, SourceId, SourceMap};
use crate::span::Span;
use crate::ty::{TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::{
    EffectLoweredStageOutput, LlvmArtifactKind, TypedHirStageOutput,
    build_effect_facts_stage_output_with_compilation_sources, build_effect_lowered_stage_output,
    mir_stage,
};

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(test)]
static TEST_STAGE_RUNS: AtomicUsize = AtomicUsize::new(0);

#[cfg(test)]
thread_local! {
    static TEST_STAGE_RUN_COUNTING_ENABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// LLVM codegen stage 的显式输入。
///
/// 约束：
/// - `lowered_hir` 必须来自 build/frontend 的统一 typed lowering；
/// - `abi_visibility_lowered_hir` 若存在，只能用于发布 request-source 范围的 ABI shell；它不能改变
///   reachable body lowering / fail-fast 的 authoritative handoff；
/// - stage 会显式把它推进到 P5 late-lowered handoff；
/// - stage 输出中的 `hir_compat_scaffold` 仅保留当前仍由通用 LLVM codegen 复用的非 effect side
///   tables，不能再作为 effect lowering 的 authoritative 输入。
#[derive(Debug)]
pub struct LlvmCodegenStageInput {
    lowered_hir: LoweredHir,
    abi_visibility_lowered_hir: Option<LoweredHir>,
    source_map: SourceMap,
    entry_source_id: SourceId,
    entry_main_fqn: Option<String>,
    opt_level: OptLevel,
}

impl LlvmCodegenStageInput {
    pub fn new(
        lowered_hir: LoweredHir,
        abi_visibility_lowered_hir: Option<LoweredHir>,
        source_map: SourceMap,
        entry_source_id: SourceId,
        entry_main_fqn: Option<String>,
        opt_level: OptLevel,
    ) -> Self {
        Self {
            lowered_hir,
            abi_visibility_lowered_hir,
            source_map,
            entry_source_id,
            entry_main_fqn,
            opt_level,
        }
    }
}

/// LLVM codegen stage 的稳定 handoff。
///
/// 说明：
/// - `effect_lowered_stage_output` 是 P5 -> P6 的 authoritative handoff；
/// - `abi_visibility_effect_lowered_stage_output` 若存在，则只用于发布 build fixture / ABI 断言所需的
///   request-source callable shell，可见性与 reachable body lowering 明确分离；
/// - `hir_compat_scaffold` 只为当前仍未迁出的通用 LLVM 布局/顶层索引查询提供过渡输入；
/// - 该 scaffold 明确不再携带 `materialized_mir/pass_view`，避免 refactor 路径再回落到旧的
///   `materialized_lowered_hir` emit helper；
/// - `.ll/.o/.s` 三类产物都必须共用这份 handoff，再进入新的 refactor emit API。
#[derive(Debug)]
pub struct LlvmCodegenStageOutput {
    source_map: SourceMap,
    entry_source_id: SourceId,
    entry_main_fqn: Option<String>,
    opt_level: OptLevel,
    hir_compat_scaffold: LoweredHir,
    effect_lowered_stage_output: EffectLoweredStageOutput,
    abi_visibility_effect_lowered_stage_output: Option<EffectLoweredStageOutput>,
}

impl LlvmCodegenStageOutput {
    fn new(
        source_map: SourceMap,
        entry_source_id: SourceId,
        entry_main_fqn: Option<String>,
        opt_level: OptLevel,
        hir_compat_scaffold: LoweredHir,
        effect_lowered_stage_output: EffectLoweredStageOutput,
        abi_visibility_effect_lowered_stage_output: Option<EffectLoweredStageOutput>,
    ) -> Self {
        Self {
            source_map,
            entry_source_id,
            entry_main_fqn,
            opt_level,
            hir_compat_scaffold,
            effect_lowered_stage_output,
            abi_visibility_effect_lowered_stage_output,
        }
    }

    pub fn source_map(&self) -> &SourceMap {
        &self.source_map
    }

    pub fn entry_source_id(&self) -> SourceId {
        self.entry_source_id
    }

    pub fn entry_main_fqn(&self) -> Option<&str> {
        self.entry_main_fqn.as_deref()
    }

    pub fn opt_level(&self) -> OptLevel {
        self.opt_level
    }

    pub fn hir_compat_scaffold(&self) -> &LoweredHir {
        &self.hir_compat_scaffold
    }

    pub fn effect_lowered_stage_output(&self) -> &EffectLoweredStageOutput {
        &self.effect_lowered_stage_output
    }

    pub fn abi_visibility_effect_lowered_stage_output(&self) -> Option<&EffectLoweredStageOutput> {
        self.abi_visibility_effect_lowered_stage_output.as_ref()
    }
}

fn run_effect_lowered_stage_from_lowered_hir(
    session: &Session,
    source_map: &SourceMap,
    entry_source: &SourceFile,
    lowered_hir: LoweredHir,
    preserve_published_resume_shells: bool,
) -> Result<EffectLoweredStageOutput, LlvmEmitError> {
    precheck_invalid_integer_literals(source_map, entry_source, &lowered_hir)?;
    let source_path = entry_source.path().to_path_buf();
    let typed_hir_output = TypedHirStageOutput::new(lowered_hir, &source_path)
        .map_err(crate::hir::HirLowerError::from)?;
    let mir_stage_output =
        mir_stage::run(typed_hir_output).map_err(|err| stage_error("direct-style MIR", err))?;
    let compilation_sources = source_map_compilation_sources(session, source_map);
    let effect_facts_stage_output = build_effect_facts_stage_output_with_compilation_sources(
        session,
        entry_source,
        &compilation_sources,
        mir_stage_output,
    )
    .map_err(|err| stage_error("effect facts", err))?;
    let effect_lowered_stage_output = if preserve_published_resume_shells {
        super::effect_lowering_stage::run_preserving_published_resume_shells(
            effect_facts_stage_output,
        )
    } else {
        build_effect_lowered_stage_output(session, effect_facts_stage_output)
    };
    effect_lowered_stage_output.map_err(|err| stage_error("late lowering", err))
}

fn source_map_compilation_sources(session: &Session, source_map: &SourceMap) -> Vec<SourceFile> {
    let sysroot_paths = session
        .sysroot()
        .files
        .iter()
        .map(|file| file.source.path().to_path_buf())
        .collect::<HashSet<_>>();
    source_map
        .sources()
        .filter(|source| !sysroot_paths.contains(source.path()))
        .cloned()
        .collect()
}

pub(crate) fn run(
    session: &Session,
    input: LlvmCodegenStageInput,
) -> Result<LlvmCodegenStageOutput, LlvmEmitError> {
    #[cfg(test)]
    record_test_stage_run();

    let LlvmCodegenStageInput {
        lowered_hir,
        abi_visibility_lowered_hir,
        source_map,
        entry_source_id,
        entry_main_fqn,
        opt_level,
    } = input;
    let entry_source =
        source_map
            .source(entry_source_id)
            .ok_or_else(|| LlvmEmitError::Frontend {
                message: format!(
                    "refactor LLVM stage 找不到入口源文件（source_id={})",
                    entry_source_id.as_usize()
                ),
            })?;
    let hir_compat_scaffold = lowered_hir.clone_hir_compat_scaffold_without_materialized_mir();
    let effect_lowered_stage_output = run_effect_lowered_stage_from_lowered_hir(
        session,
        &source_map,
        entry_source,
        lowered_hir,
        false,
    )?;
    let abi_visibility_effect_lowered_stage_output = abi_visibility_lowered_hir
        .map(|lowered_hir| {
            run_effect_lowered_stage_from_lowered_hir(
                session,
                &source_map,
                entry_source,
                lowered_hir,
                true,
            )
        })
        .transpose()?;

    Ok(LlvmCodegenStageOutput::new(
        source_map,
        entry_source_id,
        entry_main_fqn,
        opt_level,
        hir_compat_scaffold,
        effect_lowered_stage_output,
        abi_visibility_effect_lowered_stage_output,
    ))
}

pub(crate) fn emit_artifact_to_file(
    session: &Session,
    input: LlvmCodegenStageInput,
    output: &Path,
    artifact: LlvmArtifactKind,
) -> Result<(), LlvmEmitError> {
    let stage_output = run(session, input)?;
    match artifact {
        LlvmArtifactKind::LlvmIr => crate::llvm::emit_main_ir_to_file_from_stage_output(
            stage_output.source_map(),
            stage_output.entry_source_id(),
            crate::llvm::StageEmitInput::new(
                stage_output.hir_compat_scaffold(),
                stage_output.effect_lowered_stage_output(),
                stage_output.abi_visibility_effect_lowered_stage_output(),
            ),
            output,
            stage_output.entry_main_fqn(),
            stage_output.opt_level(),
        ),
        LlvmArtifactKind::Object => crate::llvm::emit_main_obj_to_file_from_stage_output(
            stage_output.source_map(),
            stage_output.entry_source_id(),
            crate::llvm::StageEmitInput::new(
                stage_output.hir_compat_scaffold(),
                stage_output.effect_lowered_stage_output(),
                stage_output.abi_visibility_effect_lowered_stage_output(),
            ),
            output,
            stage_output.entry_main_fqn(),
            stage_output.opt_level(),
        ),
        LlvmArtifactKind::Asm => crate::llvm::emit_main_asm_to_file_from_stage_output(
            stage_output.source_map(),
            stage_output.entry_source_id(),
            crate::llvm::StageEmitInput::new(
                stage_output.hir_compat_scaffold(),
                stage_output.effect_lowered_stage_output(),
                stage_output.abi_visibility_effect_lowered_stage_output(),
            ),
            output,
            stage_output.entry_main_fqn(),
            stage_output.opt_level(),
        ),
    }
}

#[derive(Clone, Copy)]
struct PrecheckIntTy {
    bits: u32,
    signed: bool,
}

fn precheck_invalid_integer_literals(
    source_map: &SourceMap,
    entry_source: &SourceFile,
    lowered_hir: &LoweredHir,
) -> Result<(), LlvmEmitError> {
    let entry_source_id = source_map.source_id_of_path(entry_source.path());
    for item in &lowered_hir.file.items {
        match item {
            hir::Item::Fun(fun) => {
                precheck_fun_integer_literals(source_map, &lowered_hir.types, fun)?
            }
            hir::Item::Val(decl) => {
                if let Some(source_id) = entry_source_id
                    && let Some(init) = &decl.init
                {
                    precheck_expr_integer_literals_with_expected(
                        source_map,
                        source_id,
                        &lowered_hir.types,
                        init,
                        Some(decl.ty),
                    )?;
                }
            }
            hir::Item::Todo { .. } => {}
        }
    }
    for fun in &lowered_hir.member_funs {
        precheck_fun_integer_literals(source_map, &lowered_hir.types, fun)?;
    }
    Ok(())
}

fn precheck_fun_integer_literals(
    source_map: &SourceMap,
    types: &TypeStore,
    fun: &hir::FunDecl,
) -> Result<(), LlvmEmitError> {
    let Some(source_id) = source_map.source_id_of_path(&fun.source_path) else {
        return Ok(());
    };
    if let Some(body) = &fun.body {
        precheck_block_integer_literals(source_map, source_id, types, body)?;
    }
    Ok(())
}

fn precheck_block_integer_literals(
    source_map: &SourceMap,
    source_id: SourceId,
    types: &TypeStore,
    block: &hir::Block,
) -> Result<(), LlvmEmitError> {
    for stmt in &block.stmts {
        precheck_stmt_integer_literals(source_map, source_id, types, stmt)?;
    }
    Ok(())
}

fn precheck_stmt_integer_literals(
    source_map: &SourceMap,
    source_id: SourceId,
    types: &TypeStore,
    stmt: &hir::Stmt,
) -> Result<(), LlvmEmitError> {
    match &stmt.kind {
        hir::StmtKind::Empty | hir::StmtKind::Break { .. } | hir::StmtKind::Continue { .. } => {}
        hir::StmtKind::Expr(expr) => {
            precheck_expr_integer_literals(source_map, source_id, types, expr)?;
        }
        hir::StmtKind::Val(decl) => {
            if let Some(init) = &decl.init {
                precheck_expr_integer_literals_with_expected(
                    source_map,
                    source_id,
                    types,
                    init,
                    Some(decl.ty),
                )?;
            }
        }
        hir::StmtKind::Assign { lhs, rhs, .. } => {
            precheck_expr_integer_literals(source_map, source_id, types, lhs)?;
            precheck_expr_integer_literals_with_expected(
                source_map,
                source_id,
                types,
                rhs,
                Some(lhs.ty),
            )?;
        }
        hir::StmtKind::While { cond, body } => {
            precheck_expr_integer_literals(source_map, source_id, types, cond)?;
            precheck_block_integer_literals(source_map, source_id, types, body)?;
        }
        hir::StmtKind::Return { value } => {
            if let Some(value) = value {
                precheck_expr_integer_literals(source_map, source_id, types, value)?;
            }
        }
        hir::StmtKind::Todo(_) => {}
    }
    Ok(())
}

fn precheck_expr_integer_literals(
    source_map: &SourceMap,
    source_id: SourceId,
    types: &TypeStore,
    expr: &hir::Expr,
) -> Result<(), LlvmEmitError> {
    precheck_expr_integer_literals_with_expected(source_map, source_id, types, expr, None)
}

fn precheck_expr_integer_literals_with_expected(
    source_map: &SourceMap,
    source_id: SourceId,
    types: &TypeStore,
    expr: &hir::Expr,
    expected_ty: Option<TypeId>,
) -> Result<(), LlvmEmitError> {
    let target_ty = expected_ty.unwrap_or(expr.ty);
    match &expr.kind {
        hir::ExprKind::Literal(hir::LiteralKind::Int) => {
            precheck_positive_int_literal(source_map, source_id, types, expr.span, target_ty)?;
        }
        hir::ExprKind::Unary {
            op: crate::ast::UnaryOp::Neg,
            expr: inner,
            ..
        } if matches!(inner.kind, hir::ExprKind::Literal(hir::LiteralKind::Int)) => {
            precheck_negative_int_literal(
                source_map, source_id, types, expr.span, inner.span, target_ty,
            )?;
        }
        hir::ExprKind::Unary { expr: inner, .. }
        | hir::ExprKind::TypeCheck { expr: inner, .. }
        | hir::ExprKind::Cast { expr: inner, .. }
        | hir::ExprKind::MemberAccess {
            receiver: inner, ..
        } => {
            precheck_expr_integer_literals(source_map, source_id, types, inner)?;
        }
        hir::ExprKind::StructLit { fields, .. } => {
            for field in fields {
                precheck_expr_integer_literals(source_map, source_id, types, &field.value)?;
            }
        }
        hir::ExprKind::TupleLit { elements } => {
            for element in elements {
                precheck_expr_integer_literals(source_map, source_id, types, element)?;
            }
        }
        hir::ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let hir::InterpolatedStringPart::Expr { expr } = part {
                    precheck_expr_integer_literals(source_map, source_id, types, expr)?;
                }
            }
        }
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            precheck_expr_integer_literals(source_map, source_id, types, lhs)?;
            precheck_expr_integer_literals(source_map, source_id, types, rhs)?;
        }
        hir::ExprKind::Block(block) => {
            precheck_block_integer_literals(source_map, source_id, types, block)?;
        }
        hir::ExprKind::Closure(closure) => {
            precheck_expr_integer_literals(source_map, source_id, types, &closure.body)?;
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            precheck_expr_integer_literals(source_map, source_id, types, cond)?;
            precheck_expr_integer_literals(source_map, source_id, types, then_branch)?;
            if let Some(else_branch) = else_branch {
                precheck_expr_integer_literals(source_map, source_id, types, else_branch)?;
            }
        }
        hir::ExprKind::When { subject, arms } => {
            precheck_expr_integer_literals(source_map, source_id, types, subject)?;
            for arm in arms {
                precheck_when_pattern_integer_literals(
                    source_map, source_id, types, &arm.pat, subject.ty,
                )?;
                if let Some(guard) = &arm.guard {
                    precheck_expr_integer_literals(source_map, source_id, types, guard)?;
                }
                precheck_expr_integer_literals(source_map, source_id, types, &arm.body)?;
            }
        }
        hir::ExprKind::Call { callee, args } => {
            precheck_expr_integer_literals(source_map, source_id, types, callee)?;
            for arg in args {
                precheck_call_arg_integer_literals(source_map, source_id, types, arg)?;
            }
        }
        hir::ExprKind::Perform { args, .. } => {
            for arg in args {
                precheck_call_arg_integer_literals(source_map, source_id, types, arg)?;
            }
        }
        hir::ExprKind::Handle(handle) => {
            precheck_block_integer_literals(source_map, source_id, types, &handle.body)?;
            for arm in &handle.arms {
                precheck_expr_integer_literals(source_map, source_id, types, &arm.body)?;
            }
            if let Some(finally) = &handle.finally {
                precheck_block_integer_literals(source_map, source_id, types, finally)?;
            }
        }
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::ClassLiteral(_)
        | hir::ExprKind::VarRef(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::Todo(_) => {}
    }
    Ok(())
}

fn precheck_call_arg_integer_literals(
    source_map: &SourceMap,
    source_id: SourceId,
    types: &TypeStore,
    arg: &hir::CallArg,
) -> Result<(), LlvmEmitError> {
    match arg {
        hir::CallArg::Positional(expr) | hir::CallArg::Named { value: expr, .. } => {
            precheck_expr_integer_literals(source_map, source_id, types, expr)
        }
    }
}

fn precheck_when_pattern_integer_literals(
    source_map: &SourceMap,
    source_id: SourceId,
    types: &TypeStore,
    pat: &hir::WhenPat,
    subject_ty: TypeId,
) -> Result<(), LlvmEmitError> {
    match pat {
        hir::WhenPat::IntLit { span, raw } => {
            precheck_int_literal_text(source_map, source_id, types, *span, raw, subject_ty)?;
        }
        hir::WhenPat::Or { pats, .. } => {
            for pat in pats {
                precheck_when_pattern_integer_literals(
                    source_map, source_id, types, pat, subject_ty,
                )?;
            }
        }
        hir::WhenPat::Else { .. }
        | hir::WhenPat::Wildcard { .. }
        | hir::WhenPat::Rest { .. }
        | hir::WhenPat::Is { .. }
        | hir::WhenPat::Bind { .. }
        | hir::WhenPat::Tuple { .. }
        | hir::WhenPat::Variant { .. }
        | hir::WhenPat::CharLit { .. }
        | hir::WhenPat::StringLit { .. }
        | hir::WhenPat::BoolLit { .. } => {}
    }
    Ok(())
}

fn precheck_positive_int_literal(
    source_map: &SourceMap,
    source_id: SourceId,
    types: &TypeStore,
    span: Span,
    ty: TypeId,
) -> Result<(), LlvmEmitError> {
    let Some(text) = source_text(source_map, source_id, span) else {
        return Ok(());
    };
    precheck_int_literal_text(source_map, source_id, types, span, text, ty)
}

fn precheck_negative_int_literal(
    source_map: &SourceMap,
    source_id: SourceId,
    types: &TypeStore,
    span: Span,
    literal_span: Span,
    ty: TypeId,
) -> Result<(), LlvmEmitError> {
    let Some(text) = source_text(source_map, source_id, span) else {
        return Ok(());
    };
    let Some(literal_text) = source_text(source_map, source_id, literal_span) else {
        return Ok(());
    };
    let Some(int_ty) = precheck_int_ty(types, ty) else {
        return Ok(());
    };
    let source = source_map
        .source(source_id)
        .ok_or_else(|| LlvmEmitError::Frontend {
            message: format!(
                "refactor LLVM literal precheck 找不到 source_id={}",
                source_id.as_usize()
            ),
        })?;
    let raw =
        crate::syntax::int_literal::parse_int_literal_checked(literal_text).map_err(|err| {
            LlvmEmitError::invalid_literal(source, span, "integer literal", err.reason(), text)
        })?;
    if checked_negated_int_literal_bits(raw, int_ty).is_none() {
        return Err(LlvmEmitError::invalid_literal(
            source,
            span,
            "integer literal",
            "超出目标整数类型可表示范围",
            text,
        ));
    }
    Ok(())
}

fn precheck_int_literal_text(
    source_map: &SourceMap,
    source_id: SourceId,
    types: &TypeStore,
    span: Span,
    text: &str,
    ty: TypeId,
) -> Result<(), LlvmEmitError> {
    let Some(int_ty) = precheck_int_ty(types, ty) else {
        return Ok(());
    };
    let Some((negative, body)) = source_text_int_literal_body(text) else {
        return Ok(());
    };
    let source = source_map
        .source(source_id)
        .ok_or_else(|| LlvmEmitError::Frontend {
            message: format!(
                "refactor LLVM literal precheck 找不到 source_id={}",
                source_id.as_usize()
            ),
        })?;
    let raw = crate::syntax::int_literal::parse_int_literal_checked(body).map_err(|err| {
        LlvmEmitError::invalid_literal(source, span, "integer literal", err.reason(), text)
    })?;
    let valid = if negative {
        checked_negated_int_literal_bits(raw, int_ty).is_some()
    } else {
        checked_positive_int_literal_bits(raw, int_ty).is_some()
    };
    if !valid {
        return Err(LlvmEmitError::invalid_literal(
            source,
            span,
            "integer literal",
            "超出目标整数类型可表示范围",
            text,
        ));
    }
    Ok(())
}

fn source_text(source_map: &SourceMap, source_id: SourceId, span: Span) -> Option<&str> {
    let bound = source_map.bind_span(source_id, span).ok()?;
    source_map.slice(bound).ok()
}

fn precheck_int_ty(types: &TypeStore, ty: TypeId) -> Option<PrecheckIntTy> {
    match types.kind(ty) {
        TypeKind::Value(ValueTypeKind::Int) => Some(PrecheckIntTy {
            bits: 64,
            signed: true,
        }),
        TypeKind::Value(ValueTypeKind::UInt) => Some(PrecheckIntTy {
            bits: 64,
            signed: false,
        }),
        TypeKind::Value(ValueTypeKind::IntN(bits)) => Some(PrecheckIntTy {
            bits: u32::from(*bits),
            signed: true,
        }),
        TypeKind::Value(ValueTypeKind::UIntN(bits)) => Some(PrecheckIntTy {
            bits: u32::from(*bits),
            signed: false,
        }),
        _ => None,
    }
}

fn checked_positive_int_literal_bits(value: u128, int_ty: PrecheckIntTy) -> Option<u128> {
    let max = if int_ty.signed {
        signed_int_max(int_ty.bits)
    } else {
        unsigned_int_max(int_ty.bits)
    };
    (value <= max).then_some(value)
}

fn checked_negated_int_literal_bits(value: u128, int_ty: PrecheckIntTy) -> Option<u128> {
    if !int_ty.signed {
        return None;
    }
    let min_abs = signed_int_min_abs(int_ty.bits);
    (value <= min_abs).then_some(mask_to_bits(0u128.wrapping_sub(value), int_ty.bits))
}

fn mask_to_bits(value: u128, bits: u32) -> u128 {
    if bits == 0 {
        return 0;
    }
    if bits >= 128 {
        return value;
    }
    value & ((1u128 << bits) - 1)
}

fn unsigned_int_max(bits: u32) -> u128 {
    if bits == 0 {
        return 0;
    }
    if bits >= 128 {
        return u128::MAX;
    }
    (1u128 << bits) - 1
}

fn signed_int_max(bits: u32) -> u128 {
    if bits <= 1 {
        return 0;
    }
    if bits >= 128 {
        return i128::MAX as u128;
    }
    (1u128 << (bits - 1)) - 1
}

fn signed_int_min_abs(bits: u32) -> u128 {
    if bits == 0 {
        return 0;
    }
    if bits >= 128 {
        return 1u128 << 127;
    }
    1u128 << (bits - 1)
}

fn source_text_int_literal_body(text: &str) -> Option<(bool, &str)> {
    let (negative, body) = if let Some(rest) = text.strip_prefix('-') {
        (true, rest)
    } else {
        (false, text)
    };
    if body.is_empty() || !body.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        return None;
    }
    if let Some(rest) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        return rest
            .bytes()
            .all(|b| char::from(b).is_ascii_hexdigit() || b == b'_')
            .then_some((negative, body));
    }
    if let Some(rest) = body.strip_prefix("0b").or_else(|| body.strip_prefix("0B")) {
        return rest
            .bytes()
            .all(|b| matches!(b, b'0' | b'1' | b'_'))
            .then_some((negative, body));
    }
    body.bytes()
        .all(|b| char::from(b).is_ascii_digit() || b == b'_')
        .then_some((negative, body))
}

fn stage_error(stage: &'static str, error: impl std::fmt::Display) -> LlvmEmitError {
    LlvmEmitError::Frontend {
        message: format!("refactor LLVM stage `{stage}` 失败：{error}"),
    }
}

#[cfg(test)]
fn record_test_stage_run() {
    TEST_STAGE_RUN_COUNTING_ENABLED.with(|enabled| {
        if enabled.get() {
            TEST_STAGE_RUNS.fetch_add(1, Ordering::SeqCst);
        }
    });
}

#[cfg(test)]
fn reset_test_stage_run_count() {
    TEST_STAGE_RUNS.store(0, Ordering::SeqCst);
}

#[cfg(test)]
struct TestStageRunCountGuard;

#[cfg(test)]
impl Drop for TestStageRunCountGuard {
    fn drop(&mut self) {
        TEST_STAGE_RUN_COUNTING_ENABLED.with(|enabled| enabled.set(false));
    }
}

#[cfg(test)]
fn enable_test_stage_run_counting() -> TestStageRunCountGuard {
    reset_test_stage_run_count();
    TEST_STAGE_RUN_COUNTING_ENABLED.with(|enabled| enabled.set(true));
    TestStageRunCountGuard
}

#[cfg(test)]
fn test_stage_run_count() -> usize {
    TEST_STAGE_RUNS.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    use inkwell::context::Context;

    use super::{LlvmCodegenStageInput, enable_test_stage_run_counting, test_stage_run_count};
    use crate::llvm::{LlvmEmitError, build_main_module_from_stage_output};
    use crate::opt::OptLevel;
    use crate::pipeline::{self as pipeline, LlvmArtifactKind};
    use crate::session::{Session, SessionOptions};
    use crate::source::{SourceFile, SourceMap};

    fn session() -> Session {
        Session::with_options(SessionOptions::new()).unwrap()
    }

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    struct TempDirGuard(PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    impl TempDirGuard {
        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    fn make_temp_dir() -> TempDirGuard {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let ordinal = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "scoopc_refactor_llvm_codegen_stage_{}_{}_{}",
            std::process::id(),
            unique,
            ordinal
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempDirGuard(dir)
    }

    fn sample_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/refactor_llvm_codegen_stage_fixture.scoop",
            r#"
package sample

fun main(): Int {
    return 0
}
"#,
        )
    }

    fn effectful_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/refactor_llvm_codegen_stage_effectful_fixture.scoop",
            r#"
package sample

import scoop.core.Raise

fun main(): Int {
    return handle {
        Raise.raise(1)
        0
    } with {
        Raise.raise(e) -> 2
    }
}
"#,
        )
    }

    fn member_codegen_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/refactor_mir_member_codegen_fixture.scoop",
            r#"
package sample

class Cell(var count: Int)

fun bump(cell: Cell): Int {
    cell.count = cell.count + 1
    return cell.count
}

fun main(): Int {
    val cell = Cell(41)
    return bump(cell)
}
"#,
        )
    }

    fn unhandled_outward_entry_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/refactor_main_unhandled_outward_fixture.scoop",
            r#"
package sample

effect Ping {
    fun hit(): Unit
}

fun effectEntry(): Unit / Ping {
    Ping.hit()
}

fun main() {}
"#,
        )
    }

    fn array_string_main_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/refactor_main_array_string_fixture.scoop",
            r#"
package sample

import scoop.core.*

fun main(args: Array<String>): Int {
    return args.size()
}
"#,
        )
    }

    fn runtime_type_primitives_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/refactor_runtime_type_primitives_fixture.scoop",
            r#"
package sample

import scoop.core.*

interface IFace {
    fun ping(): Int
}

open class Base()
class Impl() : Base(), IFace {
    fun ping(): Int {
        return 42
    }
}
class Other() : Base()

fun classifyValue(x: Int): Int {
    return when (x) {
        is Int -> 7
        else -> 9
    }
}

fun main(): Int {
    val x: Any = Impl()
    val _is_iface: Bool = x is IFace
    val _not_other: Bool = x !is Other
    val _maybe_iface: IFace? = x as? IFace
    val _maybe_other: Other? = x as? Other
    val _value_pattern: Int = classifyValue(1)
    val caught: Int = try {
        val _bad: Other = x as Other
        0
    } catch (e: RuntimeError) {
        val _unused: RuntimeError = e
        1
    }
    return caught
}
"#,
        )
    }

    fn composite_transport_contract_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/refactor_composite_transport_contract_fixture.scoop",
            r#"
package sample

import scoop.core.*

struct Named(val name: String, val score: Int)

fun makeNamed(): Named {
    return Named { name: "hi", score: 1 }
}

fun main(): Int {
    val named: Named = makeNamed()
    __scoop_gc_collect()
    return named.score
}
"#,
        )
    }

    fn value_boxing_transport_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/refactor_value_boxing_transport_fixture.scoop",
            r#"
package sample

import scoop.core.*

struct Named(val name: String, val score: Int)

fun keepAny(value: Any): Int {
    __scoop_gc_collect()
    return 1
}

fun makeAny(named: Named): Any {
    val localAny: Any = named
    val tupleAny: Any = (named, Named("tuple", 2))
    keepAny(tupleAny)
    return localAny
}

fun main(): Int {
    val boxed = makeAny(Named("main", 1))
    return keepAny(boxed)
}
"#,
        )
    }

    fn enum_payload_transport_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/refactor_enum_payload_transport_fixture.scoop",
            r#"
package sample

import scoop.core.*

struct Point(val x: Int, val y: Int)

enum Inner {
    Hit(val value: Int),
    Miss,
}

enum Outer {
    UnitPair(val marker: Unit, val point: Point),
    Nested(val inner: Inner, val payload: (Point, Int)),
}

fun marker(): Unit {}

fun keepAny(value: Any): Int {
    __scoop_gc_collect()
    return 1
}

fun makeAny(): Any {
    return Nested(Hit(2), (Point(3, 4), 5))
}

fun eval(x: Outer): Int {
    return when (x) {
        UnitPair(_, point) -> point.x + point.y
        Nested(Hit(v), (point, n)) -> v + point.x + n
        Nested(Miss, (_, n)) -> n
    }
}

fun main(): Int {
    val unitPayload: Outer = UnitPair(marker(), Point(1, 2))
    val nested: Outer = Nested(Hit(7), (Point(8, 9), 10))
    val erased: Any = makeAny()
    return eval(unitPayload) + eval(nested) + keepAny(erased)
}
"#,
        )
    }

    fn array_composite_transport_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/refactor_array_composite_transport_fixture.scoop",
            r#"
package sample

import scoop.core.*

struct Point(val x: Int, val y: Int)

enum Item {
    Hit(val point: Point),
    Pair(val payload: (Point, Int)),
}

fun score(item: Item): Int {
    return when (item) {
        Hit(point) -> point.x + point.y
        Pair(payload) -> payload._0.x + payload._0.y + payload._1
    }
}

fun main(): Int {
    val points: Array<Point> = [Point(1, 2), Point(3, 4)]
    val p: Point = points.get(1)

    val pairs: MutableArray<(Point, Int)> = [(Point(5, 6), 7)]
    val first: (Point, Int) = pairs.get(0)
    pairs.set(0, (Point(8, 9), 10))
    val second: (Point, Int) = pairs.get(0)

    val items: MutableArray<Item> = [Hit(Point(11, 12)), Pair((Point(13, 14), 15))]
    val before: Int = score(items.get(1))
    items.set(0, Pair((Point(16, 17), 18)))
    val after: Int = score(items.get(0))

    return p.x + p.y + first._0.x + first._0.y + first._1 + second._0.x + second._0.y + second._1 + before + after
}
"#,
        )
    }

    fn closure_env_transport_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/refactor_closure_env_transport_fixture.scoop",
            r#"
package sample

import scoop.core.*

struct Point(val x: Int, val label: String)

enum Item {
    Hit(val point: Point),
    Pair(val payload: (Point, Int)),
}

fun keepAny(value: Any): Int {
    __scoop_gc_collect()
    return 1
}

fun score(item: Item): Int {
    return when (item) {
        Hit(point) -> point.x
        Pair(payload) -> payload._0.x + payload._1
    }
}

fun callAfterGc(f: () -> Int): Int {
    __scoop_gc_collect()
    return f()
}

fun main(): Int {
    val title: String = "cap"
    val point: Point = Point(1, title)
    val pair: (Point, Int) = (Point(2, "tuple"), 3)
    val item: Item = Pair((Point(4, "enum"), 5))
    val points: Array<Point> = [Point(6, "array")]
    var mutablePoint: Point = Point(7, "box")

    val f: () -> Int = {
        mutablePoint = Point(mutablePoint.x + 1, title)
        point.x + pair._0.x + pair._1 + score(item) + points.get(0).x + mutablePoint.x + keepAny(title)
    }
    return callAfterGc(f)
}
"#,
        )
    }

    fn cross_thread_resume_payload_transport_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/refactor_cross_thread_resume_payload_transport_fixture.scoop",
            r#"
package sample

import scoop.core.*

struct Point(val x: Int, val label: String)

effect AwaitPoint {
    fun next(): Point
}

fun main(): Int {
    var saved: Continuation<Point, Unit>? = None()
    var observed: Int = 0

    val handled: Unit = handle {
        val point: Point = AwaitPoint.next()
        __scoop_gc_collect()
        observed = point.x
    } with {
        AwaitPoint.next(), k -> {
            saved = Some(k)
        }
    }

    when (saved) {
        Some(k) -> {
            saved = None()
            if (observed < 0) {
                val ignored: Unit = try {
                    k.resume(Point(0, "unused"))
                } catch (e: RuntimeError) {
                    val unused: RuntimeError = e
                }
            }
            __scoop_thread_spawn_join_resume(k, Point(7, "resume"))
        }
        None -> {}
    }
    return observed
}
"#,
        )
    }

    fn emit_refactor_ir_for_source(source: SourceFile, file_name: &str) -> String {
        emit_refactor_ir_for_source_with_entry(source, file_name, None).unwrap()
    }

    fn emit_refactor_ir_for_source_with_entry(
        source: SourceFile,
        file_name: &str,
        entry_main_fqn: Option<&str>,
    ) -> Result<String, LlvmEmitError> {
        let _guard = test_lock();
        let temp = make_temp_dir();
        let out = temp.path().join(file_name);
        let (session, source_map, entry_source_id, lowered) = emit_args_for_source(source);
        pipeline::emit_production_llvm_artifact_to_file(
            &session,
            &source_map,
            entry_source_id,
            lowered,
            None,
            &out,
            entry_main_fqn,
            OptLevel::O0,
            LlvmArtifactKind::LlvmIr,
        )?;
        Ok(std::fs::read_to_string(out).unwrap())
    }

    fn ir_function_body(ir: &str, header: &str) -> String {
        let start = ir
            .find(header)
            .unwrap_or_else(|| panic!("IR should contain function header `{header}`:\n{ir}"));
        let rest = &ir[start..];
        let end = rest
            .find("\n}")
            .map(|index| index + 2)
            .unwrap_or(rest.len());
        rest[..end].to_string()
    }

    fn emit_args_for_source(
        source: SourceFile,
    ) -> (
        Session,
        SourceMap,
        crate::source::SourceId,
        crate::hir::LoweredHir,
    ) {
        let session = session();
        let lowered = crate::hir::lower_typed_for_dump(&session, &source).unwrap();
        let mut source_map = SourceMap::new();
        let entry_source_id = source_map.add_source_clone(&source);
        (session, source_map, entry_source_id, lowered)
    }

    fn sample_emit_args() -> (
        Session,
        SourceMap,
        crate::source::SourceId,
        crate::hir::LoweredHir,
    ) {
        emit_args_for_source(sample_source())
    }

    fn effectful_emit_args() -> (
        Session,
        SourceMap,
        crate::source::SourceId,
        crate::hir::LoweredHir,
    ) {
        emit_args_for_source(effectful_source())
    }

    #[test]
    fn refactor_mir_member_access_codegen() {
        let ir = emit_refactor_ir_for_source(member_codegen_source(), "member_access.ll");

        assert!(
            ir.contains("pass_mir_member_load"),
            "member read should be lowered through the canonical MIR helper:\n{ir}"
        );
    }

    #[test]
    fn refactor_mir_store_member_codegen() {
        let ir = emit_refactor_ir_for_source(member_codegen_source(), "store_member.ll");

        assert!(
            ir.contains("store i64 %pass_mir_iadd"),
            "member store should use the canonical MIR StoreMember helper:\n{ir}"
        );
    }

    #[test]
    fn refactor_llvm_composite_transport_contract_emits_layout_descriptor_globals() {
        let ir = emit_refactor_ir_for_source(
            composite_transport_contract_source(),
            "composite_transport_contract.ll",
        );

        assert!(
            ir.contains("scoop.runtime.ScoopCompositeTransportDescriptor"),
            "composite transport contract should define the shared runtime descriptor type\n{ir}"
        );
        assert!(
            ir.contains("@__scoop_composite_transport_desc__inline__sample_Named"),
            "struct composite transport should publish a normalized layout descriptor\n{ir}"
        );
        assert!(
            ir.contains("__gc_slots"),
            "traceable composite transport should publish an explicit GC slot map\n{ir}"
        );
        assert!(
            ir.contains("@scoop_composite_trace")
                && ir.contains("@scoop_composite_copy")
                && ir.contains("@scoop_composite_drop"),
            "descriptor should register trace/copy/drop runtime hook surface\n{ir}"
        );
    }

    #[test]
    fn refactor_llvm_value_boxing_transport() {
        let ir = emit_refactor_ir_for_source(
            value_boxing_transport_source(),
            "value_boxing_transport.ll",
        );

        assert!(
            ir.contains("@__scoop_composite_transport_desc__erased__sample_Named"),
            "struct -> Any boxing should consume the erased composite transport descriptor\n{ir}"
        );
        assert!(
            ir.contains("@__scoop_type_desc_mir_value_box__sample_Named"),
            "boxed struct carrier should publish a runtime type descriptor\n{ir}"
        );
        assert!(
            ir.contains("rt_alloc_mir_value_box"),
            "boxed struct carrier should allocate a GC-managed value box\n{ir}"
        );
        assert!(
            ir.contains("mir_value_box_payload_gep"),
            "boxed struct carrier should store the source payload in the value box\n{ir}"
        );
    }

    #[test]
    fn refactor_llvm_enum_payload_transport() {
        let ir = emit_refactor_ir_for_source(
            enum_payload_transport_source(),
            "enum_payload_transport.ll",
        );

        assert!(
            ir.contains("@__scoop_composite_transport_desc__inline__sample_Outer"),
            "enum constructors should publish an explicit enum payload layout descriptor\n{ir}"
        );
        assert!(
            ir.contains("@__scoop_type_desc_runtime__enum_boxed_payload__sample_Outer__UnitPair"),
            "boxed Unit+struct enum payload should publish a runtime type descriptor\n{ir}"
        );
        assert!(
            ir.contains("@__scoop_type_desc_runtime__enum_boxed_payload__sample_Outer__Nested"),
            "boxed nested enum/tuple payload should publish a runtime type descriptor\n{ir}"
        );
        assert!(
            ir.contains("rt_alloc_enum_boxed_payload"),
            "payload-bearing enum constructors should allocate GC-managed boxed payload objects\n{ir}"
        );
        assert!(
            ir.contains("@__scoop_composite_transport_desc__erased__sample_Outer"),
            "enum -> Any erasure should consume the erased composite transport descriptor\n{ir}"
        );
        assert!(
            ir.contains("@__scoop_type_desc_mir_value_box__sample_Outer"),
            "enum -> Any erasure should use the CG-T04b value box carrier\n{ir}"
        );
        assert!(
            ir.contains("when_payload_field"),
            "when/pattern extraction should project boxed enum payload fields\n{ir}"
        );
    }

    #[test]
    fn refactor_llvm_array_composite_transport() {
        let ir = emit_refactor_ir_for_source(
            array_composite_transport_source(),
            "array_composite_transport.ll",
        );

        assert!(
            ir.contains("scoop.runtime.ScoopCompositeTransportDescriptor"),
            "array composite transport should consume shared layout descriptors\n{ir}"
        );
        assert!(
            ir.contains("@scoop_array_builder_push_composite"),
            "composite array literal elements should use descriptor-backed builder push\n{ir}"
        );
        assert!(
            ir.contains("@scoop_array_builder_build_array_composite")
                && ir.contains("@scoop_array_builder_build_mutable_array_composite"),
            "composite array build should pass the element descriptor to runtime\n{ir}"
        );
        assert!(
            ir.contains("@scoop_array_get_composite") && ir.contains("@scoop_array_set_composite"),
            "composite array get/set should copy through descriptor-backed runtime hooks\n{ir}"
        );
    }

    #[test]
    fn refactor_llvm_closure_env_transport() {
        let ir =
            emit_refactor_ir_for_source(closure_env_transport_source(), "closure_env_transport.ll");

        assert!(
            ir.contains("__scoop_composite_transport_desc__boxed") && ir.contains("ClosureEnv"),
            "closure env lowering should consume the boxed composite transport descriptor\n{ir}"
        );
        assert!(
            ir.contains("__scoop_type_desc_mir_closure_env__"),
            "closure env heap object should publish a runtime type descriptor\n{ir}"
        );
        assert!(
            ir.contains("__scoop_type_desc_mir_capture_box__sample_Point"),
            "mutable struct capture should allocate a typed capture box descriptor\n{ir}"
        );
        assert!(
            ir.contains("pass_mir_closure_env_field_gep")
                && ir.contains("pass_mir_capture_box_set_field_gep"),
            "closure allocation/invoke should store env fields and mutate through capture boxes\n{ir}"
        );
        assert!(
            ir.contains("__gc_slots"),
            "traceable closure captures should publish GC slot maps\n{ir}"
        );
    }

    #[test]
    fn refactor_llvm_cross_thread_resume_payload_transport() {
        let ir = emit_refactor_ir_for_source(
            cross_thread_resume_payload_transport_source(),
            "cross_thread_resume_payload_transport.ll",
        );

        assert!(
            ir.contains("@scoop_thread_spawn_join_resume_transport"),
            "cross-thread resume should call the typed transport runtime helper\n{ir}"
        );
        assert!(
            ir.contains("__scoop_refactor_thread_resume_transport__"),
            "cross-thread resume should generate a typed surface-resume thunk\n{ir}"
        );
        assert!(
            ir.contains("__scoop_composite_transport_desc__boxed__sample_Point")
                && ir.contains("__gc_slots"),
            "composite resume payload should pass a descriptor with GC slot metadata\n{ir}"
        );
        assert!(
            ir.contains("%refactor_thread_resume_payload")
                && ir.contains("@scoop_thread_spawn_join_resume_transport"),
            "composite resume payload should be passed through an explicit carrier pointer\n{ir}"
        );
    }

    #[test]
    fn refactor_llvm_function_abi_entry_shells_use_refactor_direct_entry() {
        let ir = emit_refactor_ir_for_source_with_entry(
            unhandled_outward_entry_source(),
            "function_abi.ll",
            Some("sample.effectEntry"),
        )
        .unwrap();

        let dynamic = ir_function_body(
            &ir,
            "define %scoop.refactor.Step__sample_effectEntry @__scoop_refactor_dynamic_invoke__sample_effectEntry(",
        );
        assert!(
            dynamic.contains(
                "call %scoop.refactor.Step__sample_effectEntry @__scoop_refactor_direct_invoke__sample_effectEntry("
            ),
            "dynamic entry should forward through the published refactor direct entry:\n{dynamic}"
        );

        let main = ir_function_body(&ir, "define i32 @main(");
        assert!(
            main.contains(
                "call %scoop.refactor.Step__sample_effectEntry @__scoop_refactor_direct_invoke__sample_effectEntry("
            ),
            "C main wrapper should call the refactor direct entry, not the legacy function ABI:\n{main}"
        );
        assert!(
            !main.contains("@sample.effectEntry(") && !main.contains("@\"sample.effectEntry\""),
            "refactor main wrapper must not call the legacy callable ABI:\n{main}"
        );
    }

    #[test]
    fn refactor_llvm_main_wrapper_routes_unhandled_outward_to_exit_code() {
        let ir = emit_refactor_ir_for_source_with_entry(
            unhandled_outward_entry_source(),
            "main_unhandled.ll",
            Some("sample.effectEntry"),
        )
        .unwrap();
        let main = ir_function_body(&ir, "define i32 @main(");
        let unhandled = main
            .split("refactor_main_unhandled:")
            .nth(1)
            .and_then(|tail| tail.split("refactor_main_done:").next())
            .expect("main wrapper should contain an unhandled Step branch");

        assert!(
            unhandled.contains("br label %refactor_main_done"),
            "unhandled outward case should rejoin the explicit exit path:\n{main}"
        );
        assert!(
            !unhandled.contains("unreachable"),
            "published outward cases must not be represented as unreachable at the main wrapper:\n{main}"
        );
    }

    #[test]
    fn refactor_llvm_main_wrapper_passes_array_string_argv_to_plain_entry() {
        let ir = emit_refactor_ir_for_source_with_entry(
            array_string_main_source(),
            "main_argv.ll",
            None,
        )
        .expect("refactor argv ABI should lower through the plain entry ABI");
        let main = ir_function_body(&ir, "define i32 @main(");

        assert!(
            main.contains("@scoop_entry_argv_array") && main.contains("@sample.main"),
            "main wrapper should build argv array and pass it to the refactor plain entry:\n{main}"
        );
    }

    #[test]
    fn refactor_llvm_runtime_type_primitives() {
        let ir = emit_refactor_ir_for_source(
            runtime_type_primitives_source(),
            "runtime_type_primitives.ll",
        );

        assert!(
            ir.contains("mir_typecheck_not"),
            "`!is` should lower as a runtime type test plus boolean negation:\n{ir}"
        );
        assert!(
            ir.contains("mir_asq_value"),
            "`as?` should construct an Option<T> value in refactor LLVM:\n{ir}"
        );
        assert!(
            ir.contains("isa_iface") || ir.contains("isa_loop"),
            "runtime type tests should use descriptor/itable matching helpers:\n{ir}"
        );
        assert!(
            ir.contains("@sample.classifyValue"),
            "pattern `is Type` should codegen through MIR pattern metadata, including static folds:\n{ir}"
        );
    }

    #[test]
    fn refactor_llvm_codegen_stage_output_is_constructible() {
        let _guard = test_lock();
        let (session, source_map, entry_source_id, lowered) = sample_emit_args();
        let input = LlvmCodegenStageInput::new(
            lowered,
            None,
            source_map,
            entry_source_id,
            None,
            OptLevel::O0,
        );
        let stage_output = super::run(&session, input).unwrap();

        assert_eq!(stage_output.opt_level(), OptLevel::O0);
        assert!(
            stage_output
                .effect_lowered_stage_output()
                .program()
                .callable("sample.main")
                .is_some()
        );
        assert!(
            stage_output
                .hir_compat_scaffold()
                .materialized_pass_view()
                .is_none(),
            "refactor LLVM stage 的 HIR scaffold 不应再携带旧 production pass-view 入口"
        );
        assert!(
            stage_output
                .abi_visibility_effect_lowered_stage_output()
                .is_none(),
            "未显式提供 ABI visibility handoff 时，不应伪造第二份 stage 输出"
        );

        let context = Context::create();
        let module = build_main_module_from_stage_output(
            stage_output.source_map(),
            stage_output.entry_source_id(),
            &context,
            crate::llvm::StageEmitInput::new(
                stage_output.hir_compat_scaffold(),
                stage_output.effect_lowered_stage_output(),
                stage_output.abi_visibility_effect_lowered_stage_output(),
            ),
            stage_output.entry_main_fqn(),
        )
        .unwrap();
        let ir = module.print_to_string().to_string();
        assert!(ir.contains("define i32 @main("));
    }

    #[test]
    fn single_pipeline_llvm_codegen_stage_build_entry_uses_stage() {
        let _guard = test_lock();
        let _stage_run_guard = enable_test_stage_run_counting();
        let temp = make_temp_dir();
        let out = temp.path().join("single.ll");

        let (session, source_map, entry_source_id, lowered) = sample_emit_args();
        pipeline::emit_production_llvm_artifact_to_file(
            &session,
            &source_map,
            entry_source_id,
            lowered,
            None,
            &out,
            None,
            OptLevel::O0,
            LlvmArtifactKind::LlvmIr,
        )
        .unwrap();
        assert_eq!(test_stage_run_count(), 1);
        assert!(out.is_file());
    }

    #[test]
    fn default_minimal_main_ir_helper_uses_stage() {
        let _guard = test_lock();
        let _stage_run_guard = enable_test_stage_run_counting();

        let session = session();
        let source = effectful_source();
        let ir = crate::llvm::emit_minimal_main_ir(&session, &source)
            .expect("默认单文件 IR helper 应经 refactor LLVM stage 成功 lower effectful main");

        assert!(ir.contains("define i32 @main("));
        assert_eq!(test_stage_run_count(), 1);
    }

    #[test]
    fn single_pipeline_single_file_artifact_entry_uses_stage_for_all_artifacts() {
        let _guard = test_lock();
        let _stage_run_guard = enable_test_stage_run_counting();
        let temp = make_temp_dir();
        let session = session();
        let source = effectful_source();
        let artifacts = [
            (LlvmArtifactKind::LlvmIr, PathBuf::from("single.ll")),
            (LlvmArtifactKind::Object, PathBuf::from("single.o")),
            (LlvmArtifactKind::Asm, PathBuf::from("single.s")),
        ];

        for (artifact, rel) in artifacts {
            let out = temp.path().join(rel);
            pipeline::emit_virtual_cone_llvm_artifact_to_file(&session, &source, &out, artifact)
                .unwrap();
            let size = std::fs::metadata(&out).unwrap().len();
            assert!(size > 0, "产物不应为空：{}", out.display());
        }

        assert_eq!(test_stage_run_count(), 3);
    }

    #[test]
    fn refactor_llvm_codegen_stage_shares_same_stage_entry_for_ir_obj_and_asm() {
        let _guard = test_lock();
        let _stage_run_guard = enable_test_stage_run_counting();
        let temp = make_temp_dir();
        let artifacts = [
            (LlvmArtifactKind::LlvmIr, PathBuf::from("stage.ll")),
            (LlvmArtifactKind::Object, PathBuf::from("stage.o")),
            (LlvmArtifactKind::Asm, PathBuf::from("stage.s")),
        ];

        for (artifact, rel) in artifacts {
            let out = temp.path().join(rel);
            let (session, source_map, entry_source_id, lowered) = sample_emit_args();
            pipeline::emit_production_llvm_artifact_to_file(
                &session,
                &source_map,
                entry_source_id,
                lowered,
                None,
                &out,
                None,
                OptLevel::O0,
                artifact,
            )
            .unwrap();
            let size = std::fs::metadata(&out).unwrap().len();
            assert!(size > 0, "产物不应为空：{}", out.display());
        }

        assert_eq!(test_stage_run_count(), 3);
    }

    #[test]
    fn refactor_llvm_backend_gate_smoke_lowers_effectful_handle_body_without_legacy() {
        let _guard = test_lock();
        let _stage_run_guard = enable_test_stage_run_counting();
        let temp = make_temp_dir();
        let out = temp.path().join("effect.ll");

        let (session, source_map, entry_source_id, lowered) = effectful_emit_args();
        pipeline::emit_production_llvm_artifact_to_file(
            &session,
            &source_map,
            entry_source_id,
            lowered,
            None,
            &out,
            None,
            OptLevel::O0,
            LlvmArtifactKind::LlvmIr,
        )
        .expect("effectful refactor LLVM path 应由 clean stage lowering 成功生成 IR");

        assert_eq!(test_stage_run_count(), 1);
        let ir = std::fs::read_to_string(out).unwrap();
        assert!(ir.contains("scoop.refactor.Step"));
        assert!(ir.contains("call void @scoop_runtime_init()"));
    }
}
