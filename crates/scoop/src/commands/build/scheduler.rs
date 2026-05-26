//! Cone DAG 调度器（P10-T06）。
//!
//! 把 `LocalDependency` cone 的子进程派发从 `build.rs` 的主流程里拆出来，做成
//! 一条可单测的 driver：
//! - 状态机：`Pending` / `Ready` / `InFlight` / `Done` / `Failed`；
//! - 拓扑遍历 `SourceConeGraph`，调度所有需要 artifact 的 `LocalDependency` 与
//!   `Consumer` cone；sysroot 由 `scoopc build-single-cone` 内的 sysroot loader 加载，
//!   不作为 facade 传入的 artifact；
//! - cache-hit 短路：若 `BuildFingerprint::per_cone[cone_id].cached_outputs_fingerprint`
//!   已经命中，则 cone 直接进入 `Done` 状态，不派发子进程；
//! - 失败传播：任何 cone 子进程失败立即停止派发新任务，已派发的任务跑完后聚合
//!   诊断（带 cone 前缀）抛回 driver；
//! - 并发上限：[`super::concurrency::ConcurrencyStrategy::max_concurrent_jobs`]。
//!
//! driver 与 trait 的耦合：通过 [`SubprocessConeCompiler`] 派发，trait 方法只接受
//! `ConeCompileRequest` / 返回 `ConeCompileResponse`，因此 driver 单测可以注入 fake
//! 实现而不依赖真正的 fork+exec。

use std::collections::{HashMap, HashSet};
use std::sync::mpsc;

use miette::Result;

use scoop_project_model::{ConeId, SourceConeGraph, SourceConeRole};

use super::concurrency::{
    ConcurrencyStrategy, ConeCompileRequest, ConeCompileResponse, SubprocessConeCompileError,
    SubprocessConeCompiler,
};
use super::incremental::BuildFingerprint;

/// driver 派发 cone 子进程时聚合在一起的运行依赖。
///
/// 拆成 struct 是为了让 `dispatch_artifact_cones` 的签名长期稳定——若
/// 后续把 strategy / compiler 改成同一个 trait 的多种实现，调用点不必跟着改。
pub(crate) struct ConeBuildDispatch<'a> {
    pub strategy: &'a (dyn ConcurrencyStrategy + 'a),
    pub compiler: &'a (dyn SubprocessConeCompiler + 'a),
    pub opt_level: scoop_project_model::OptLevel,
    pub extra_sysroot_dependencies: &'a [String],
    pub sysroot_overlay: Option<&'a std::path::Path>,
}

#[derive(Debug)]
enum ConeJobState {
    /// 等待至少一个 dep 完成，或等待槽位空闲。
    Pending,
    /// 正在某条 worker 线程上跑 `compile_cone`。
    InFlight,
    /// 子进程成功（或 cache hit），artifact 已落盘。
    Done,
    /// 子进程失败；driver 不再向该 cone 派发。
    Failed,
}

