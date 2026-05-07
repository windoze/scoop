use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
struct MatrixCase {
    owner: &'static str,
    fixture: &'static str,
    coverage: &'static str,
}

const MATRIX: &[MatrixCase] = &[
    MatrixCase {
        owner: "CG-T01",
        fixture: "tests/fixtures/build/effect_refactor_direct_handle_resume_emit_llvm.scoop",
        coverage: "effect/control body reroute and refactor LLVM gate",
    },
    MatrixCase {
        owner: "CG-T02",
        fixture: "tests/fixtures/run-pass/type_check_cast_is_as_asq_basic.scoop",
        coverage: "runtime is/as/as? lowering",
    },
    MatrixCase {
        owner: "CG-T02",
        fixture: "tests/fixtures/run-pass/not_null_assert_basic.scoop",
        coverage: "not-null assertion runtime-error path",
    },
    MatrixCase {
        owner: "CG-T03",
        fixture: "tests/fixtures/run-pass/class_ctor_named_default_and_delegation_basic.scoop",
        coverage: "selected constructor and ordered args contract",
    },
    MatrixCase {
        owner: "CG-T03",
        fixture: "tests/fixtures/run-pass/interface_default_method_dispatch_basic.scoop",
        coverage: "interface default dispatch contract",
    },
    MatrixCase {
        owner: "CG-T03",
        fixture: "tests/fixtures/codegen/intrinsic_size_of_int_word.scoop",
        coverage: "runtime reflection/platform intrinsic lowering",
    },
    MatrixCase {
        owner: "CG-T04",
        fixture: "tests/fixtures/run-pass/value_boxing_tuple_struct_any_basic.scoop",
        coverage: "value boxing composite transport",
    },
    MatrixCase {
        owner: "CG-T04",
        fixture: "tests/fixtures/run-pass/enum_payload_boxing_any_basic.scoop",
        coverage: "payload-bearing enum boxing transport",
    },
    MatrixCase {
        owner: "CG-T04",
        fixture: "tests/fixtures/run-pass/array_composite_transport_basic.scoop",
        coverage: "composite array element transport",
    },
    MatrixCase {
        owner: "CG-T04",
        fixture: "tests/fixtures/run-pass/closure_env_composite_capture_basic.scoop",
        coverage: "closure env composite capture transport",
    },
    MatrixCase {
        owner: "CG-T04",
        fixture: "tests/fixtures/runtime_gc/effect_cross_thread_resume_payload_composite.scoop",
        coverage: "cross-thread composite resume payload rooting",
    },
    MatrixCase {
        owner: "CG-T05",
        fixture: "tests/fixtures/run-pass/effect_typed_plain_adapter_aggregate_return_basic.scoop",
        coverage: "effect-typed adapter aggregate return",
    },
    MatrixCase {
        owner: "CG-T05",
        fixture: "tests/fixtures/run-pass/entry_main_args_int_exit_basic.scoop",
        coverage: "plain main(args) argv ABI",
    },
    MatrixCase {
        owner: "CG-T06",
        fixture: "tests/fixtures/run-pass/handle_finally_boundary.scoop",
        coverage: "cleanup/finally boundary lowering",
    },
    MatrixCase {
        owner: "CG-T06",
        fixture: "tests/fixtures/run-pass/effect_escape_continuation_resume_cross_thread.scoop",
        coverage: "cross-thread continuation resume policy",
    },
    MatrixCase {
        owner: "CG-T06",
        fixture: "tests/fixtures/run-pass/effect_resume_finally_body_raise_after_resume.scoop",
        coverage: "resumed-body raise with finally pending completion",
    },
    MatrixCase {
        owner: "CG-T07",
        fixture: "tests/fixtures/run-pass/extern_global_load_store_basic.scoop",
        coverage: "extern global storage contract",
    },
    MatrixCase {
        owner: "CG-T07",
        fixture: "tests/fixtures/unsafe_nogc/extern_global_access_requires_unsafe_is_error.scoop",
        coverage: "extern global unsafe access diagnostic",
    },
    MatrixCase {
        owner: "CG-T07",
        fixture: "tests/fixtures/run-pass/gc_pin_unpin_basic.scoop",
        coverage: "GC pin/unpin runtime surface",
    },
    MatrixCase {
        owner: "CG-T07",
        fixture: "tests/fixtures/runtime_gc/gc_handle_roundtrip.scoop",
        coverage: "GC stable handle runtime surface",
    },
    MatrixCase {
        owner: "P7-T02Z",
        fixture: "tests/fixtures/run-pass/async_await_minimal_int_basic.scoop",
        coverage: "default refactor task/continuation resume payload blocker",
    },
    MatrixCase {
        owner: "P7-T02Z",
        fixture: "tests/fixtures/run-pass/effect_indirect_perform_nonresuming_function_value_higher_order_when_direct.scoop",
        coverage: "higher-order function-value handled effect blocker",
    },
    MatrixCase {
        owner: "P7-T02Z",
        fixture: "tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_indirect_multi.scoop",
        coverage: "multi-owner continuation schema blocker",
    },
];

fn workspace_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn fixture_phase(fixture: &str) -> &str {
    fixture
        .strip_prefix("tests/fixtures/")
        .and_then(|path| path.split('/').next())
        .expect("matrix fixture path must live under tests/fixtures")
}

#[test]
fn cg8_codegen_regression_matrix_covers_codegen_phase_owners() {
    let owners = MATRIX
        .iter()
        .map(|case| case.owner)
        .collect::<BTreeSet<_>>();

    for expected in [
        "CG-T01", "CG-T02", "CG-T03", "CG-T04", "CG-T05", "CG-T06", "CG-T07",
    ] {
        assert!(owners.contains(expected), "missing matrix owner {expected}");
    }

    assert!(
        owners.contains("P7-T02Z"),
        "CG-T08 must keep the restored P7 run-pass blockers visible"
    );
}

#[test]
fn cg8_codegen_regression_matrix_points_at_default_fixture_phases() {
    for case in MATRIX {
        let path = workspace_path(case.fixture);
        assert!(
            path.is_file(),
            "{} matrix fixture does not exist: {}",
            case.owner,
            case.fixture
        );
        assert!(
            !case.coverage.is_empty(),
            "{} matrix fixture lacks coverage rationale: {}",
            case.owner,
            case.fixture
        );

        let phase = fixture_phase(case.fixture);
        assert!(
            matches!(
                phase,
                "build" | "codegen" | "run-pass" | "runtime_gc" | "typecheck" | "unsafe_nogc"
            ),
            "{} matrix fixture uses unexpected phase `{phase}`: {}",
            case.owner,
            case.fixture
        );
    }
}
