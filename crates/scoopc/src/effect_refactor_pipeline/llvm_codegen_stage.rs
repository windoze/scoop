use std::path::Path;

use crate::hir::LoweredHir;
use crate::llvm::LlvmEmitError;
use crate::opt::OptLevel;
use crate::session::Session;
use crate::source::{SourceFile, SourceId, SourceMap};

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
    entry_source: &SourceFile,
    lowered_hir: LoweredHir,
    preserve_published_resume_shells: bool,
) -> Result<RefactorEffectLoweredStageOutput, LlvmEmitError> {
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
    let effect_lowered_stage_output =
        run_effect_lowered_stage_from_lowered_hir(session, entry_source, lowered_hir, false)?;
    let abi_visibility_effect_lowered_stage_output = abi_visibility_lowered_hir
        .map(|lowered_hir| {
            run_effect_lowered_stage_from_lowered_hir(session, entry_source, lowered_hir, true)
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

    fn emit_refactor_ir_for_source(source: SourceFile, file_name: &str) -> String {
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
            None,
            OptLevel::O0,
            LlvmArtifactKind::LlvmIr,
        )
        .unwrap();
        std::fs::read_to_string(out).unwrap()
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
    fn refactor_llvm_codegen_stage_rejects_unmigrated_effect_lowering() {
        let _guard = test_lock();
        let temp = make_temp_dir();
        let out = temp.path().join("effect.ll");

        reset_test_stage_run_count();
        let (session, source_map, entry_source_id, lowered) =
            effectful_emit_args(EffectPipelineMode::Refactor);
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
        .expect_err("effectful refactor LLVM path 应在 stage 边界 fail fast");

        assert_eq!(test_stage_run_count(), 1);
        match err {
            LlvmEmitError::RefactorEffectLoweringUnsupported {
                entry,
                callable,
                unsupported_paths,
            } => {
                assert_eq!(entry, "sample.main");
                assert_eq!(callable, "sample.main");
                assert!(
                    unsupported_paths.contains("perform boundary lowering"),
                    "诊断应明确指出 perform lowering 尚未迁移：{unsupported_paths}"
                );
                assert!(
                    unsupported_paths.contains("resume-state lowering"),
                    "诊断应明确指出 resume-state lowering 尚未迁移：{unsupported_paths}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
