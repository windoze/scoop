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
        let source = fixture(name);
        let output = lower(&source);
        let (calls, declaration_calls): (Vec<_>, Vec<_>) = output
            .committed_dependency_call_occurrences()
            .unwrap()
            .into_iter()
            .partition(|call| call.binding().is_some());
        assert_eq!(
            output
                .imported_dependencies()
                .callables()
                .filter(|callable| callable.provider() != scoop_identity::ConeIdentity::CORE)
                .count(),
            usize::from(name != "rejected")
        );
        let core = output
            .imported_dependencies()
            .callables()
            .filter(|callable| callable.provider() == scoop_identity::ConeIdentity::CORE)
            .collect::<Vec<_>>();
        assert_eq!(core.len(), usize::from(name == "combined"));
        assert_eq!(!declaration_calls.is_empty(), name == "combined");
        for call in declaration_calls {
            assert_eq!(call.declaration(), core[0].interface().declaration());
            assert_eq!(
                core[0].interface().effects().operator_role(),
                hir::CallableOperatorRoleV1::Language(hir::CallableOperatorV1::Equals)
            );
            assert!(call.position().root.generated_template().is_some());
            assert_eq!(call.arguments().len(), 2);
            let local = output.output().local.module();
            assert!(
                call.arguments()
                    .iter()
                    .all(|argument| local.types[argument.ty].kind
                        == hir::concrete::TypeKind::Boolean)
            );
            assert_eq!(
                local.types[call.result_type()].kind,
                hir::concrete::TypeKind::Boolean
            );
        }
        for call in &calls {
            let origin = call.origin();
            let token =
                &source[origin.definition.span.start as usize..origin.definition.span.end as usize];
            assert!(["run()", "direct()", "forwarded()"].contains(&token));
            if origin.definition.context == origin.evaluation.context {
                assert_eq!(origin.definition.span, origin.evaluation.span);
            } else {
                assert_eq!(token, "run()");
                assert_eq!(
                    &source[origin.evaluation.span.start as usize
                        ..origin.evaluation.span.end as usize],
                    "withDefault()"
                );
            }
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
                let route = |index: usize| {
                    calls[index]
                        .binding()
                        .expect("source import route")
                        .sources()
                        .next()
                        .unwrap()
                        .route()
                        .clone()
                };
                assert_eq!(route(0), route(2));
                assert_ne!(route(0), route(1));
                assert!(
                    calls.iter().all(|call| call
                        .binding()
                        .expect("source import route")
                        .source_count()
                        == 1)
                );
            }
            "combined" => {
                assert_eq!(calls.len(), 18);
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
