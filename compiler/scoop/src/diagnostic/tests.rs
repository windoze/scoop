use std::collections::BTreeSet;

use scoop_identity::ConeIdentity;
use scoop_protocol::{DiagnosticOriginV1, DiagnosticSeverityV1, StructuredDiagnosticV1};

use super::*;

#[test]
fn phases_keep_the_frozen_diagnostic_order() {
    let phases = [
        BuildFailurePhase::Request,
        BuildFailurePhase::Toolchain,
        BuildFailurePhase::Root,
        BuildFailurePhase::Locator,
        BuildFailurePhase::Summary,
        BuildFailurePhase::GraphIdentity,
        BuildFailurePhase::GraphVersionKind,
        BuildFailurePhase::GraphCycleOrder,
        BuildFailurePhase::SourceSnapshot,
        BuildFailurePhase::PrebuiltArtifact,
        BuildFailurePhase::CoreSlot,
        BuildFailurePhase::Cache,
        BuildFailurePhase::ChildTransport,
        BuildFailurePhase::ChildDiagnostic,
        BuildFailurePhase::ChildOutput,
        BuildFailurePhase::CachePublish,
    ];
    assert!(phases.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn stable_orchestrator_codes_are_unique() {
    let codes = [
        BuildDiagnosticCode::LOCATOR_NOT_FOUND,
        BuildDiagnosticCode::LOCATOR_WRONG_TYPE,
        BuildDiagnosticCode::LOCATOR_COORDINATE_MISMATCH,
        BuildDiagnosticCode::LOCATOR_CONFLICTING_SOURCE,
        BuildDiagnosticCode::LOCATOR_CONFLICTING_REPRESENTATION,
        BuildDiagnosticCode::LOCATOR_AMBIGUOUS_ARTIFACT,
        BuildDiagnosticCode::GRAPH_RESERVED_IDENTITY,
        BuildDiagnosticCode::GRAPH_MULTIPLE_VERSIONS,
        BuildDiagnosticCode::GRAPH_EXECUTABLE_DEPENDENCY,
        BuildDiagnosticCode::GRAPH_SINGLE_FILE_DEPENDENCY,
        BuildDiagnosticCode::GRAPH_MISSING_CORE,
        BuildDiagnosticCode::GRAPH_CYCLE,
        BuildDiagnosticCode::GRAPH_UNREACHABLE_NODE,
        BuildDiagnosticCode::GRAPH_RESOURCE_LIMIT,
        BuildDiagnosticCode::PREBUILT_SUMMARY_MISMATCH,
        BuildDiagnosticCode::PREBUILT_VIEW_INVALID,
        BuildDiagnosticCode::PREBUILT_STALE_DEPENDENCY,
        BuildDiagnosticCode::PREBUILT_CHANGED,
        BuildDiagnosticCode::CACHE_IO,
        BuildDiagnosticCode::CACHE_ENTRY_CORRUPT,
        BuildDiagnosticCode::CACHE_LOCK,
        BuildDiagnosticCode::CACHE_NONDETERMINISTIC_PRODUCTION,
        BuildDiagnosticCode::CACHE_PUBLISH,
        BuildDiagnosticCode::CORE_SLOT_CORRUPT,
        BuildDiagnosticCode::CORE_SOURCE_CHANGED,
        BuildDiagnosticCode::CORE_BOOTSTRAP_FAILED,
        BuildDiagnosticCode::CHILD_TOOL_MISMATCH,
        BuildDiagnosticCode::CHILD_TRANSPORT,
        BuildDiagnosticCode::CHILD_PROTOCOL,
        BuildDiagnosticCode::CHILD_EXIT,
        BuildDiagnosticCode::CHILD_OUTPUT_MISSING,
        BuildDiagnosticCode::CHILD_OUTPUT_PLAN_MISMATCH,
        BuildDiagnosticCode::CHILD_RESPONSE_MISMATCH,
    ];
    assert_eq!(
        codes.len(),
        codes
            .iter()
            .map(|code| code.as_str())
            .collect::<BTreeSet<_>>()
            .len()
    );
}

#[test]
fn representative_failures_have_stable_phase_and_code() {
    let not_found = DependencyLocatorError::ArtifactNotFound {
        coordinate: scoop_identity::ConeCoordinate::new("example", "missing", "1.0.0").unwrap(),
        checked: Vec::new(),
    };
    assert_eq!(
        not_found.classification(),
        classified(
            BuildFailurePhase::Locator,
            BuildDiagnosticCode::LOCATOR_NOT_FOUND
        )
    );

    let cycle = ResolveBuildGraphError::Cycles(Vec::new());
    assert_eq!(
        cycle.classification(),
        classified(
            BuildFailurePhase::GraphCycleOrder,
            BuildDiagnosticCode::GRAPH_CYCLE
        )
    );

    let protocol = ChildTransportError::RequestIdMismatch {
        expected: [1; 16],
        actual: [2; 16],
    };
    assert_eq!(
        protocol.classification(),
        classified(
            BuildFailurePhase::ChildTransport,
            BuildDiagnosticCode::CHILD_PROTOCOL
        )
    );
}

#[test]
fn child_diagnostic_codes_are_not_flattened() {
    let diagnostic = StructuredDiagnosticV1::new(
        DiagnosticSeverityV1::Error,
        "SCOOP_HIR_DEPENDENCY_CAPABILITY".into(),
        "dependency capability is not available".into(),
        DiagnosticOriginV1::None,
        Vec::new(),
    )
    .unwrap();
    let error = BuildGraphExecutionError::Ordinary(
        ConeIdentity::CORE,
        Box::new(OrdinarySourceExecutionError::ChildFailure(vec![diagnostic])),
    );

    assert_eq!(
        error.classification(),
        phase_only(BuildFailurePhase::ChildDiagnostic)
    );
    assert_eq!(
        error.child_diagnostics().unwrap()[0].code(),
        "SCOOP_HIR_DEPENDENCY_CAPABILITY"
    );
}
