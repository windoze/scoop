use std::io::{self, Write};
use std::path::Path;

use scoop::{BuildFailure, BuildOutcome, CompletedNodeOrigin, LinkOutcome};
use scoop_slib::ArtifactManifestSummaryV1;
use serde_json::{Value, json};

use super::args::MessageFormat;

mod diagnostics;

pub(super) struct Reporter(pub MessageFormat);

impl Reporter {
    pub fn failure(&self, failure: &BuildFailure) -> io::Result<()> {
        self.diagnostics(&failure.warnings, &failure.sources)?;
        if !failure.diagnostics.is_empty() {
            return self.diagnostics(&failure.diagnostics, &failure.sources);
        }
        match self.0 {
            MessageFormat::Json => write_json(&json!({
                "schema": 1, "kind": "diagnostic", "severity": "error", "code": failure.code,
                "phase": failure.phase.as_str(), "message": failure.message,
                "origin": {"kind": "none"}, "notes": [], "display": null,
            })),
            MessageFormat::Human => writeln!(
                io::stderr().lock(),
                "error[{}]: {}",
                failure.code,
                failure.message
            ),
        }
    }

    pub fn built(&self, outcome: &BuildOutcome) -> io::Result<()> {
        let build = outcome.artifacts();
        self.diagnostics(build.graph.warnings(), &build.sources)?;
        if let MessageFormat::Human = self.0 {
            return writeln!(
                io::stderr().lock(),
                "built {} ({})",
                build.output.display(),
                build.profile.as_str()
            );
        }
        let root = build.graph.root();
        let dependencies = build
            .graph
            .artifacts()
            .iter()
            .filter(|node| node.cone() != root.cone())
            .map(|node| artifact(node.artifact().summary(), node.artifact_locator()))
            .collect::<Vec<_>>();
        let observations = build.graph.observations();
        let nodes = observations.nodes().iter().map(|node| json!({
            "identity": node.identity().to_string(),
            "origin": match node.origin() { CompletedNodeOrigin::Compiled => "compiled", CompletedNodeOrigin::CacheHit => "cache", CompletedNodeOrigin::Prebuilt => "prebuilt" },
            "cache_key": node.cache_key().map(|key| key.to_string()),
        })).collect::<Vec<_>>();
        let mut record = json!({
            "schema": 1, "kind": "library", "profile": build.profile.as_str(), "output": path_value(&build.output),
            "root": artifact(root.artifact().summary(), root.artifact_locator()), "dependencies": dependencies,
            "observations": { "nodes": nodes, "child_invocations": observations.child_invocations().iter().map(ToString::to_string).collect::<Vec<_>>() },
        });
        if let BuildOutcome::Executable {
            runtime_index,
            link_plan_fingerprint,
            ..
        } = outcome
        {
            record["kind"] = json!("executable");
            record["runtime_index"] = path_value(runtime_index);
            record["link_plan_fingerprint"] = json!(link_plan_fingerprint.to_string());
        }
        write_json(&record)
    }

    pub fn linked(&self, outcome: &LinkOutcome, dump_plan: bool) -> io::Result<()> {
        match self.0 {
            MessageFormat::Human => {
                let mut stderr = io::stderr().lock();
                if dump_plan {
                    write!(stderr, "{}", outcome.link_plan)?;
                }
                writeln!(stderr, "linked {}", outcome.output.display())
            }
            MessageFormat::Json => {
                let mut record = json!({
                    "schema": 1, "kind": "link", "output": path_value(&outcome.output),
                    "root": artifact(&outcome.root.summary, &outcome.root.path),
                    "dependencies": outcome.dependencies.iter().map(|dependency| artifact(&dependency.summary, &dependency.path)).collect::<Vec<_>>(),
                    "runtime_index": path_value(&outcome.runtime_index), "link_plan_fingerprint": outcome.link_plan_fingerprint.to_string(),
                });
                if dump_plan {
                    record["link_plan"] = json!(outcome.link_plan);
                }
                write_json(&record)
            }
        }
    }
}

fn artifact(summary: &ArtifactManifestSummaryV1, path: &Path) -> Value {
    let coordinate = summary.cone().coordinate();
    json!({
        "coordinate": { "group": coordinate.group(), "name": coordinate.name(), "version": coordinate.version() },
        "identity": summary.cone().identity().to_string(), "artifact_fingerprint": summary.artifact_fingerprint().to_string(),
        "path": path_value(path),
    })
}

fn write_json(value: &Value) -> io::Result<()> {
    let mut stderr = io::stderr().lock();
    serde_json::to_writer(&mut stderr, value)?;
    writeln!(stderr)
}

fn path_value(path: &Path) -> Value {
    if let Some(path) = path.to_str() {
        return json!(path);
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        json!({"encoding": "unix-bytes", "hex": path.as_os_str().as_bytes().iter().map(|byte| format!("{byte:02x}")).collect::<String>()})
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        json!({"encoding": "windows-wide", "units": path.as_os_str().encode_wide().collect::<Vec<_>>()})
    }
}
