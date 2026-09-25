use std::path::PathBuf;

use scoop_hir as hir;

use super::*;

mod executed;
mod metadata;
mod rejection;
mod support;
use support::{lower, root_name};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(fixture_root().join(format!("{name}.scoop"))).unwrap()
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-dependency-call-occurrences")
}

#[test]
fn actual_dependency_call_occurrences_keep_shared_targets_and_distinct_origins() {
    for name in ["standalone", "routes", "combined", "rejected"] {
        let output = lower(&fixture(name));
        let calls = output.committed_dependency_call_occurrences().unwrap();
        assert_eq!(
            output.imported_dependencies().callable_count(),
            usize::from(name != "rejected")
        );
        let dump = render(&output, &calls);
        if let Some(directory) = std::env::var_os("SCOOP_CALL_OCCURRENCE_SNAPSHOT_DIR") {
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(PathBuf::from(directory).join(format!("{name}.snap")), dump).unwrap();
        } else {
            assert_eq!(
                dump,
                std::fs::read_to_string(fixture_root().join(format!("{name}.snap"))).unwrap()
            );
        }
        match name {
            "standalone" => {
                assert_eq!(calls.len(), 3);
                assert!(
                    calls
                        .windows(2)
                        .all(|pair| pair[0].position() != pair[1].position())
                );
            }
            "routes" => {
                assert_eq!(calls.len(), 3);
                assert_eq!(calls[0].callable().binding().source_count(), 2);
                let route = |index: usize| {
                    calls[index]
                        .binding()
                        .sources()
                        .next()
                        .unwrap()
                        .witness()
                        .route()
                        .clone()
                };
                assert_eq!(route(0), route(2));
                assert_ne!(route(0), route(1));
                assert!(calls.iter().all(|call| call.binding().source_count() == 1));
            }
            "combined" => {
                let defaults = calls
                    .iter()
                    .filter(|call| {
                        output
                            .output()
                            .export
                            .source_context_names(call.origin().definition.context)
                            .0
                            == "withDefault"
                    })
                    .collect::<Vec<_>>();
                assert_eq!(defaults.len(), 2);
                assert_eq!(
                    defaults[0].origin().definition,
                    defaults[1].origin().definition
                );
                assert_ne!(
                    defaults[0].origin().evaluation,
                    defaults[1].origin().evaluation
                );
                assert_ne!(defaults[0].position(), defaults[1].position());
                assert!(calls.iter().all(|call| {
                    output
                        .output()
                        .export
                        .source_context_names(call.origin().definition.context)
                        .0
                        != "dormant"
                }));
            }
            "rejected" => assert!(calls.is_empty()),
            _ => unreachable!(),
        }
    }
}

#[test]
fn occurrence_positions_do_not_depend_on_unrelated_arena_entries() {
    let source = fixture("standalone");
    let baseline = lower(&source);
    let extended = lower(&source.replace(
        "public fun repeated",
        "private fun unrelated(): Boolean = true\n\npublic fun repeated",
    ));
    let positions = |output: &hir::DependencyHirOutput| {
        output
            .committed_dependency_call_occurrences()
            .unwrap()
            .iter()
            .map(|call| call.position())
            .collect::<Vec<_>>()
    };
    assert_eq!(positions(&baseline), positions(&extended));
}

fn render(
    output: &hir::DependencyHirOutput,
    calls: &[hir::CommittedDependencyCallOccurrence<'_>],
) -> String {
    let mut rows = Vec::new();
    for call in calls {
        let origin = call.origin();
        let (definition, _) = output
            .output()
            .export
            .source_context_names(origin.definition.context);
        let (evaluation, _) = output
            .output()
            .export
            .source_context_names(origin.evaluation.context);
        let hops = call
            .binding()
            .sources()
            .map(|source| source.witness().route().hops().len().to_string())
            .collect::<Vec<_>>()
            .join(",");
        rows.push((
            root_name(output.output().local.module(), call.position().root),
            call.position().expression_index,
            format!(
                "routes=[{hops}] definition={definition}@{}..{} evaluation={evaluation}@{}..{}",
                origin.definition.span.start,
                origin.definition.span.end,
                origin.evaluation.span.start,
                origin.evaluation.span.end
            ),
        ));
    }
    rows.sort();
    let mut result = format!(
        "selected={} occurrences={}\n",
        output.imported_dependencies().callable_count(),
        calls.len()
    );
    for (root, index, source) in rows {
        result.push_str(&format!("{root} #{index}: {source}\n"));
    }
    result
}
