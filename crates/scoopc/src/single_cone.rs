//! Subprocess-friendly single-cone artifact compile entry point (P10-T06).
//!
//! `scoop` 的 cone DAG scheduler 把每个 cone 通过 `scoopc build-single-cone` 的子进程
//! 派发到这里：
//! 1. 把 cone-being-compiled 视为 graph consumer，加载它自身的 cone 子图（含递归
//!    sysroot/local-dep）；
//! 2. 把 parent process 提供的 upstream artifact 目录映射成 [`FrontendArtifactCache`]
//!    条目，让所有 dep cone 走 cache-hit 短路；
//! 3. 在主流程里给 consumer 装上 `is_artifact_target=true` 的 cache 条目，让 frontend
//!    构造 skeleton artifact（只含 `frontend_import`，stage products 为空），但**不**
//!    立刻把它写盘——subprocess 调用方拿到 skeleton 后跑完后端 pipeline，再把非空
//!    `LateLoweredProgram` / LIR facts / `.o` 装回去并写盘（P10-T04-c 步骤 1）。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use miette::{Diagnostic, IntoDiagnostic, Result};
use thiserror::Error;

use crate::cone::{
    ConeArtifact, ConeArtifactObject, ConeId, ConeKind, SourceConeRole, StableConeKey,
};
use crate::frontend::{
    FrontendArtifactCache, FrontendArtifactCacheEntry, MirRequestRootMode,
    load_single_cone_project_input_from_path, lower_hir_for_codegen_with_request_root_mode,
    run_frontend_with_artifact_cache,
};
use crate::opt::OptLevel;
use crate::pipeline::{build_llvm_codegen_input, run_llvm_codegen_stage};
use crate::session::{Session, SessionOptions};

#[derive(Debug, Error, Diagnostic)]
#[error("build-single-cone requires `--upstream-artifact` for upstream cone `{cone_id}`")]
#[diagnostic(code(scoopc::single_cone::upstream_artifact_required))]
pub struct UpstreamArtifactRequired {
    pub cone_id: String,
}

/// 子进程视角下的单 .o 命名：dep artifact 目录里只放一个 LLVM object，
/// 由 consumer link 阶段沿 manifest 列出的 `object_files` 拉起。
const SINGLE_CONE_OBJECT_FILE_NAME: &str = "scoop.o";

