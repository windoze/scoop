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
/// - 该 scaffold 明确不再携带 `materialized_mir/pass_view`，避免 lowering 路径再回落到旧的
///   `materialized_lowered_hir` emit helper；
/// - `.ll/.o/.s` 三类产物都必须共用这份 handoff，再进入新的 emit API。
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
                    "LLVM stage 找不到入口源文件（source_id={})",
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
                "LLVM literal precheck 找不到 source_id={}",
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
                "LLVM literal precheck 找不到 source_id={}",
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
        message: format!("LLVM stage `{stage}` 失败：{error}"),
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
    use std::collections::BTreeSet;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    use inkwell::context::Context;
    use object::{BinaryFormat, Object, ObjectSymbol, SymbolKind, SymbolScope};

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
            "scoopc_llvm_codegen_stage_{}_{}_{}",
            std::process::id(),
            unique,
            ordinal
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempDirGuard(dir)
    }

    fn sample_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/llvm_codegen_stage_fixture.scoop",
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
            "<mem>/llvm_codegen_stage_effectful_fixture.scoop",
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
            "<mem>/mir_member_codegen_fixture.scoop",
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
            "<mem>/main_unhandled_outward_fixture.scoop",
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
            "<mem>/main_array_string_fixture.scoop",
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
            "<mem>/runtime_type_primitives_fixture.scoop",
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

fun classifyImpossible(x: Int): Int {
    return when (x) {
        is String -> 7
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
            "<mem>/composite_transport_contract_fixture.scoop",
            r#"
package sample

import scoop.core.*
import scoop.runtime.test.*

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
            "<mem>/value_boxing_transport_fixture.scoop",
            r#"
package sample

import scoop.core.*
import scoop.runtime.test.*

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
            "<mem>/enum_payload_transport_fixture.scoop",
            r#"
package sample

import scoop.core.*
import scoop.runtime.test.*

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
            "<mem>/array_composite_transport_fixture.scoop",
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
            "<mem>/closure_env_transport_fixture.scoop",
            r#"
package sample

import scoop.core.*
import scoop.runtime.test.*

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

    fn closure_mutable_capture_per_call_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/closure_mutable_capture_per_call.scoop",
            r#"
package sample

import scoop.core.*

fun callTwice(f: () -> Int): Int {
    val a: Int = f()
    val b: Int = f()
    return a * 100 + b * 10
}

fun main(): Int {
    var x: Int = 0
    val f: () -> Int = {
        x = x + 1
        x
    }
    return callTwice(f) + x
}
"#,
        )
    }

    fn atomic_ref_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/atomic_ref_fixture.scoop",
            r#"
package sample

import scoop.core.*

class Node(var value: Int)

