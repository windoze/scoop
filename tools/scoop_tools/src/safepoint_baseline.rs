use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use miette::{Context as _, IntoDiagnostic, Result, miette};

const OPT_LEVELS: &[u8] = &[0, 2];

struct Workload {
    name: &'static str,
    fixture_path: &'static str,
    intent: &'static str,
}

const WORKLOADS: &[Workload] = &[
    Workload {
        name: "inline_wrapper_string",
        fixture_path: "tests/fixtures/build/safepoint_inline_wrapper_string_basic.scoop",
        intent: "summary-driven inlining + DirectCallOnly provenance 摊平后的普通调用边界",
    },
    Workload {
        name: "non_escaping_closure",
        fixture_path: "tests/fixtures/build/safepoint_non_escaping_closure_basic.scoop",
        intent: "non-escaping closure simplification 对局部 closure 调用边界的影响",
    },
    Workload {
        name: "root_pressure_loop",
        fixture_path: "tests/fixtures/build/safepoint_root_pressure_loop_basic.scoop",
        intent: "普通 loop 中多个 live String root 跨调用边界的局部 roots 压力",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct SafepointMetrics {
    statepoint_calls: usize,
    rooted_statepoints: usize,
    total_gc_live_roots: usize,
    max_gc_live_roots: usize,
}

#[derive(Debug)]
struct WorkloadResult {
    workload_name: &'static str,
    fixture_path: &'static str,
    opt_level: u8,
    metrics: SafepointMetrics,
}

pub fn run() -> Result<String> {
    let workspace_root = workspace_root()?;
    let temp_root = TempDirGuard::new(workspace_root.join("target"))?;
    let mut results = Vec::new();

    for workload in WORKLOADS {
        for &opt_level in OPT_LEVELS {
            let output = temp_root
                .path()
                .join(format!("{}_O{opt_level}.ll", workload.name));
            build_fixture_ir(&workspace_root, workload.fixture_path, opt_level, &output)
                .with_context(|| {
                    format!(
                        "生成 workload `{}` 的 O{opt_level} LLVM IR 失败",
                        workload.name
                    )
                })?;
            let ir = fs::read_to_string(&output)
                .into_diagnostic()
                .with_context(|| format!("读取 LLVM IR 失败：{}", output.display()))?;
            results.push(WorkloadResult {
                workload_name: workload.name,
                fixture_path: workload.fixture_path,
                opt_level,
                metrics: analyze_ir(&ir),
            });
        }
    }

    Ok(render_report(&results))
}

fn build_fixture_ir(
    workspace_root: &Path,
    fixture_path: &str,
    opt_level: u8,
    output: &Path,
) -> Result<()> {
    let status = Command::new("cargo")
        .current_dir(workspace_root)
        .args([
            "run",
            "-q",
            "-p",
            "scoop",
            "--",
            "build",
            fixture_path,
            "--emit-llvm",
            "--opt-level",
            &opt_level.to_string(),
            "-o",
        ])
        .arg(output)
        .status()
        .into_diagnostic()
        .with_context(|| format!("启动 cargo build 失败：{fixture_path}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(miette!(
            "`cargo run -p scoop -- build {fixture_path} --emit-llvm --opt-level {opt_level}` 失败，退出码 {status}"
        ))
    }
}

fn analyze_ir(ir: &str) -> SafepointMetrics {
    let mut metrics = SafepointMetrics::default();

    for line in ir.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("declare ") || !trimmed.contains("@llvm.experimental.gc.statepoint")
        {
            continue;
        }

        metrics.statepoint_calls += 1;
        let root_count = gc_live_root_count(trimmed);
        if root_count > 0 {
            metrics.rooted_statepoints += 1;
        }
        metrics.total_gc_live_roots += root_count;
        metrics.max_gc_live_roots = metrics.max_gc_live_roots.max(root_count);
    }

    metrics
}

fn gc_live_root_count(line: &str) -> usize {
    const GC_LIVE_PREFIX: &str = r#"[ "gc-live"("#;
    let Some(start) = line.find(GC_LIVE_PREFIX) else {
        return 0;
    };
    let roots = &line[start + GC_LIVE_PREFIX.len()..];
    let Some(end) = roots.find(") ]") else {
        return 0;
    };
    let roots = &roots[..end];
    roots.matches("ptr addrspace(1)").count()
}