/// Build the single-cone artifact for `cone_root`.
///
/// `output_dir` receives the artifact directory layout (`manifest.json`,
/// per-stage `.bin` payloads, `frontend_import.json`, `inputs.fingerprint`,
/// `outputs.fingerprint`, `objs/`). `inputs_fingerprint` is whatever the parent
/// driver computed and is written verbatim into the artifact; the parent owns
/// the responsibility of comparing fingerprints when deciding whether to
/// dispatch this subprocess at all.
///
/// `upstream_artifact_dirs` lists every upstream cone artifact this cone may
/// import from. Each upstream's `manifest.json` is read to recover its
/// [`StableConeKey`], which is matched against the consumer cone's dep graph
/// nodes. Unrelated upstreams are ignored; non-sysroot upstream nodes without a
/// matching artifact are rejected before frontend lowering so this subprocess
/// cannot silently compile an upstream dependency from source.
pub fn run_single_cone_artifact_compile(
    session: &Session,
    cone_root: &Path,
    output_dir: &Path,
    inputs_fingerprint: Vec<u8>,
    upstream_artifact_dirs: &[PathBuf],
    session_options: &SessionOptions,
    opt_level: OptLevel,
) -> Result<()> {
    let input = load_single_cone_project_input_from_path(cone_root, session_options)?;
    let consumer_cone_id = input.consumer_cone_id();

    let mut deps_by_key: HashMap<StableConeKey, ConeId> = HashMap::new();
    for node in input.graph().nodes() {
        if node.id == consumer_cone_id {
            continue;
        }
        if !matches!(
            node.role,
            SourceConeRole::LocalDependency | SourceConeRole::SysrootAuto
        ) {
            continue;
        }
        let key = StableConeKey::from_manifest(&node.manifest);
        deps_by_key.insert(key, node.id);
    }

    let mut cache = FrontendArtifactCache::new();
    let mut provided_upstream_keys = HashSet::new();
    for upstream_dir in upstream_artifact_dirs {
        let (manifest, inputs_fp) =
            ConeArtifact::read_manifest_and_inputs_fingerprint(upstream_dir).map_err(|err| {
                miette::miette!(
                    "build-single-cone 无法读取上游 cone artifact `{}`: {err}",
                    upstream_dir.display()
                )
            })?;
        let key = manifest.stable_cone_key();
        provided_upstream_keys.insert(key.clone());
        let Some(&dep_cone_id) = deps_by_key.get(&key) else {
            continue;
        };
        cache.insert(
            dep_cone_id,
            FrontendArtifactCacheEntry::new(upstream_dir.clone(), inputs_fp)
                .with_write_on_cache_miss(false),
        );
    }

    for node in input.graph().nodes() {
        if node.id == consumer_cone_id {
            continue;
        }
        if node.role != SourceConeRole::LocalDependency {
            continue;
        }
        let key = StableConeKey::from_manifest(&node.manifest);
        if !provided_upstream_keys.contains(&key) {
            return Err(UpstreamArtifactRequired {
                cone_id: format!("{}@{}", key.name(), key.version()),
            }
            .into());
        }
    }

    // consumer entry：标记 `is_artifact_target=true` 让 frontend 给 consumer 也构造 skeleton
    // artifact；`write_on_cache_miss=false` 让 frontend 不在这里写盘——subprocess 跑完 LLVM
    // 阶段后再把非空 LIR/.o 装回 skeleton 一并写出。
    cache.insert(
        consumer_cone_id,
        FrontendArtifactCacheEntry::new(output_dir.to_path_buf(), inputs_fingerprint.clone())
            .with_artifact_target(true)
            .with_write_on_cache_miss(false),
    );

    let mut front = run_frontend_with_artifact_cache(session, input, Some(&cache))?;
    let mut skeleton = front
        .take_consumer_artifact_skeleton()
        .ok_or_else(|| {
            miette::miette!(
                "build-single-cone: frontend 未给 consumer cone 生成 artifact skeleton（is_artifact_target=true 应保证）"
            )
        })?;

    let is_bin_consumer = front.input().cone_manifest().cone.kind == ConeKind::Bin;
    let root_mode = if is_bin_consumer {
        MirRequestRootMode::EntryMain
    } else {
        MirRequestRootMode::RequestSources
    };
    let lowering =
        lower_hir_for_codegen_with_request_root_mode(session, &front, opt_level, root_mode)?;
    let extern_libs = lowering.lowered_hir.extern_libs.clone();
    let abi_visibility_lowering = if is_bin_consumer {
        Some(lower_hir_for_codegen_with_request_root_mode(
            session,
            &front,
            opt_level,
            MirRequestRootMode::RequestSources,
        )?)
    } else {
        None
    };
    let (source_map, entry_source_id) = crate::frontend::build_source_map(session, front.input());
    let entry_main_fqn = front.input().entry_main_fqn().map(str::to_owned);

    let cached_dep_artifacts = front.cached_dep_artifacts().to_vec();
    let codegen_input = build_llvm_codegen_input(
        session,
        source_map,
        entry_source_id,
        lowering,
        abi_visibility_lowering,
        entry_main_fqn,
        opt_level,
        cached_dep_artifacts,
    )?;
    let stage_output = run_llvm_codegen_stage(session, codegen_input)?;

    // 把 dep 自己的 LLVM object 写到 artifact `objs/` 目录里，待 consumer link 拉起。
    std::fs::create_dir_all(output_dir).into_diagnostic()?;
    let objs_dir = output_dir.join(crate::cone::CONE_ARTIFACT_OBJS_DIR_NAME);
    std::fs::create_dir_all(&objs_dir).into_diagnostic()?;
    let obj_path = objs_dir.join(SINGLE_CONE_OBJECT_FILE_NAME);
    let stage_input = crate::llvm::StageEmitInput::from_stage_output(&stage_output);
    if is_bin_consumer {
        crate::llvm::emit_main_obj_to_file_from_stage_output(
            stage_output.source_map(),
            stage_output.entry_source_id(),
            stage_input,
            &obj_path,
            front.input().entry_main_fqn(),
            opt_level,
        )?;
    } else {
        crate::llvm::emit_lib_obj_to_file_from_stage_output(
            stage_output.source_map(),
            stage_output.entry_source_id(),
            stage_input,
            &obj_path,
            opt_level,
        )?;
    }
    let obj_bytes = std::fs::read(&obj_path).into_diagnostic()?;
    let mut native_objects = crate::native_build::compile_native_build_objects(
        front.input().graph().consumer(),
        &objs_dir,
    )?;
    if is_bin_consumer {
        for node in front.input().graph().nodes() {
            if node.role != SourceConeRole::SysrootAuto {
                continue;
            }
            let sysroot_objects =
                crate::native_build::compile_native_build_objects(node, &objs_dir)?;
            native_objects.objects.extend(sysroot_objects.objects);
        }
    }

    // 把空 skeleton 升级成包含真实 LIR program / LIR facts / object 的完整 artifact。
    skeleton.lir_program = stage_output.lir().clone();
    skeleton.lir_facts = stage_output.lir_facts().clone();
    skeleton.type_store = stage_output.base_context().types().clone();
    skeleton.manifest.extern_libs = extern_libs;
    skeleton.objects = vec![
        ConeArtifactObject::new(SINGLE_CONE_OBJECT_FILE_NAME, obj_bytes)
            .map_err(|err| miette::miette!("{err}"))?,
    ];
    skeleton.objects.extend(native_objects.objects);
    skeleton.inputs_fingerprint = inputs_fingerprint;

    skeleton
        .write_with_computed_outputs_fingerprint(output_dir)
        .map_err(|err| miette::miette!("{err}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_non_sysroot_upstream_artifact_is_hard_error() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("app");
        let dep = app.join("deps").join("dep");
        std::fs::create_dir_all(app.join("src")).unwrap();
        std::fs::create_dir_all(dep.join("src")).unwrap();
        std::fs::write(
            app.join("Cone.toml"),
            r#"
[cone]
name = "app"
version = "0.0.0"
kind = "bin"

[dependencies]
dep = { path = "deps/dep" }
"#,
        )
        .unwrap();
        std::fs::write(app.join("src/main.scoop"), "package app\nfun main() {}\n").unwrap();
        std::fs::write(
            dep.join("Cone.toml"),
            r#"
[cone]
name = "dep"
version = "0.0.0"
kind = "lib"
"#,
        )
        .unwrap();
        std::fs::write(
            dep.join("src/lib.scoop"),
            "package dep\npublic fun value(): Int { return 1 }\n",
        )
        .unwrap();

        let session = Session::new().unwrap();
        let err = run_single_cone_artifact_compile(
            &session,
            &app,
            &dir.path().join("out"),
            vec![0],
            &[],
            &SessionOptions::new(),
            OptLevel::O0,
        )
        .unwrap_err();

        assert!(
            err.to_string().contains("dep@0.0.0"),
            "unexpected diagnostic: {err:?}"
        );
    }
}