/// 调度需要落盘 artifact 的 cone 子进程派发。
///
/// 调用约定：
/// - `graph` 必须是当前 build 的 source cone graph（`compute_cone_build_fingerprint`
///   消费的是同一份）；
/// - `fingerprint.per_cone` 必须包含 graph 中每个 cone 的条目，否则视为内部错误；
/// - 该函数确保 `LocalDependency` 与 `Consumer` cone 的 artifact 上盘后返回 `Ok(())`；
///   sysroot cone 不生成 facade-owned artifact。
pub(crate) fn dispatch_artifact_cones(
    graph: &SourceConeGraph,
    fingerprint: &BuildFingerprint,
    dispatch: &ConeBuildDispatch<'_>,
) -> Result<()> {
    let mut planner = ConeDispatchPlanner::build(
        graph,
        fingerprint,
        dispatch.opt_level,
        dispatch.extra_sysroot_dependencies,
        dispatch.sysroot_overlay,
    )?;
    if planner.is_empty() {
        return Ok(());
    }

    let max_jobs = dispatch.strategy.max_concurrent_jobs().get();
    let (result_tx, result_rx) = mpsc::channel::<DispatchResult>();
    let mut in_flight: usize = 0;
    let mut first_failure: Option<DispatchFailure> = None;

    std::thread::scope(|scope| {
        loop {
            // 派发 ready 任务直到打满 max_jobs 上限。
            while in_flight < max_jobs && first_failure.is_none() {
                let Some(ready) = planner.pop_ready() else {
                    break;
                };
                let cone_id = ready.cone_id;
                let request = ready.request;
                planner.mark_in_flight(cone_id);
                in_flight += 1;
                let tx = result_tx.clone();
                let compiler = dispatch.compiler;
                let label = ready.label.clone();
                scope.spawn(move || {
                    let result = compiler.compile_cone(request);
                    let _ = tx.send(DispatchResult {
                        cone_id,
                        label,
                        result,
                    });
                });
            }

            if in_flight == 0 {
                // 没有 in-flight 也没有 ready 任务 → 调度结束（或被失败 short-circuit）。
                break;
            }

            // 阻塞等待任一 worker 完成。`scope` 还持有发送端，recv 不会因 channel 关闭
            // 而提前返回 Err。
            let DispatchResult {
                cone_id,
                label,
                result,
            } = result_rx
                .recv()
                .expect("scope 内 result sender 不应被提前 drop");
            in_flight -= 1;
            match result {
                Ok(response) => planner.mark_done(cone_id, response),
                Err(err) => {
                    if first_failure.is_none() {
                        first_failure = Some(DispatchFailure { label, source: err });
                    }
                    planner.mark_failed(cone_id);
                }
            }
        }
        // 所有 in-flight 都已经 join；channel 仍持有 sender 句柄但已无 worker 在用。
        Ok::<_, miette::Report>(())
    })?;

    if let Some(failure) = first_failure {
        let message = format!(
            "per-cone 子进程编译失败 [{label}]：{detail}",
            label = failure.label,
            detail = render_error_with_source_chain(&failure.source),
        );
        if let Some(code) = diagnostic_code_from_subprocess_error(&failure.source) {
            return Err(miette::Report::new(
                miette::MietteDiagnostic::new(message).with_code(code),
            ));
        }
        return Err(miette::miette!("{message}"));
    }

    Ok(())
}

fn diagnostic_code_from_subprocess_error(err: &SubprocessConeCompileError) -> Option<String> {
    let SubprocessConeCompileError::ExitNonZero { stderr, .. } = err else {
        return None;
    };
    stderr.lines().find_map(|line| {
        let rest = line.trim_start().strip_prefix("Error:")?.trim_start();
        rest.split_whitespace()
            .next()
            .filter(|code| code.contains("::"))
            .map(ToOwned::to_owned)
    })
}