fun main(): Int {
    val first: Node = Node(1)
    val second: Node = Node(2)
    val cell: Atomic<Node> = Atomic(first)
    val loaded: Node = cell.load()
    cell.store(second)
    val swapped: Bool = cell.cas(second, first)
    if (swapped) {
        return loaded.value + cell.load().value
    }
    return 0
}
"#,
        )
    }

    fn emit_ir_for_source(source: SourceFile, file_name: &str) -> String {
        emit_ir_for_source_with_entry(source, file_name, None).unwrap()
    }

    fn emit_ir_for_source_with_entry(
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

    fn emit_object_external_symbols_for_source_with_entry(
        source: SourceFile,
        file_name: &str,
        entry_main_fqn: Option<&str>,
    ) -> Result<Vec<String>, LlvmEmitError> {
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
            LlvmArtifactKind::Object,
        )?;

        let bytes = std::fs::read(&out).unwrap();
        let obj = object::File::parse(&*bytes).expect("generated object should parse");
        Ok(object_external_symbols(&obj))
    }

    fn object_external_symbols(obj: &object::File<'_>) -> Vec<String> {
        let mut symbols = Vec::new();
        for symbol in obj.symbols() {
            if matches!(
                symbol.kind(),
                SymbolKind::Section | SymbolKind::File | SymbolKind::Label
            ) {
                continue;
            }
            let is_external = symbol.is_undefined()
                || matches!(symbol.scope(), SymbolScope::Linkage | SymbolScope::Dynamic);
            if !is_external {
                continue;
            }
            let Ok(raw_name) = symbol.name() else {
                continue;
            };
            if raw_name.is_empty() {
                continue;
            }
            symbols.push(normalize_object_symbol_name(raw_name, obj.format()).to_string());
        }
        symbols.sort();
        symbols.dedup();
        symbols
    }

    fn normalize_object_symbol_name(name: &str, format: BinaryFormat) -> &str {
        if matches!(format, BinaryFormat::MachO) {
            name.strip_prefix('_').unwrap_or(name)
        } else {
            name
        }
    }

    #[test]
    fn normalize_object_symbol_name_only_strips_macho_abi_prefix() {
        assert_eq!(
            normalize_object_symbol_name(
                "___scoop_abi0_fun__sample_helper__hdeadbeef",
                BinaryFormat::MachO
            ),
            "__scoop_abi0_fun__sample_helper__hdeadbeef"
        );
        assert_eq!(
            normalize_object_symbol_name(
                "__scoop_abi0_fun__sample_helper__hdeadbeef",
                BinaryFormat::Elf
            ),
            "__scoop_abi0_fun__sample_helper__hdeadbeef"
        );
    }

    fn write_source_under_root(
        root: &std::path::Path,
        relative_path: &str,
        text: &str,
    ) -> SourceFile {
        let path = root.join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, text).unwrap();
        SourceFile::load(&path).unwrap()
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

    fn ir_function_matching<'a, F>(ir: &'a str, description: &str, predicate: F) -> &'a str
    where
        F: Fn(&str, &str) -> bool,
    {
        for chunk in ir.split("\ndefine ").skip(1) {
            let end = chunk.find("\n}").expect("expected end of function body") + 2;
            let function = &chunk[..end];
            let header = function.lines().next().expect("expected function header");
            if predicate(header, function) {
                return function;
            }
        }
        panic!("IR should contain function matching `{description}`:\n{ir}");
    }

    fn ir_function_symbol_name(function_ir: &str) -> &str {
        let header = function_ir
            .lines()
            .next()
            .expect("expected function header");
        let symbol = header
            .split_once('@')
            .map(|(_, rest)| rest)
            .expect("expected function symbol name");
        if let Some(symbol) = symbol.strip_prefix('"') {
            symbol
                .split_once('"')
                .map(|(name, _)| name)
                .expect("expected closing quote in function symbol")
        } else {
            symbol
                .split_once('(')
                .map(|(name, _)| name)
                .expect("expected opening paren in function symbol")
        }
    }

    fn ir_call_target_symbol(line: &str) -> Option<&str> {
        let after_call = if let Some(idx) = line.find(" call ") {
            &line[idx + " call ".len()..]
        } else if let Some(idx) = line.find(" invoke ") {
            &line[idx + " invoke ".len()..]
        } else {
            return None;
        };
        let symbol = after_call.split_once('@')?.1;
        if let Some(symbol) = symbol.strip_prefix('"') {
            Some(
                symbol
                    .split_once('"')
                    .map(|(name, _)| name)
                    .expect("expected closing quote in call target symbol"),
            )
        } else {
            let end = symbol.find(['(', ' ', ',']).unwrap_or(symbol.len());
            Some(&symbol[..end])
        }
    }

    fn ir_defined_function_symbols(ir: &str) -> Vec<&str> {
        ir.split("\ndefine ")
            .skip(1)
            .map(|chunk| {
                let end = chunk.find("\n}").expect("expected end of function body") + 2;
                let function = &chunk[..end];
                ir_function_symbol_name(function)
            })
            .collect()
    }

    fn ir_function_defined_call_targets(ir: &str, function_ir: &str) -> Vec<String> {
        let defined = ir_defined_function_symbols(ir);
        function_ir
            .lines()
            .filter_map(ir_call_target_symbol)
            .filter(|target| {
                defined
                    .iter()
                    .any(|defined_symbol| defined_symbol == target)
            })
            .map(str::to_owned)
            .collect()
    }

    fn ir_line_mentions_symbol(line: &str, symbol_name: &str) -> bool {
        line.contains(&format!("@{symbol_name}")) || line.contains(&format!("@\"{symbol_name}\""))
    }

    fn ir_global_definition_matching<'a, F>(ir: &'a str, description: &str, predicate: F) -> &'a str
    where
        F: Fn(&str) -> bool,
    {
        ir.lines()
            .find(|line| line.starts_with('@') && line.contains(" = ") && predicate(line))
            .unwrap_or_else(|| panic!("IR should contain global matching `{description}`:\n{ir}"))
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
        let context =
            crate::frontend::prepare_virtual_cone_context_with_options(source, session.options())
                .unwrap();
        let front = crate::frontend::run_project_frontend(&session, context).unwrap();
        let lowered = crate::frontend::lower_hir_for_codegen_with_request_root_mode(
            &session,
            &front,
            OptLevel::O0,
            crate::frontend::MirRequestRootMode::RequestSources,
        )
        .unwrap();
        let (source_map, entry_source_id) =
            crate::frontend::build_source_map(&session, front.input());
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
    fn mir_member_access_codegen() {
        let ir = emit_ir_for_source(member_codegen_source(), "member_access.ll");

        assert!(
            ir.contains("pass_mir_member_load"),
            "member read should be lowered through the canonical MIR helper:\n{ir}"
        );
    }

    #[test]
    fn mir_store_member_codegen() {
        let ir = emit_ir_for_source(member_codegen_source(), "store_member.ll");

        assert!(
            ir.contains("store i64 %intrinsic_iadd"),
            "member store should use the canonical MIR StoreMember helper:\n{ir}"
        );
    }

    #[test]
    fn llvm_composite_transport_contract_emits_layout_descriptor_globals() {
        let ir = emit_ir_for_source(
            composite_transport_contract_source(),
            "composite_transport_contract.ll",
        );

        assert!(
            ir.contains("scoop.runtime.ScoopCompositeTransportDescriptor"),
            "composite transport contract should define the shared runtime descriptor type\n{ir}"
        );
        let descriptor = ir_global_definition_matching(
            &ir,
            "traceable composite transport descriptor global",
            |line| {
                line.contains("%scoop.runtime.ScoopCompositeTransportDescriptor")
                    && line.contains("__gc_slots")
                    && line.contains("@scoop_composite_trace")
                    && line.contains("@scoop_composite_copy")
                    && line.contains("@scoop_composite_drop")
            },
        );
        assert!(
            descriptor.contains("__gc_slots"),
            "traceable composite transport should publish an explicit GC slot map\n{descriptor}"
        );
        assert!(
            descriptor.starts_with("@__scoop_priv0__composite_transport_desc__h"),
            "composite transport descriptor global naming 应改走 stable private namespace\n{descriptor}"
        );
        assert!(
            descriptor.contains("@scoop_composite_trace")
                && descriptor.contains("@scoop_composite_copy")
                && descriptor.contains("@scoop_composite_drop"),
            "descriptor should register trace/copy/drop runtime hook surface\n{descriptor}"
        );
    }

    #[test]
    fn llvm_value_boxing_transport() {
        let ir = emit_ir_for_source(value_boxing_transport_source(), "value_boxing_transport.ll");

        let composite_descriptor = ir_global_definition_matching(
            &ir,
            "descriptor-backed composite transport global used during value boxing",
            |line| {
                line.contains("%scoop.runtime.ScoopCompositeTransportDescriptor")
                    && line.contains("@scoop_composite_copy")
                    && line.contains("@scoop_composite_drop")
            },
        );
        assert!(
            composite_descriptor.contains("@scoop_composite_copy")
                && composite_descriptor.contains("@scoop_composite_drop"),
            "struct -> Any boxing should consume a descriptor-backed composite transport contract\n{composite_descriptor}"
        );
        assert!(
            ir.lines().any(|line| {
                line.starts_with("%scoop.lowered.MirValueBox__h")
                    && line
                        .contains(" = type { %scoop.runtime.ScoopGcObjectHeader, %sample.Named }")
            }) && ir.contains("@__scoop_priv0__mir_value_box_type_desc__h")
                && ir.contains("rt_alloc_mir_value_box"),
            "boxed struct carrier should materialize a concrete value-box object type and allocate it through typed alloc\n{ir}"
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
    fn llvm_enum_payload_transport() {
        let ir = emit_ir_for_source(enum_payload_transport_source(), "enum_payload_transport.ll");

        let payload_descriptor =
            ir_global_definition_matching(&ir, "enum payload layout descriptor global", |line| {
                line.contains("%scoop.runtime.ScoopCompositeTransportDescriptor")
                    && line.contains("__gc_slots")
                    && line.contains("@scoop_composite_trace")
            });
        assert!(
            payload_descriptor.contains("__gc_slots"),
            "enum constructors should publish an explicit payload layout descriptor with GC slot metadata\n{payload_descriptor}"
        );
        assert!(
            payload_descriptor.starts_with("@__scoop_priv0__composite_transport_desc__h"),
            "enum payload composite descriptor 应改走 stable private namespace\n{payload_descriptor}"
        );
        assert!(
            ir.contains("rt_alloc_enum_boxed_payload"),
            "payload-bearing enum constructors should allocate GC-managed boxed payload objects\n{ir}"
        );
        assert!(
            ir.matches("rt_alloc_enum_boxed_payload").count() >= 2
                && ir.matches("enum_boxed_payload_gep").count() >= 2,
            "boxed Unit+struct / nested tuple payload 都应通过 descriptor-backed typed alloc 发布 boxed variant path\n{ir}"
        );
        assert!(
            ir.lines().any(|line| {
                line.starts_with("%scoop.lowered.MirValueBox__h")
                    && line
                        .contains(" = type { %scoop.runtime.ScoopGcObjectHeader, %sample.Outer }")
            }) && ir.contains("@__scoop_priv0__mir_value_box_type_desc__h")
                && ir.contains("@__scoop_priv0__enum_boxed_payload_type_desc__h")
                && ir.contains("rt_alloc_mir_value_box")
                && ir.contains("mir_value_box_payload_gep"),
            "enum -> Any 擦除应继续走 descriptor-backed value box carrier，而不是锁死当前 value-box symbol\n{ir}"
        );
        assert!(
            ir.contains("when_payload_field"),
            "when/pattern extraction should project boxed enum payload fields\n{ir}"
        );
    }

    #[test]
    fn llvm_array_composite_transport() {
        let ir = emit_ir_for_source(
            array_composite_transport_source(),
            "array_composite_transport.ll",
        );

        assert!(
            ir.contains("scoop.runtime.ScoopCompositeTransportDescriptor"),
            "array composite transport should consume shared layout descriptors\n{ir}"
        );
        assert!(
            ir.contains("@scoop_mutable_array_push_composite"),
            "composite array literal elements should use descriptor-backed MutableArray.push\n{ir}"
        );
        assert!(
            ir.contains("@scoop_mutable_array_new") && ir.contains("@scoop_mutable_array_freeze"),
            "composite array literal build should allocate MutableArray and freeze Array results\n{ir}"
        );
        assert!(
            ir.contains("array_get_load")
                && ir.contains("@scoop_composite_drop")
                && ir.contains("@scoop_composite_copy")
                && ir.contains("@scoop_gc_write_barrier"),
            "composite array get/set should lower as direct load/store + descriptor-backed copy/write-barrier collaboration\n{ir}"
        );
        assert!(
            !ir.contains("@scoop_array_get_composite")
                && !ir.contains("@scoop_array_set_composite"),
            "composite array get/set should no longer depend on array-specific runtime helpers\n{ir}"
        );
    }

    #[test]
    fn llvm_closure_env_transport() {
        let ir = emit_ir_for_source(closure_env_transport_source(), "closure_env_transport.ll");

        let closure_env_descriptor = ir_global_definition_matching(
            &ir,
            "closure env composite transport descriptor global",
            |line| {
                line.contains("%scoop.runtime.ScoopCompositeTransportDescriptor")
                    && line.contains("__gc_slots")
                    && line.contains("@scoop_composite_trace")
            },
        );
        assert!(
            closure_env_descriptor.contains("__gc_slots"),
            "closure env lowering should consume a boxed composite transport descriptor with GC slot metadata\n{closure_env_descriptor}"
        );
        assert!(
            closure_env_descriptor.starts_with("@__scoop_priv0__composite_transport_desc__h"),
            "closure env composite descriptor 应改走 stable private namespace\n{closure_env_descriptor}"
        );
        assert!(
            ir.contains("rt_alloc_pass_mir_closure_env"),
            "closure env heap object 应继续通过 typed alloc 发布 descriptor-backed runtime object，而不是锁死当前 closure-env descriptor symbol\n{ir}"
        );
        let legacy_heap_alloc_marker = ["rt_alloc_pass_mir_", "capture", "_", "box"].concat();
        let legacy_descriptor_marker = ["Mir", "Capture", "Box"].concat();
        let legacy_snake_marker = ["capture", "_", "box"].concat();
        assert!(
            !ir.contains(&legacy_heap_alloc_marker)
                && !ir.contains(&legacy_descriptor_marker)
                && !ir.contains(&legacy_snake_marker),
            "closure capture lowering should not emit legacy mutable-capture allocation/type descriptors\n{ir}"
        );
        assert!(
            ir.contains("pass_mir_closure_env_field_gep"),
            "closure allocation should still store captured values into env fields\n{ir}"
        );
    }

    #[test]
    fn llvm_closure_mutable_capture_reloads_env_into_per_call_local() {
        let ir = emit_ir_for_source(
            closure_mutable_capture_per_call_source(),
            "closure_mutable_capture_per_call.ll",
        );

        let closure_body =
            ir_function_matching(&ir, "mutable capture closure body", |_header, function| {
                function.contains("pass_mir_closure_env_field_load")
                    && function.contains("intrinsic_iadd")
            });
        assert!(
            closure_body.contains("pass_mir_closure_env_field_load"),
            "closure body should reload captured x from env at each invocation\n{closure_body}"
        );
        assert!(
            closure_body.contains("pass_mir_tuple_extract")
                || closure_body.contains("pass_mir_closure_env_tuple_insert"),
            "closure body should move the env snapshot through ordinary local tuple storage\n{closure_body}"
        );
        assert!(
            !closure_body.lines().any(|line| {
                line.contains("store ") && line.contains("pass_mir_closure_env_field_gep")
            }),
            "closure body must not write rebinding back into the env object\n{closure_body}"
        );
    }

    #[test]
    fn llvm_atomic_ref_uses_atomic_instructions_and_gc_barrier() {
        let ir = emit_ir_for_source(atomic_ref_source(), "atomic_ref.ll");

        assert!(
            ir.contains("load atomic ptr addrspace(1)")
                && ir.contains("store atomic ptr addrspace(1)")
                && ir.contains("cmpxchg ptr addrspace(1)"),
            "Atomic<T: AnyRef> should lower load/store/cas to pointer atomic instructions\n{ir}"
        );
        let cas_function =
            ir_function_matching(&ir, "atomic ref cas function", |_header, function| {
                function.contains("cmpxchg ptr addrspace(1)")
            });
        assert!(
            cas_function.contains("atomic_ref_cas_barrier")
                && cas_function.contains("@scoop_gc_write_barrier"),
            "atomic-ref CAS must run the GC write barrier only on the success path\n{cas_function}"
        );
        let store_function =
            ir_function_matching(&ir, "atomic ref store function", |_header, function| {
                function.contains("store atomic ptr addrspace(1)")
            });
        assert!(
            store_function.contains("@scoop_gc_write_barrier"),
            "atomic-ref store must invoke the GC write barrier protocol\n{store_function}"
        );
    }

    #[test]
    fn llvm_function_abi_entry_shells_use_direct_entry() {
        let ir = emit_ir_for_source_with_entry(
            unhandled_outward_entry_source(),
            "function_abi.ll",
            Some("sample.effectEntry"),
        )
        .unwrap();

        let main = ir_function_body(&ir, "define i32 @main(");
        let main_defined_calls = ir_function_defined_call_targets(&ir, &main);
        assert!(
            main_defined_calls.len() == 1,
            "C main wrapper should forward to exactly one defined entry shell instead of a legacy callable ABI: {:?}\n{main}",
            main_defined_calls
        );
        let direct_entry_symbol = main_defined_calls[0].clone();
        let _direct_entry = ir_function_matching(
            &ir,
            "direct entry shell called by main",
            |header, function| {
                !header.contains("@main(")
                    && ir_function_symbol_name(function) == direct_entry_symbol.as_str()
            },
        );
        let dynamic = ir_function_matching(
            &ir,
            "dynamic entry shell forwarding to direct entry",
            |header, function| {
                if header.contains("@main(")
                    || ir_function_symbol_name(function) == direct_entry_symbol.as_str()
                {
                    return false;
                }
                let calls = ir_function_defined_call_targets(&ir, function);
                calls.len() == 1 && calls[0] == direct_entry_symbol
            },
        );
        let dynamic_calls = ir_function_defined_call_targets(&ir, dynamic);
        assert!(
            dynamic_calls.len() == 1 && dynamic_calls[0] == direct_entry_symbol,
            "dynamic entry should forward through the published direct entry:\n{dynamic}"
        );

        let main_calls = ir_function_defined_call_targets(&ir, &main);
        assert!(
            main_calls.len() == 1 && main_calls[0] == direct_entry_symbol,
            "C main wrapper should call the direct entry, not the legacy function ABI:\n{main}"
        );
    }

    #[test]
    fn llvm_main_wrapper_routes_unhandled_outward_to_exit_code() {
        let ir = emit_ir_for_source_with_entry(
            unhandled_outward_entry_source(),
            "main_unhandled.ll",
            Some("sample.effectEntry"),
        )
        .unwrap();
        let main = ir_function_body(&ir, "define i32 @main(");
        let unhandled = main
            .split("main_unhandled:")
            .nth(1)
            .and_then(|tail| tail.split("main_done:").next())
            .expect("main wrapper should contain an unhandled Step branch");

        assert!(
            unhandled.contains("br label %main_done"),
            "unhandled outward case should rejoin the explicit exit path:\n{main}"
        );
        assert!(
            !unhandled.contains("unreachable"),
            "published outward cases must not be represented as unreachable at the main wrapper:\n{main}"
        );
    }

    #[test]
    fn llvm_main_wrapper_passes_array_string_argv_to_plain_entry() {
        let ir = emit_ir_for_source_with_entry(array_string_main_source(), "main_argv.ll", None)
            .expect("argv ABI should lower through the plain entry ABI");
        let main = ir_function_body(&ir, "define i32 @main(");
        let main_defined_calls = ir_function_defined_call_targets(&ir, &main);
        assert_eq!(
            main_defined_calls.len(),
            1,
            "main wrapper should forward to exactly one defined plain entry shell: {:?}\n{main}",
            main_defined_calls
        );
        let plain_entry_symbol = main_defined_calls[0].clone();
        let plain_entry = ir_function_matching(
            &ir,
            "plain argv entry shell called by C main wrapper",
            |header, function| {
                !header.contains("@main(")
                    && ir_function_symbol_name(function) == plain_entry_symbol.as_str()
                    && header.contains("ptr addrspace(1)")
                    && !function.contains("switch i32 %step_tag")
            },
        );

        assert!(
            main.contains("@scoop_entry_argv_array")
                && ir_function_symbol_name(plain_entry) == plain_entry_symbol,
            "main wrapper should build argv array and pass it to the plain entry:\n{main}"
        );
    }

    #[test]
    fn llvm_runtime_type_primitives() {
        let ir = emit_ir_for_source(
            runtime_type_primitives_source(),
            "runtime_type_primitives.ll",
        );

        assert!(
            ir.contains("mir_typecheck_not"),
            "`!is` should lower as a runtime type test plus boolean negation:\n{ir}"
        );
        assert!(
            ir.contains("mir_asq_value"),
            "`as?` should construct an Option<T> value in LLVM:\n{ir}"
        );
        assert!(
            ir.contains("isa_iface") || ir.contains("isa_loop"),
            "runtime type tests should use descriptor/itable matching helpers:\n{ir}"
        );
        let classify_ir = ir_function_matching(
            &ir,
            "type-pattern helper with statically folded branch condition",
            |header, function| {
                !header.contains("@main(")
                    && header.contains("(i64")
                    && function.contains("br i1 true")
                    && function.contains("phi i64 [ 7,")
                    && function.contains("[ 9,")
            },
        );
        assert!(
            classify_ir.contains("br i1 true")
                && classify_ir.contains("phi i64 [ 7,")
                && classify_ir.contains("[ 9,")
                && !classify_ir.contains("isa_iface")
                && !classify_ir.contains("isa_loop"),
            "pattern `is Type` should codegen through MIR pattern metadata, including static folds，而不是依赖当前 callable symbol 文本:\n{classify_ir}"
        );
        let impossible_ir = ir_function_matching(
            &ir,
            "type-pattern helper with statically folded false branch condition",
            |header, function| {
                !header.contains("@main(")
                    && header.contains("(i64")
                    && function.contains("br i1 false")
                    && function.contains("phi i64 [ 7,")
                    && function.contains("[ 9,")
            },
        );
        assert!(
            impossible_ir.contains("br i1 false")
                && impossible_ir.contains("phi i64 [ 7,")
                && impossible_ir.contains("[ 9,")
                && !impossible_ir.contains("isa_iface")
                && !impossible_ir.contains("isa_loop"),
            "disjoint value/ref pattern `is Type` 应静态折叠为 false，而不是继续走 runtime type test:\n{impossible_ir}"
        );
    }

    #[test]
    fn llvm_overloaded_source_level_callables_publish_distinct_abi_symbols() {
        let ir = emit_ir_for_source(
            SourceFile::new_virtual(
                "<mem>/overload_export_fixture.scoop",
                r#"
package sample

fun pick(x: Int): Int {
    return x
}

fun pick(x: Bool): Int {
    return 2
}

fun main(): Int {
    return pick(1) + pick(true)
}
"#,
            ),
            "overload_export.ll",
        );

        let overload_symbols = ir_defined_function_symbols(&ir)
            .into_iter()
            .filter(|symbol| symbol.starts_with("__scoop_abi0_fun__sample_pick_overload_"))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            overload_symbols.len(),
            2,
            "两个 source-level overload 应发布两个 distinct ABI-mangled symbol: {overload_symbols:#?}\n{ir}"
        );
        assert!(
            !ir.contains("@sample.pick(") && !ir.contains("@\"sample.pick\""),
            "source-level overload declaration path 不应再把 raw callable symbol 当作 linker-visible surface:\n{ir}"
        );
    }

    #[test]
    fn llvm_vtable_targets_use_abi_mangler_namespace() {
        let ir = emit_ir_for_source(
            SourceFile::new_virtual(
                "<mem>/vtable_symbol_fixture.scoop",
                r#"
package fixtures.build

open class Base() {
    open fun ping(): Int {
        return 1
    }
}

class DerivedA() : Base() {
    override fun ping(): Int {
        return 11
    }
}

fun helper(base: Base): Int {
    return base.ping() + 1
}

fun main() {
    val base: Base = DerivedA()
    val got: Int = helper(base)
}
"#,
            ),
            "vtable_symbol.ll",
        );
        let base_symbols = ir_defined_function_symbols(&ir)
            .into_iter()
            .filter(|symbol| symbol.starts_with("__scoop_abi0_fun__fixtures_build_Base_ping__h"))
            .collect::<BTreeSet<_>>();
        let derived_symbols = ir_defined_function_symbols(&ir)
            .into_iter()
            .filter(|symbol| {
                symbol.starts_with("__scoop_abi0_fun__fixtures_build_DerivedA_ping__h")
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            base_symbols.len(),
            1,
            "Base.ping 应只发布一个 authoritative ABI symbol，避免定义路径和 vtable 路径各自产生不同 hash: {base_symbols:#?}\n{ir}"
        );
        assert_eq!(
            derived_symbols.len(),
            1,
            "DerivedA.ping 应只发布一个 authoritative ABI symbol，避免定义路径和 vtable 路径各自产生不同 hash: {derived_symbols:#?}\n{ir}"
        );
        let base_symbol = *base_symbols
            .iter()
            .next()
            .expect("Base.ping symbol should exist");
        let derived_symbol = *derived_symbols
            .iter()
            .next()
            .expect("DerivedA.ping symbol should exist");

        assert!(
            ir.lines().any(|line| {
                line.contains("@__scoop_priv0__class_vtable__h")
                    && line.contains("internal constant [1 x ptr]")
                    && ir_line_mentions_symbol(line, base_symbol)
            }),
            "Base vtable target 应引用与函数定义相同的 authoritative ABI symbol，而不是另一条声明路径重新算出来的名字:\n{ir}"
        );
        assert!(
            ir.lines().any(|line| {
                line.contains("@__scoop_priv0__class_vtable__h")
                    && line.contains("internal constant [1 x ptr]")
                    && ir_line_mentions_symbol(line, derived_symbol)
            }),
            "DerivedA vtable target 应引用与函数定义相同的 authoritative ABI symbol，而不是另一条声明路径重新算出来的名字:\n{ir}"
        );
        assert!(
            !ir.contains("@fixtures.build.Base.ping")
                && !ir.contains("@fixtures.build.DerivedA.ping"),
            "class vtable dispatch target 不应再把 raw method fqn 当作 linker-visible surface:\n{ir}"
        );
    }

    #[test]
    fn llvm_codegen_stage_output_is_constructible() {
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
            "LLVM stage 的 HIR scaffold 不应再携带旧 production pass-view 入口"
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
            .expect("默认单文件 IR helper 应经 LLVM stage 成功 lower effectful main");

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
    fn llvm_codegen_stage_shares_same_stage_entry_for_ir_obj_and_asm() {
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
    fn llvm_backend_gate_smoke_lowers_effectful_handle_body_without_legacy() {
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
        .expect("effectful LLVM path 应由 clean stage lowering 成功生成 IR");

        assert_eq!(test_stage_run_count(), 1);
        let ir = std::fs::read_to_string(out).unwrap();
        assert!(ir.contains("scoop.lowered.Step"));
        assert!(ir.contains("call void @scoop_runtime_init()"));
    }

    #[test]
    fn llvm_exported_object_symbols_are_path_stable_across_checkout_roots() {
        let source_text = r#"
package sample

fun helper(): Int {
    return 41
}

fun main(): Int {
    return helper() + 1
}
"#;
        let root_a = make_temp_dir();
        let root_b = make_temp_dir();
        let source_a = write_source_under_root(
            root_a.path(),
            "fixtures/build/path_stable_plain_export.scoop",
            source_text,
        );
        let source_b = write_source_under_root(
            root_b.path(),
            "fixtures/build/path_stable_plain_export.scoop",
            source_text,
        );

        let symbols_a = emit_object_external_symbols_for_source_with_entry(
            source_a,
            "path_stable_plain_export_a.o",
            None,
        )
        .expect("path-stable plain export source 应可成功发 object");
        let symbols_b = emit_object_external_symbols_for_source_with_entry(
            source_b,
            "path_stable_plain_export_b.o",
            None,
        )
        .expect("第二个 checkout 根路径下的同源程序也应可成功发 object");

        assert_eq!(
            symbols_a, symbols_b,
            "同一份输入在不同 checkout 根路径下的 external symbol 集必须保持一致"
        );
        assert!(
            symbols_a
                .iter()
                .any(|symbol| symbol.starts_with("__scoop_abi0_fun__sample_helper__h")),
            "source-level exported helper 应通过 AbiMangler 发布到 object 外部符号表: {symbols_a:#?}"
        );
        assert!(
            !symbols_a.iter().any(|symbol| symbol == "sample.helper"),
            "external symbol 集不应再泄漏 raw callable fqn: {symbols_a:#?}"
        );
    }

    #[test]
    fn llvm_user_abi_symbols_stay_disjoint_for_distinct_virtual_cones() {
        let source_text = r#"
package sample

fun helper(): Int {
    return 41
}

fun main(): Int {
    return helper() + 1
}
"#;
        let root_a = make_temp_dir();
        let root_b = make_temp_dir();
        let source_a = write_source_under_root(
            root_a.path(),
            "fixtures/build/collision_alpha.scoop",
            source_text,
        );
        let source_b = write_source_under_root(
            root_b.path(),
            "fixtures/build/collision_beta.scoop",
            source_text,
        );

        let user_abi_a =
            emit_object_external_symbols_for_source_with_entry(source_a, "collision_alpha.o", None)
                .expect("alpha virtual cone 应可成功发 object")
                .into_iter()
                .filter(|symbol| symbol.starts_with("__scoop_abi0_fun__sample_helper__h"))
                .collect::<BTreeSet<_>>();
        let user_abi_b =
            emit_object_external_symbols_for_source_with_entry(source_b, "collision_beta.o", None)
                .expect("beta virtual cone 应可成功发 object")
                .into_iter()
                .filter(|symbol| symbol.starts_with("__scoop_abi0_fun__sample_helper__h"))
                .collect::<BTreeSet<_>>();

        assert_eq!(
            user_abi_a.len(),
            1,
            "alpha virtual cone 应发布唯一的 helper user ABI symbol: {user_abi_a:#?}"
        );
        assert_eq!(
            user_abi_b.len(),
            1,
            "beta virtual cone 应发布唯一的 helper user ABI symbol: {user_abi_b:#?}"
        );
        let shared = user_abi_a
            .intersection(&user_abi_b)
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            shared.is_empty(),
            "不同 virtual cone 的 overload user ABI symbol 不应碰撞，否则链接阶段会发生冲突: {shared:#?}"
        );
    }
}
