//! Cone DAG 调度器（P10-T06）。
//!
//! 把 `LocalDependency` cone 的子进程派发从 `build.rs` 的主流程里拆出来，做成
//! 一条可单测的 driver：
//! - 状态机：`Pending` / `Ready` / `InFlight` / `Done` / `Failed`；
//! - 拓扑遍历 `SourceConeGraph`，只调度 `LocalDependency` cone（consumer 由父进程
//!   in-process 跑完成 frontend + 全程 codegen；sysroot 由 frontend cache 流程自动
//!   在每次 build 内复用，不需要子进程产物）；
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

use scoopc::cone::{ConeId, SourceConeGraph, SourceConeRole};

use super::concurrency::{
    ConcurrencyStrategy, ConeCompileRequest, ConeCompileResponse, SubprocessConeCompileError,
    SubprocessConeCompiler,
};
use super::incremental::BuildFingerprint;

/// driver 派发 cone 子进程时聚合在一起的运行依赖。
///
/// 拆成 struct 是为了让 `dispatch_local_dependency_cones` 的签名长期稳定——若
/// 后续把 strategy / compiler 改成同一个 trait 的多种实现，调用点不必跟着改。
pub(crate) struct ConeBuildDispatch<'a> {
    pub strategy: &'a (dyn ConcurrencyStrategy + 'a),
    pub compiler: &'a (dyn SubprocessConeCompiler + 'a),
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

