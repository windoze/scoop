use std::path::Path;

use crate::hir::{self, LoweredHir};
use crate::llvm::LlvmEmitError;
use crate::opt::OptLevel;
use crate::session::Session;
use crate::source::{SourceFile, SourceId, SourceMap};
use crate::span::Span;
use crate::ty::{TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::{
    LlvmArtifactKind, RefactorEffectLoweredStageOutput, TypedHirStageOutput,
    build_effect_facts_stage_output, build_effect_lowered_stage_output, mir_stage,
};

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(test)]
static TEST_STAGE_RUNS: AtomicUsize = AtomicUsize::new(0);

/// refactor LLVM codegen stage 的显式输入。
///
/// 约束：
/// - `lowered_hir` 必须来自 build/frontend 的统一 typed lowering；
/// - `abi_visibility_lowered_hir` 若存在，只能用于发布 request-source 范围的 ABI shell；它不能改变
///   reachable body lowering / fail-fast 的 authoritative handoff；
/// - stage 会显式把它推进到 P5 late-lowered handoff；
/// - stage 输出中的 `hir_compat_scaffold` 仅保留当前仍由通用 LLVM codegen 复用的非 effect side
///   tables，不能再作为 effect lowering 的 authoritative 输入。
#[derive(Debug)]
pub struct RefactorLlvmCodegenStageInput {
    lowered_hir: LoweredHir,
    abi_visibility_lowered_hir: Option<LoweredHir>,
    source_map: SourceMap,
    entry_source_id: SourceId,
    entry_main_fqn: Option<String>,
    opt_level: OptLevel,
}