fn render_report(results: &[WorkloadResult]) -> String {
    let mut out = String::new();
    out.push_str("# Safepoint Baseline\n\n");
    out.push_str("| workload | opt | statepoints | rooted statepoints | total gc-live roots | max gc-live roots | fixture |\n");
    out.push_str("| --- | --- | ---: | ---: | ---: | ---: | --- |\n");

    for result in results {
        let _ = writeln!(
            out,
            "| `{}` | `O{}` | {} | {} | {} | {} | `{}` |",
            result.workload_name,
            result.opt_level,
            result.metrics.statepoint_calls,
            result.metrics.rooted_statepoints,
            result.metrics.total_gc_live_roots,
            result.metrics.max_gc_live_roots,
            result.fixture_path,
        );
    }

    out.push_str("\n## Workloads\n\n");
    for workload in WORKLOADS {
        let _ = writeln!(
            out,
            "- `{}`: {}（`{}`）",
            workload.name, workload.intent, workload.fixture_path
        );
    }

    out.push_str("\n## O0 vs O2 Deltas\n\n");
    for workload in WORKLOADS {
        let o0 = results
            .iter()
            .find(|result| result.workload_name == workload.name && result.opt_level == 0)
            .expect("missing O0 result");
        let o2 = results
            .iter()
            .find(|result| result.workload_name == workload.name && result.opt_level == 2)
            .expect("missing O2 result");
        let _ = writeln!(
            out,
            "- `{}`: statepoints {} -> {} ({:+}), rooted statepoints {} -> {} ({:+}), total gc-live roots {} -> {} ({:+}), max gc-live roots {} -> {} ({:+})",
            workload.name,
            o0.metrics.statepoint_calls,
            o2.metrics.statepoint_calls,
            delta(o0.metrics.statepoint_calls, o2.metrics.statepoint_calls),
            o0.metrics.rooted_statepoints,
            o2.metrics.rooted_statepoints,
            delta(o0.metrics.rooted_statepoints, o2.metrics.rooted_statepoints),
            o0.metrics.total_gc_live_roots,
            o2.metrics.total_gc_live_roots,
            delta(
                o0.metrics.total_gc_live_roots,
                o2.metrics.total_gc_live_roots
            ),
            o0.metrics.max_gc_live_roots,
            o2.metrics.max_gc_live_roots,
            delta(o0.metrics.max_gc_live_roots, o2.metrics.max_gc_live_roots),
        );
    }

    out.push_str("\n## Notes\n\n");
    out.push_str("- `statepoints` 统计的是 LLVM IR 中实际发射的 `llvm.experimental.gc.statepoint` 调用点，不包含 declaration。\n");
    out.push_str("- `gc-live roots` 统计的是每个 statepoint 上 `\"gc-live\"(...)` metadata 中的 `ptr addrspace(1)` 个数，用作当前 root-pressure 的最小可复验代理指标。\n");
    out.push_str("- 默认 workload 同时覆盖：普通调用边界摊平、non-escaping closure 简化，以及 loop 中多个 live root 跨调用边界的局部 roots 压力。\n");

    out
}

fn delta(before: usize, after: usize) -> isize {
    after as isize - before as isize
}

fn workspace_root() -> Result<PathBuf> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .into_diagnostic()
        .context("解析 workspace root 失败")
}

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(parent: PathBuf) -> Result<Self> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .into_diagnostic()
            .context("生成临时目录时间戳失败")?
            .as_nanos();
        let path = parent.join(format!("safepoint-baseline-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path)
            .into_diagnostic()
            .with_context(|| format!("创建临时目录失败：{}", path.display()))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::{SafepointMetrics, analyze_ir, gc_live_root_count};

    #[test]
    fn gc_live_root_count_ignores_missing_metadata() {
        assert_eq!(
            gc_live_root_count("%x = call token @llvm.experimental.gc.statepoint()"),
            0
        );
    }

    #[test]
    fn gc_live_root_count_counts_roots_inside_metadata_only() {
        let line = r#"%sp = call token @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr @callee, i32 1, i32 0, ptr addrspace(1) %arg, i32 0, i32 0) [ "gc-live"(ptr addrspace(1) %a, ptr addrspace(1) %b) ]"#;
        assert_eq!(gc_live_root_count(line), 2);
    }

    #[test]
    fn analyze_ir_counts_only_real_statepoint_callsites() {
        let ir = r#"
define void @main() gc "statepoint-example" {
entry:
  %sp0 = call token @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr @callee0, i32 0, i32 0, i32 0, i32 0)
  %sp1 = call token @llvm.experimental.gc.statepoint.p0(i64 0, i32 0, ptr @callee1, i32 0, i32 0, i32 0, i32 0) [ "gc-live"(ptr addrspace(1) %a, ptr addrspace(1) %b, ptr addrspace(1) %c) ]
  ret void
}

declare token @llvm.experimental.gc.statepoint.p0(i64 immarg, i32 immarg, ptr, i32 immarg, i32 immarg, ...)
"#;

        assert_eq!(
            analyze_ir(ir),
            SafepointMetrics {
                statepoint_calls: 2,
                rooted_statepoints: 1,
                total_gc_live_roots: 3,
                max_gc_live_roots: 3,
            }
        );
    }
}