/// 调度 `LocalDependency` cone 的 frontend artifact 子进程派发。
///
/// 调用约定：
/// - `graph` 必须是当前 build 的 source cone graph（`compute_cone_build_fingerprint`
///   消费的是同一份）；
/// - `fingerprint.per_cone` 必须包含 graph 中每个 cone 的条目，否则视为内部错误；
/// - 该函数只确保 `LocalDependency` cone 的 artifact 上盘后返回 `Ok(())`；consumer 与
///   sysroot 的 frontend lowering 仍由 driver 在主进程内跑（与现有 in-process 行为一致）。
pub(crate) fn dispatch_local_dependency_cones(
    graph: &SourceConeGraph,
    fingerprint: &BuildFingerprint,
    dispatch: &ConeBuildDispatch<'_>,
) -> Result<()> {
    let mut planner = ConeDispatchPlanner::build(graph, fingerprint)?;
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
        return Err(miette::miette!(
            "per-cone 子进程编译失败 [{label}]：{detail}",
            label = failure.label,
            detail = render_error_with_source_chain(&failure.source),
        ));
    }

    Ok(())
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
    fn build(graph: &SourceConeGraph, fingerprint: &BuildFingerprint) -> Result<Self> {
        // 先收集所有 LocalDependency cone 的 id，用于过滤 upstream_artifact_dirs：
        // 只有 LocalDependency cone 才会真的把 artifact 落盘到 build/<profile>/cones/...，
        // 也只有这些 dep 的 artifact_dir 应该作为 `--upstream-artifact` 传给 scoopc 子进程。
        // sysroot dep 由 scoopc 在 build-single-cone 内通过 sysroot loader 自行加载，
        // 它们在 fingerprint.per_cone 里虽有占位 artifact_dir，但磁盘上不会真的存在。
        let mut local_dependency_ids: HashSet<ConeId> = HashSet::new();
        for unit in graph.compilation_units() {
            if unit.role() == SourceConeRole::LocalDependency {
                local_dependency_ids.insert(unit.id());
            }
        }

        // 哪些 cone 真正需要走子进程派发：
        // - 必须是 LocalDependency（consumer / sysroot 不参与）；
        // - 必须没有 cache hit。
        let mut should_dispatch: HashSet<ConeId> = HashSet::new();
        for unit in graph.compilation_units() {
            if unit.role() != SourceConeRole::LocalDependency {
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
                if !local_dependency_ids.contains(&dep_id) {
                    // sysroot dep（或其它非 LocalDependency 角色）在 in-process frontend 中加载，
                    // 不会有上盘 artifact；不能传给 scoopc，否则 import_upstream_artifacts 会
                    // 在不存在的目录上失败（多 cone fixture 上观察到的 ENOENT 路径）。
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

    /// 单测专用的可记录 fake：实现 `SubprocessConeCompiler`，记录每次调用的 cone_id，
    /// 并按预先 stub 的应答返回（默认成功）。
    #[derive(Debug)]
    struct RecordingFakeConeCompiler {
        log: Mutex<Vec<String>>,
        responses: Mutex<HashMap<String, Result<ConeCompileResponse, FakeFailure>>>,
    }

    /// 与真实 `SubprocessConeCompileError::ExitNonZero` 等价的 fake 错误形态，避免
    /// 在单测里手工造系统级 ExitStatus。
    #[derive(Debug, Clone)]
    struct FakeFailure {
        message: String,
    }

    impl RecordingFakeConeCompiler {
        fn new() -> Self {
            Self {
                log: Mutex::new(Vec::new()),
                responses: Mutex::new(HashMap::new()),
            }
        }

        fn stub_failure(&self, cone_id: &str, message: &str) {
            self.responses.lock().unwrap().insert(
                cone_id.to_string(),
                Err(FakeFailure {
                    message: message.to_string(),
                }),
            );
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
            let stub = self.responses.lock().unwrap().remove(&request.cone_id);
            match stub {
                Some(Ok(response)) => Ok(response),
                Some(Err(failure)) => Err(SubprocessConeCompileError::ArtifactMissing {
                    cone_id: request.cone_id.clone(),
                    dir: request.output_artifact_dir.clone(),
                    source: std::io::Error::other(failure.message),
                }),
                None => Ok(ConeCompileResponse {
                    output_artifact_dir: request.output_artifact_dir,
                    outputs_fingerprint: vec![0xfa, 0xce],
                }),
            }
        }
    }

    fn make_dep_fp(name: &str) -> ConeBuildFingerprint {
        ConeBuildFingerprint {
            artifact_dir: PathBuf::from(format!("/tmp/{name}")),
            inputs_fingerprint: name.as_bytes().to_vec(),
            cached_outputs_fingerprint: None,
            direct_dependency_outputs_fingerprints: Vec::new(),
        }
    }

    fn fixture_two_dep_chain_graph() -> (SourceConeGraph, BuildFingerprint) {
        // dep_a (id=2, leaf) ← dep_b (id=3, depends on dep_a) ← consumer (id=1)
        let consumer = make_node(
            1,
            scoopc::cone::SourceConeRole::Consumer,
            "fixture.app",
            scoopc::cone::ConeKind::Bin,
            &[3],
        );
        let dep_b = make_node(
            3,
            scoopc::cone::SourceConeRole::LocalDependency,
            "fixture.dep_b",
            scoopc::cone::ConeKind::Lib,
            &[2],
        );
        let dep_a = make_node(
            2,
            scoopc::cone::SourceConeRole::LocalDependency,
            "fixture.dep_a",
            scoopc::cone::ConeKind::Lib,
            &[],
        );
        let graph = scoopc::cone::SourceConeGraph::from_nodes(
            vec![consumer, dep_b, dep_a],
            scoopc::cone::ConeId::new(1),
        )
        .unwrap();

        let mut per_cone = HashMap::new();
        per_cone.insert(scoopc::cone::ConeId::new(1), make_dep_fp("consumer"));
        per_cone.insert(scoopc::cone::ConeId::new(2), make_dep_fp("dep_a"));
        per_cone.insert(scoopc::cone::ConeId::new(3), make_dep_fp("dep_b"));

        let fp = BuildFingerprint {
            fingerprint: "FAKE".to_string(),
            consumer_cone_id: scoopc::cone::ConeId::new(1),
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

    fn make_node(
        id: u32,
        role: scoopc::cone::SourceConeRole,
        name: &str,
        kind: scoopc::cone::ConeKind,
        deps: &[u32],
    ) -> scoopc::cone::SourceConeNode {
        let manifest = scoopc::cone::ConeManifest {
            cone: scoopc::cone::ConeSection {
                name: name.to_string(),
                version: "0.0.0".to_string(),
                kind,
            },
            dependencies: Default::default(),
            pre_specialize_functions: Vec::new(),
            pre_specialize_types: Vec::new(),
            export_entry_points: Vec::new(),
            selectors: Vec::new(),
            native_build: scoopc::cone::ConeNativeBuildConfig::default(),
        };
        let root = PathBuf::from(format!("/tmp/{name}"));
        let source_path = root.join("src/main.scoop");
        scoopc::cone::SourceConeNode {
            id: scoopc::cone::ConeId::new(id),
            role,
            root: root.clone(),
            manifest_path: root.join(scoopc::cone::CONE_TOML_FILE_NAME),
            kind,
            native_build: manifest.native_build.clone(),
            manifest,
            trust: scoopc::cone::SourceConeTrust::Untrusted,
            sources: vec![scoopc::source::SourceFile::new_virtual(
                source_path.clone(),
                format!("package {name}\nfun marker() {{}}\n"),
            )],
            entry_main: (role == scoopc::cone::SourceConeRole::Consumer).then_some(source_path),
            dependencies: deps
                .iter()
                .map(|id| scoopc::cone::SourceConeDependencyEdge {
                    target: scoopc::cone::ConeId::new(*id),
                    kind: scoopc::cone::SourceConeDependencyKind::LocalSource,
                })
                .collect(),
        }
    }

    #[test]
    fn dispatch_compiles_dep_cones_in_topological_order_with_fake_compiler() {
        let (graph, fp) = fixture_two_dep_chain_graph();
        let compiler = RecordingFakeConeCompiler::new();
        let strategy = FixedJobsStrategy::new(NonZeroUsize::new(2).unwrap());

        dispatch_local_dependency_cones(
            &graph,
            &fp,
            &ConeBuildDispatch {
                strategy: &strategy,
                compiler: &compiler,
            },
        )
        .unwrap();

        let calls = compiler.calls();
        // 必须只调度 LocalDependency cones：consumer 不出现在调用列表里。
        assert!(
            calls.iter().all(|id| id.starts_with("fixture.dep_")),
            "调度器只应派发 LocalDependency cone，得到：{calls:?}"
        );
        assert_eq!(
            calls.len(),
            2,
            "两个 LocalDependency cone 都应被派发：{calls:?}"
        );
        let pos_a = calls
            .iter()
            .position(|id| id == "fixture.dep_a@0.0.0")
            .expect("dep_a 应被派发");
        let pos_b = calls
            .iter()
            .position(|id| id == "fixture.dep_b@0.0.0")
            .expect("dep_b 应被派发");
        assert!(
            pos_a < pos_b,
            "dep_a 是 leaf，必须在 dep_b 之前完成：{calls:?}"
        );
    }

    #[test]
    fn cache_hit_short_circuits_subprocess_dispatch() {
        let (graph, mut fp) = fixture_two_dep_chain_graph();
        // 把 dep_a 标记为 cache hit。
        fp.per_cone
            .get_mut(&scoopc::cone::ConeId::new(2))
            .unwrap()
            .cached_outputs_fingerprint = Some(vec![0xc0, 0xff, 0xee]);

        let compiler = RecordingFakeConeCompiler::new();
        let strategy = FixedJobsStrategy::new(NonZeroUsize::new(4).unwrap());

        dispatch_local_dependency_cones(
            &graph,
            &fp,
            &ConeBuildDispatch {
                strategy: &strategy,
                compiler: &compiler,
            },
        )
        .unwrap();

        let calls = compiler.calls();
        assert_eq!(
            calls,
            vec!["fixture.dep_b@0.0.0".to_string()],
            "cache hit 的 cone 不应再被派发：{calls:?}"
        );
    }

    #[test]
    fn dispatch_propagates_subprocess_failure_with_cone_prefixed_diagnostic() {
        let (graph, fp) = fixture_two_dep_chain_graph();
        let compiler = RecordingFakeConeCompiler::new();
        compiler.stub_failure("fixture.dep_a@0.0.0", "synthetic dep failure");
        let strategy = FixedJobsStrategy::new(NonZeroUsize::new(1).unwrap());

        let err = dispatch_local_dependency_cones(
            &graph,
            &fp,
            &ConeBuildDispatch {
                strategy: &strategy,
                compiler: &compiler,
            },
        )
        .unwrap_err();

        let msg = format!("{err:?}");
        assert!(
            msg.contains("fixture.dep_a@0.0.0"),
            "失败诊断应带 cone 前缀：{msg}"
        );
        assert!(
            msg.contains("synthetic dep failure"),
            "失败诊断应回溯子进程错误：{msg}"
        );

        // dep_a 失败后，dep_b（依赖 dep_a）不应被派发。
        let calls = compiler.calls();
        assert!(
            !calls.iter().any(|id| id == "fixture.dep_b@0.0.0"),
            "leaf 失败后下游不应被调度：{calls:?}"
        );
    }

    /// 用 [`std::sync::Barrier`] 卡住两个 worker，从而能直接观测调度器并发上限。
    ///
    /// 与 [`RecordingFakeConeCompiler`] 同等地实现 [`SubprocessConeCompiler`]，但所有 worker
    /// 在 barrier 上对齐：只有 `max_jobs >= barrier.size` 时调度器才能让所有 worker 同时
    /// 进入 `compile_cone`，否则 barrier 永远等不齐 → 测试 deadlock。
    #[derive(Debug)]
    struct BarrierFakeConeCompiler {
        log: Mutex<Vec<String>>,
        in_flight: std::sync::atomic::AtomicUsize,
        peak_in_flight: std::sync::atomic::AtomicUsize,
        barrier: std::sync::Barrier,
    }

    impl BarrierFakeConeCompiler {
        fn new(expect_concurrent: usize) -> Self {
            Self {
                log: Mutex::new(Vec::new()),
                in_flight: std::sync::atomic::AtomicUsize::new(0),
                peak_in_flight: std::sync::atomic::AtomicUsize::new(0),
                barrier: std::sync::Barrier::new(expect_concurrent),
            }
        }

        fn peak_in_flight(&self) -> usize {
            self.peak_in_flight
                .load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    impl SubprocessConeCompiler for BarrierFakeConeCompiler {
        fn compile_cone(
            &self,
            request: ConeCompileRequest,
        ) -> Result<ConeCompileResponse, SubprocessConeCompileError> {
            self.log.lock().unwrap().push(request.cone_id.clone());
            let now = self
                .in_flight
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1;
            self.peak_in_flight
                .fetch_max(now, std::sync::atomic::Ordering::SeqCst);
            // 卡住所有 worker，直到调度器至少把 `expect_concurrent` 个 cone 同时
            // 派发到子进程上为止。max_jobs 不够时 barrier 永远等不齐 → 测试就会
            // 在 unwrap 超时上挂掉，体现 contract 违例。
            self.barrier.wait();
            self.in_flight
                .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            Ok(ConeCompileResponse {
                output_artifact_dir: request.output_artifact_dir,
                outputs_fingerprint: vec![0xfa, 0xce],
            })
        }
    }

    fn fixture_two_independent_leaf_graph() -> (SourceConeGraph, BuildFingerprint) {
        // consumer (id=1) → dep_a (id=2, leaf), dep_b (id=3, leaf)
        let consumer = make_node(
            1,
            scoopc::cone::SourceConeRole::Consumer,
            "fixture.app",
            scoopc::cone::ConeKind::Bin,
            &[2, 3],
        );
        let dep_a = make_node(
            2,
            scoopc::cone::SourceConeRole::LocalDependency,
            "fixture.dep_a",
            scoopc::cone::ConeKind::Lib,
            &[],
        );
        let dep_b = make_node(
            3,
            scoopc::cone::SourceConeRole::LocalDependency,
            "fixture.dep_b",
            scoopc::cone::ConeKind::Lib,
            &[],
        );
        let graph = scoopc::cone::SourceConeGraph::from_nodes(
            vec![consumer, dep_a, dep_b],
            scoopc::cone::ConeId::new(1),
        )
        .unwrap();

        let mut per_cone = HashMap::new();
        per_cone.insert(scoopc::cone::ConeId::new(1), make_dep_fp("consumer"));
        per_cone.insert(scoopc::cone::ConeId::new(2), make_dep_fp("dep_a"));
        per_cone.insert(scoopc::cone::ConeId::new(3), make_dep_fp("dep_b"));

        let fp = BuildFingerprint {
            fingerprint: "FAKE".to_string(),
            consumer_cone_id: scoopc::cone::ConeId::new(1),
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
    fn dispatch_runs_independent_cones_concurrently_up_to_strategy_limit() {
        // 两个独立 leaf cone + max_jobs=2 → 两个 worker 必须同时进入 compile_cone，
        // 否则 barrier(2) 永远等不齐，dispatch_local_dependency_cones 会 deadlock。
        let (graph, fp) = fixture_two_independent_leaf_graph();
        let compiler = BarrierFakeConeCompiler::new(2);
        let strategy = FixedJobsStrategy::new(NonZeroUsize::new(2).unwrap());

        dispatch_local_dependency_cones(
            &graph,
            &fp,
            &ConeBuildDispatch {
                strategy: &strategy,
                compiler: &compiler,
            },
        )
        .unwrap();

        assert_eq!(
            compiler.peak_in_flight(),
            2,
            "ConcurrencyStrategy::max_concurrent_jobs=2 时调度器必须让两个独立 cone 真的并行 in-flight"
        );
    }

    #[test]
    fn dispatch_caps_concurrency_at_strategy_max_jobs() {
        // 两个独立 leaf + max_jobs=1 → 即使 ready_queue 同时有两条任务，调度器也
        // 只能让其中一个先跑完才能开下一个。barrier(1) 让单 worker 立刻通过，
        // peak_in_flight 只能是 1。
        let (graph, fp) = fixture_two_independent_leaf_graph();
        let compiler = BarrierFakeConeCompiler::new(1);
        let strategy = FixedJobsStrategy::new(NonZeroUsize::new(1).unwrap());

        dispatch_local_dependency_cones(
            &graph,
            &fp,
            &ConeBuildDispatch {
                strategy: &strategy,
                compiler: &compiler,
            },
        )
        .unwrap();

        assert_eq!(
            compiler.peak_in_flight(),
            1,
            "max_jobs=1 时调度器必须串行执行，peak_in_flight 不能超过 1"
        );
        assert_eq!(
            compiler.log.lock().unwrap().len(),
            2,
            "两个 cone 都应被派发，只是不能同时跑"
        );
    }

    #[test]
    fn dispatch_skips_when_no_local_dependency_cones_present() {
        // virtual cone fixture：只有 consumer，没有 LocalDependency。
        let consumer = make_node(
            1,
            scoopc::cone::SourceConeRole::Consumer,
            "fixture.solo",
            scoopc::cone::ConeKind::Bin,
            &[],
        );
        let graph =
            scoopc::cone::SourceConeGraph::from_nodes(vec![consumer], scoopc::cone::ConeId::new(1))
                .unwrap();

        let mut per_cone = HashMap::new();
        per_cone.insert(scoopc::cone::ConeId::new(1), make_dep_fp("solo"));
        let fp = BuildFingerprint {
            fingerprint: "FAKE".to_string(),
            consumer_cone_id: scoopc::cone::ConeId::new(1),
            per_cone,
            cone_toml_sha256: String::new(),
            cone_sources_sha256: String::new(),
            local_dependency_sources_sha256: String::new(),
            native_sources_sha256: String::new(),
            sysroot_sources_sha256: String::new(),
            runtime_sources_sha256: String::new(),
            toolchain_sha256: String::new(),
        };

        let compiler = RecordingFakeConeCompiler::new();
        let strategy = FixedJobsStrategy::new(NonZeroUsize::new(2).unwrap());

        dispatch_local_dependency_cones(
            &graph,
            &fp,
            &ConeBuildDispatch {
                strategy: &strategy,
                compiler: &compiler,
            },
        )
        .unwrap();

        assert!(
            compiler.calls().is_empty(),
            "纯 consumer graph 不应触发任何子进程派发"
        );
    }
}