impl RefactorLlvmCodegenStageInput {
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

/// refactor LLVM codegen stage 的稳定 handoff。
///
/// 说明：
/// - `effect_lowered_stage_output` 是 P5 -> P6 的 authoritative handoff；
/// - `abi_visibility_effect_lowered_stage_output` 若存在，则只用于发布 build fixture / ABI 断言所需的
///   request-source callable shell，可见性与 reachable body lowering 明确分离；
/// - `hir_compat_scaffold` 只为当前仍未迁出的通用 LLVM 布局/顶层索引查询提供过渡输入；
/// - 该 scaffold 明确不再携带 `materialized_mir/pass_view`，避免 refactor 路径再回落到旧的
///   `production_lowered_hir` emit helper；
/// - `.ll/.o/.s` 三类产物都必须共用这份 handoff，再进入新的 refactor emit API。
#[derive(Debug)]
pub struct RefactorLlvmCodegenStageOutput {
    source_map: SourceMap,
    entry_source_id: SourceId,
    entry_main_fqn: Option<String>,
    opt_level: OptLevel,
    hir_compat_scaffold: LoweredHir,
    effect_lowered_stage_output: RefactorEffectLoweredStageOutput,
    abi_visibility_effect_lowered_stage_output: Option<RefactorEffectLoweredStageOutput>,
}

impl RefactorLlvmCodegenStageOutput {
    fn new(
        source_map: SourceMap,
        entry_source_id: SourceId,
        entry_main_fqn: Option<String>,
        opt_level: OptLevel,
        hir_compat_scaffold: LoweredHir,
        effect_lowered_stage_output: RefactorEffectLoweredStageOutput,
        abi_visibility_effect_lowered_stage_output: Option<RefactorEffectLoweredStageOutput>,
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

    pub fn effect_lowered_stage_output(&self) -> &RefactorEffectLoweredStageOutput {
        &self.effect_lowered_stage_output
    }

    pub fn abi_visibility_effect_lowered_stage_output(
        &self,
    ) -> Option<&RefactorEffectLoweredStageOutput> {
        self.abi_visibility_effect_lowered_stage_output.as_ref()
    }
}

fn run_effect_lowered_stage_from_lowered_hir(
    session: &Session,
    source_map: &SourceMap,
    entry_source: &SourceFile,
    lowered_hir: LoweredHir,
    preserve_published_resume_shells: bool,
) -> Result<RefactorEffectLoweredStageOutput, LlvmEmitError> {
    precheck_invalid_integer_literals(source_map, entry_source, &lowered_hir)?;
    let source_path = entry_source.path().to_path_buf();
    let typed_hir_output = TypedHirStageOutput::new(lowered_hir, &source_path);
    let mir_stage_output =
        mir_stage::run(typed_hir_output).map_err(|err| stage_error("direct-style MIR", err))?;
    let effect_facts_stage_output =
        build_effect_facts_stage_output(session, entry_source, mir_stage_output)
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

pub(crate) fn run(
    session: &Session,
    input: RefactorLlvmCodegenStageInput,
) -> Result<RefactorLlvmCodegenStageOutput, LlvmEmitError> {
    #[cfg(test)]
    record_test_stage_run();

    let RefactorLlvmCodegenStageInput {
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

    Ok(RefactorLlvmCodegenStageOutput::new(
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
    input: RefactorLlvmCodegenStageInput,
    output: &Path,
    artifact: LlvmArtifactKind,
) -> Result<(), LlvmEmitError> {
    let stage_output = run(session, input)?;
    match artifact {
        LlvmArtifactKind::LlvmIr => crate::llvm::emit_refactor_main_ir_to_file_from_stage_output(
            stage_output.source_map(),
            stage_output.entry_source_id(),
            crate::llvm::RefactorStageEmitInput::new(
                stage_output.hir_compat_scaffold(),
                stage_output.effect_lowered_stage_output(),
                stage_output.abi_visibility_effect_lowered_stage_output(),
            ),
            output,
            stage_output.entry_main_fqn(),
            stage_output.opt_level(),
        ),
        LlvmArtifactKind::Object => crate::llvm::emit_refactor_main_obj_to_file_from_stage_output(
            stage_output.source_map(),
            stage_output.entry_source_id(),
            crate::llvm::RefactorStageEmitInput::new(
                stage_output.hir_compat_scaffold(),
                stage_output.effect_lowered_stage_output(),
                stage_output.abi_visibility_effect_lowered_stage_output(),
            ),
            output,
            stage_output.entry_main_fqn(),
            stage_output.opt_level(),
        ),
        LlvmArtifactKind::Asm => crate::llvm::emit_refactor_main_asm_to_file_from_stage_output(
            stage_output.source_map(),
            stage_output.entry_source_id(),
            crate::llvm::RefactorStageEmitInput::new(
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
    TEST_STAGE_RUNS.fetch_add(1, Ordering::SeqCst);
}

#[cfg(test)]
fn reset_test_stage_run_count() {
    TEST_STAGE_RUNS.store(0, Ordering::SeqCst);
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

    use super::{RefactorLlvmCodegenStageInput, reset_test_stage_run_count, test_stage_run_count};
    use crate::effect_refactor_pipeline::{self, LlvmArtifactKind};
    use crate::llvm::{LlvmEmitError, build_refactor_main_module_from_stage_output};
    use crate::opt::OptLevel;
    use crate::session::{EffectPipelineMode, Session, SessionOptions};
    use crate::source::{SourceFile, SourceMap};

    fn session_for(mode: EffectPipelineMode) -> Session {
        Session::with_options(SessionOptions::new(mode)).unwrap()
    }

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
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
        let (session, source_map, entry_source_id, lowered) =
            emit_args_for_source(EffectPipelineMode::Refactor, source);
        effect_refactor_pipeline::emit_production_llvm_artifact_to_file(
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
        mode: EffectPipelineMode,
        source: SourceFile,
    ) -> (
        Session,
        SourceMap,
        crate::source::SourceId,
        crate::hir::LoweredHir,
    ) {
        let session = session_for(mode);
        let lowered = crate::hir::lower_typed_for_dump(&session, &source).unwrap();
        let mut source_map = SourceMap::new();
        let entry_source_id = source_map.add_source_clone(&source);
        (session, source_map, entry_source_id, lowered)
    }

    fn sample_emit_args(
        mode: EffectPipelineMode,
    ) -> (
        Session,
        SourceMap,
        crate::source::SourceId,
        crate::hir::LoweredHir,
    ) {
        emit_args_for_source(mode, sample_source())
    }

    fn effectful_emit_args(
        mode: EffectPipelineMode,
    ) -> (
        Session,
        SourceMap,
        crate::source::SourceId,
        crate::hir::LoweredHir,
    ) {
        emit_args_for_source(mode, effectful_source())
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
    fn refactor_llvm_main_wrapper_rejects_array_string_argv_until_abi_is_published() {
        let err = emit_refactor_ir_for_source_with_entry(
            array_string_main_source(),
            "main_argv.ll",
            None,
        )
        .expect_err("refactor argv ABI should fail fast until published");
        let message = err.to_string();

        assert!(
            message.contains("Array<String> argv tuple ABI"),
            "diagnostic should name the missing argv ABI contract: {message}"
        );
    }

    #[test]
    fn refactor_llvm_codegen_stage_output_is_constructible() {
        let _guard = test_lock();
        let (session, source_map, entry_source_id, lowered) =
            sample_emit_args(EffectPipelineMode::Refactor);
        let input = RefactorLlvmCodegenStageInput::new(
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
        let module = build_refactor_main_module_from_stage_output(
            stage_output.source_map(),
            stage_output.entry_source_id(),
            &context,
            crate::llvm::RefactorStageEmitInput::new(
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
    fn refactor_llvm_codegen_stage_build_entry_uses_stage_but_legacy_does_not() {
        let _guard = test_lock();
        let temp = make_temp_dir();
        let out = temp.path().join("refactor.ll");

        reset_test_stage_run_count();
        let (session, source_map, entry_source_id, lowered) =
            sample_emit_args(EffectPipelineMode::Refactor);
        effect_refactor_pipeline::emit_production_llvm_artifact_to_file(
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

        reset_test_stage_run_count();
        let temp = make_temp_dir();
        let out = temp.path().join("legacy.ll");
        let (session, source_map, entry_source_id, lowered) =
            sample_emit_args(EffectPipelineMode::Legacy);
        let err = effect_refactor_pipeline::emit_production_llvm_artifact_to_file(
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
        .expect_err("legacy 路径应继续沿用原有 production_lowered_hir 入口");
        assert!(matches!(err, LlvmEmitError::MissingMaterializedPassView));
        assert_eq!(test_stage_run_count(), 0);
    }

    #[test]
    fn refactor_llvm_codegen_stage_shares_same_stage_entry_for_ir_obj_and_asm() {
        let _guard = test_lock();
        let temp = make_temp_dir();
        let artifacts = [
            (LlvmArtifactKind::LlvmIr, PathBuf::from("stage.ll")),
            (LlvmArtifactKind::Object, PathBuf::from("stage.o")),
            (LlvmArtifactKind::Asm, PathBuf::from("stage.s")),
        ];

        reset_test_stage_run_count();
        for (artifact, rel) in artifacts {
            let out = temp.path().join(rel);
            let (session, source_map, entry_source_id, lowered) =
                sample_emit_args(EffectPipelineMode::Refactor);
            effect_refactor_pipeline::emit_production_llvm_artifact_to_file(
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
    fn refactor_llvm_codegen_stage_lowers_effectful_handle_body() {
        let _guard = test_lock();
        let temp = make_temp_dir();
        let out = temp.path().join("effect.ll");

        reset_test_stage_run_count();
        let (session, source_map, entry_source_id, lowered) =
            effectful_emit_args(EffectPipelineMode::Refactor);
        effect_refactor_pipeline::emit_production_llvm_artifact_to_file(
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
        assert!(!ir.contains("scoop_effect_handler_stack"));
        assert!(!ir.contains("scoop_effect_outcome"));
    }
}