/// 把 `failure.source` 的 Display + 整条 `std::error::Error::source()` 链展平成一行。
///
/// 直接 `{err}` 只会拿到 thiserror 顶层 `#[error(...)]` 的字符串；scoopc 子进程返回的
/// io::Error / ExitStatus 等关键上下文都挂在 `#[source]` 字段下，对用户排查最重要的
/// 部分会被隐藏。展平到一行能让 driver 单测断言完整文本，也能在 stderr 里直接告诉
/// 开发者“为什么挂了”。
fn render_error_with_source_chain(err: &(dyn std::error::Error + 'static)) -> String {
    let mut out = err.to_string();
    let mut current = err.source();
    while let Some(source) = current {
        out.push_str(" :: ");
        out.push_str(&source.to_string());
        current = source.source();
    }
    out
}

#[derive(Debug)]
struct ReadyDispatch {
    cone_id: ConeId,
    label: String,
    request: ConeCompileRequest,
}

#[derive(Debug)]
struct DispatchResult {
    cone_id: ConeId,
    label: String,
    result: Result<ConeCompileResponse, SubprocessConeCompileError>,
}

#[derive(Debug)]
struct DispatchFailure {
    label: String,
    source: SubprocessConeCompileError,
}

#[derive(Debug)]
struct ConeJobNode {
    state: ConeJobState,
    label: String,
    request: Option<ConeCompileRequest>,
    pending_deps: HashSet<ConeId>,
    /// 反向边：当本 cone 完成时需要扣减 `pending_deps` 的下游 cone。
    rdeps: Vec<ConeId>,
}

#[derive(Debug)]
struct ConeDispatchPlanner {
    nodes: HashMap<ConeId, ConeJobNode>,
    ready_queue: Vec<ConeId>,
}

impl ConeDispatchPlanner {
    fn build(
        graph: &SourceConeGraph,
        fingerprint: &BuildFingerprint,
        opt_level: scoop_project_model::OptLevel,
        extra_sysroot_dependencies: &[String],
        sysroot_overlay: Option<&std::path::Path>,
    ) -> Result<Self> {
        // 先收集所有非 sysroot artifact cone 的 id，用于过滤 upstream_artifact_dirs：
        // sysroot dep 由 scoopc 在 build-single-cone 内通过 sysroot loader 自行加载，
        // 它们在 fingerprint.per_cone 里虽有占位 artifact_dir，但磁盘上不会真的存在。
        let mut artifact_cone_ids: HashSet<ConeId> = HashSet::new();
        for unit in graph.compilation_units() {
            if matches!(
                unit.role(),
                SourceConeRole::LocalDependency | SourceConeRole::Consumer
            ) {
                artifact_cone_ids.insert(unit.id());
            }
        }

        // 哪些 cone 真正需要走子进程派发：
        // - 必须是 LocalDependency 或 Consumer（sysroot 不参与）；
        // - 必须没有 cache hit。
        let mut should_dispatch: HashSet<ConeId> = HashSet::new();
        for unit in graph.compilation_units() {
            if !matches!(
                unit.role(),
                SourceConeRole::LocalDependency | SourceConeRole::Consumer
            ) {
                continue;
            }
            let cone_fp = fingerprint.per_cone.get(&unit.id()).ok_or_else(|| {
                miette::miette!(
                    "internal: BuildFingerprint::per_cone 缺少 cone {} 的条目",
                    unit.id().as_u32()
                )
            })?;
            if cone_fp.cached_outputs_fingerprint.is_some() {
                continue;
            }
            should_dispatch.insert(unit.id());
        }

        // 已经在磁盘上有 artifact 的 cone（含 cache hit + 别处已经 dispatch 过的）的 dir
        // 仍要作为 upstream 传给子进程：subprocess 才能把它们当成 cache 命中跑完整 frontend。
        let mut nodes: HashMap<ConeId, ConeJobNode> = HashMap::new();
        for unit in graph.compilation_units() {
            if !should_dispatch.contains(&unit.id()) {
                continue;
            }

            let cone_fp = fingerprint
                .per_cone
                .get(&unit.id())
                .expect("已在前一遍校验过 per_cone 条目存在");

            let mut upstream_artifact_dirs = Vec::new();
            for dep_id in unit.dependency_cone_ids() {
                if !artifact_cone_ids.contains(&dep_id) {
                    // sysroot dep（或其它非 artifact 角色）由 scoopc 单 cone 子进程内部加载，
                    // 不会有 facade 侧上盘 artifact；不能传给 scoopc，否则 upstream artifact
                    // 导入会在不存在的目录上失败。
                    continue;
                }
                let Some(dep_fp) = fingerprint.per_cone.get(&dep_id) else {
                    continue;
                };
                upstream_artifact_dirs.push(dep_fp.artifact_dir.clone());
            }

            let label = format!(
                "{}@{}",
                unit.source_cone_info().stable_key.name(),
                unit.source_cone_info().stable_key.version()
            );
            let request = ConeCompileRequest {
                cone_id: label.clone(),
                cone_root: unit.root().to_path_buf(),
                upstream_artifact_dirs,
                opt_level,
                extra_sysroot_dependencies: extra_sysroot_dependencies.to_vec(),
                sysroot_overlay: sysroot_overlay.map(std::path::Path::to_path_buf),
                inputs_fingerprint: cone_fp.inputs_fingerprint.clone(),
                output_artifact_dir: cone_fp.artifact_dir.clone(),
            };

            let mut pending_deps: HashSet<ConeId> = HashSet::new();
            for dep_id in unit.dependency_cone_ids() {
                if should_dispatch.contains(&dep_id) {
                    pending_deps.insert(dep_id);
                }
            }

            nodes.insert(
                unit.id(),
                ConeJobNode {
                    state: ConeJobState::Pending,
                    label,
                    request: Some(request),
                    pending_deps,
                    rdeps: Vec::new(),
                },
            );
        }

        // 反向边：让 mark_done 能 O(deg) 扣减下游。
        let mut rdep_edges: Vec<(ConeId, ConeId)> = Vec::new();
        for unit in graph.compilation_units() {
            if !should_dispatch.contains(&unit.id()) {
                continue;
            }
            for dep_id in unit.dependency_cone_ids() {
                if should_dispatch.contains(&dep_id) {
                    rdep_edges.push((dep_id, unit.id()));
                }
            }
        }
        for (dep_id, downstream) in rdep_edges {
            if let Some(node) = nodes.get_mut(&dep_id) {
                node.rdeps.push(downstream);
            }
        }

        let mut ready_queue: Vec<ConeId> = nodes
            .iter()
            .filter(|(_, node)| node.pending_deps.is_empty())
            .map(|(id, _)| *id)
            .collect();
        ready_queue.sort_by_key(|id| id.as_u32());

        Ok(Self { nodes, ready_queue })
    }

    fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    fn pop_ready(&mut self) -> Option<ReadyDispatch> {
        // ready_queue 在 build 时已经按 cone-id 排序；进入循环后只 push 新解锁的 cone，
        // 这里维持简单的 FIFO 即可（cone-id 顺序对调度公平性帮助不大，但调试更友好）。
        let cone_id = self.ready_queue.pop()?;
        let node = self
            .nodes
            .get_mut(&cone_id)
            .expect("ready_queue 中的 cone 必然存在于 nodes");
        if !matches!(node.state, ConeJobState::Pending) {
            return None;
        }
        let request = node
            .request
            .take()
            .expect("Pending 状态的 cone 必然带 request");
        let label = node.label.clone();
        Some(ReadyDispatch {
            cone_id,
            label,
            request,
        })
    }

    fn mark_in_flight(&mut self, cone_id: ConeId) {
        if let Some(node) = self.nodes.get_mut(&cone_id) {
            node.state = ConeJobState::InFlight;
        }
    }

    fn mark_done(&mut self, cone_id: ConeId, response: ConeCompileResponse) {
        tracing::debug!(
            cone_id = ?cone_id,
            output_dir = %response.output_artifact_dir.display(),
            outputs_fingerprint_len = response.outputs_fingerprint.len(),
            "scheduler: per-cone subprocess artifact recorded"
        );
        let rdeps: Vec<ConeId> = if let Some(node) = self.nodes.get_mut(&cone_id) {
            node.state = ConeJobState::Done;
            node.rdeps.clone()
        } else {
            Vec::new()
        };

        for downstream in rdeps {
            let now_ready = if let Some(node) = self.nodes.get_mut(&downstream) {
                node.pending_deps.remove(&cone_id);
                node.pending_deps.is_empty() && matches!(node.state, ConeJobState::Pending)
            } else {
                false
            };
            if now_ready {
                self.ready_queue.push(downstream);
            }
        }
    }

    fn mark_failed(&mut self, cone_id: ConeId) {
        if let Some(node) = self.nodes.get_mut(&cone_id) {
            node.state = ConeJobState::Failed;
        }
        // 失败后不再向 ready_queue 推新任务；其余 in-flight 任务会自然跑完。
        self.ready_queue.clear();
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::path::PathBuf;
    use std::sync::Mutex;

    use super::super::concurrency::FixedJobsStrategy;
    use super::super::incremental::ConeBuildFingerprint;
    use super::*;

    #[derive(Debug)]
    struct RecordingFakeConeCompiler {
        log: Mutex<Vec<String>>,
    }

    impl RecordingFakeConeCompiler {
        fn new() -> Self {
            Self {
                log: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.log.lock().unwrap().clone()
        }
    }

    impl SubprocessConeCompiler for RecordingFakeConeCompiler {
        fn compile_cone(
            &self,
            request: ConeCompileRequest,
        ) -> Result<ConeCompileResponse, SubprocessConeCompileError> {
            self.log.lock().unwrap().push(request.cone_id.clone());
            Ok(ConeCompileResponse {
                output_artifact_dir: request.output_artifact_dir,
                outputs_fingerprint: vec![0xfa, 0xce],
            })
        }
    }

    fn make_dep_fp(name: &str) -> ConeBuildFingerprint {
        ConeBuildFingerprint {
            artifact_dir: PathBuf::from(format!("/tmp/{name}")),
            inputs_fingerprint: name.as_bytes().to_vec(),
            cached_outputs_fingerprint: None,
        }
    }

    fn make_node(
        id: u32,
        role: SourceConeRole,
        name: &str,
        kind: scoop_project_model::ConeKind,
        deps: &[u32],
    ) -> scoop_project_model::SourceConeNode {
        let manifest = scoop_project_model::ConeManifest {
            cone: scoop_project_model::ConeSection {
                name: name.to_string(),
                version: "0.0.0".to_string(),
                kind,
            },
            dependencies: Default::default(),
            pre_specialize_functions: Vec::new(),
            pre_specialize_types: Vec::new(),
            export_entry_points: Vec::new(),
            selectors: Vec::new(),
            native_build: scoop_project_model::ConeNativeBuildConfig::default(),
        };
        let root = PathBuf::from(format!("/tmp/{name}"));
        let source_path = root.join("src/main.scoop");
        scoop_project_model::SourceConeNode {
            id: ConeId::new(id),
            role,
            root: root.clone(),
            manifest_path: root.join(scoop_project_model::CONE_TOML_FILE_NAME),
            kind,
            native_build: manifest.native_build.clone(),
            manifest,
            trust: scoop_project_model::SourceConeTrust::Untrusted,
            sources: vec![scoop_project_model::SourceFile::new_virtual(
                source_path.clone(),
                format!("package {name}\nfun marker() {{}}\n"),
            )],
            entry_main: (role == SourceConeRole::Consumer).then_some(source_path),
            dependencies: deps
                .iter()
                .map(|id| scoop_project_model::SourceConeDependencyEdge {
                    target: ConeId::new(*id),
                    kind: scoop_project_model::SourceConeDependencyKind::LocalSource,
                })
                .collect(),
        }
    }

    fn fixture_graph() -> (SourceConeGraph, BuildFingerprint) {
        let dep = make_node(
            2,
            SourceConeRole::LocalDependency,
            "fixture.dep",
            scoop_project_model::ConeKind::Lib,
            &[],
        );
        let consumer = make_node(
            1,
            SourceConeRole::Consumer,
            "fixture.app",
            scoop_project_model::ConeKind::Bin,
            &[2],
        );
        let graph = SourceConeGraph::from_nodes(vec![consumer, dep], ConeId::new(1)).unwrap();
        let mut per_cone = HashMap::new();
        per_cone.insert(ConeId::new(1), make_dep_fp("consumer"));
        per_cone.insert(ConeId::new(2), make_dep_fp("dep"));
        let fp = BuildFingerprint {
            fingerprint: "FAKE".to_string(),
            consumer_cone_id: ConeId::new(1),
            per_cone,
            cone_toml_sha256: String::new(),
            cone_sources_sha256: String::new(),
            local_dependency_sources_sha256: String::new(),
            native_sources_sha256: String::new(),
            sysroot_sources_sha256: String::new(),
            runtime_sources_sha256: String::new(),
            toolchain_sha256: String::new(),
        };
        (graph, fp)
    }

    #[test]
    fn dispatch_includes_dependency_and_consumer_cones() {
        let (graph, fp) = fixture_graph();
        let compiler = RecordingFakeConeCompiler::new();
        let strategy = FixedJobsStrategy::new(NonZeroUsize::new(1).unwrap());

        dispatch_artifact_cones(
            &graph,
            &fp,
            &ConeBuildDispatch {
                strategy: &strategy,
                compiler: &compiler,
                opt_level: scoop_project_model::OptLevel::O0,
                extra_sysroot_dependencies: &[],
                sysroot_overlay: None,
            },
        )
        .unwrap();

        assert_eq!(
            compiler.calls(),
            vec![
                "fixture.dep@0.0.0".to_string(),
                "fixture.app@0.0.0".to_string(),
            ]
        );
    }

    #[test]
    fn consumer_cache_hit_short_circuits_all_dispatch() {
        let (graph, mut fp) = fixture_graph();
        fp.per_cone
            .get_mut(&ConeId::new(1))
            .unwrap()
            .cached_outputs_fingerprint = Some(vec![1]);
        fp.per_cone
            .get_mut(&ConeId::new(2))
            .unwrap()
            .cached_outputs_fingerprint = Some(vec![2]);
        let compiler = RecordingFakeConeCompiler::new();
        let strategy = FixedJobsStrategy::new(NonZeroUsize::new(2).unwrap());

        dispatch_artifact_cones(
            &graph,
            &fp,
            &ConeBuildDispatch {
                strategy: &strategy,
                compiler: &compiler,
                opt_level: scoop_project_model::OptLevel::O0,
                extra_sysroot_dependencies: &[],
                sysroot_overlay: None,
            },
        )
        .unwrap();

        assert!(compiler.calls().is_empty());
    }
}
