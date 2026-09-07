//! Explicit closure validation tests: every error class from DESIGN
//! sections 1.4 / 5.5 plus the happy paths.

use scoop_identity::capability::scoop_lir_link_object_v1;
use scoop_identity::{
    ConeCoordinate, ConeIdentity, Digest256, ObjectFormatId, TargetProfileWireId,
};
use scoop_manifest::ConeKind;

use crate::artifact::{
    ManifestCoreTemplate, SlibBuilder, ValidatedGraphArtifact, plain_logical_key,
};
use crate::closure::{ClosureError, validate_explicit_closure};
use crate::limits::SlibDecodeLimits;
use crate::manifest::DependencyRecord;
use crate::member::{MemberStableKey, SlibMemberRole};
use crate::purpose;

fn template(
    group: &str,
    name: &str,
    version: &str,
    deps: Vec<ConeCoordinate>,
) -> ManifestCoreTemplate {
    ManifestCoreTemplate {
        container_version: 1,
        hir_wire_schema: 1,
        mir_wire_schema: 1,
        lir_wire_schema: 1,
        producer_compiler_version: "0.0.0 (test)".to_owned(),
        language_abi: 1,
        runtime_abi: Digest256::from_bytes([1; 32]),
        identity_schema_version: 1,
        coordinate: ConeCoordinate::new(group, name, version).unwrap(),
        kind: ConeKind::Library,
        dependencies: deps
            .into_iter()
            .map(|coordinate| DependencyRecord {
                cone_identity: ConeIdentity::of(&coordinate),
                hir_semantic_fingerprint: Digest256::from_bytes([7; 32]),
                mir_semantic_fingerprint: Digest256::from_bytes([8; 32]),
                lir_semantic_fingerprint: Digest256::from_bytes([9; 32]),
            })
            .collect(),
        target_profile: TargetProfileWireId::darwin_aarch64_v1(),
        target_profile_fingerprint: Digest256::from_bytes([5; 32]),
        backend_profile_fingerprint: Digest256::from_bytes([6; 32]),
    }
}

fn build(template: ManifestCoreTemplate) -> ValidatedGraphArtifact {
    let mut builder = SlibBuilder::new(template);
    for (key, role) in [
        (
            MemberStableKey::HirMetadata,
            SlibMemberRole::HirMetadata { wire_schema: 1 },
        ),
        (
            MemberStableKey::MirMetadata,
            SlibMemberRole::MirMetadata { wire_schema: 1 },
        ),
        (
            MemberStableKey::LirMetadata,
            SlibMemberRole::LirMetadata { wire_schema: 1 },
        ),
    ] {
        builder.add_member(key, role, b"metadata".to_vec()).unwrap();
    }
    let data = builder.finish().unwrap();
    ValidatedGraphArtifact::read(&data, &SlibDecodeLimits::strict()).unwrap()
}

fn core_artifact() -> ValidatedGraphArtifact {
    build(template("scoop", "scoop.core", "0.1.0", vec![]))
}

fn cone(
    group: &str,
    name: &str,
    version: &str,
    deps: Vec<ConeCoordinate>,
) -> ValidatedGraphArtifact {
    build(template(group, name, version, deps))
}

fn coord(group: &str, name: &str, version: &str) -> ConeCoordinate {
    ConeCoordinate::new(group, name, version).unwrap()
}

#[test]
fn core_only_closure_is_valid() {
    let closure =
        validate_explicit_closure::<purpose::Graph>(core_artifact(), vec![], vec![]).unwrap();
    assert!(closure.direct().is_empty());
    assert!(closure.support().is_empty());
    assert!(closure.core().coordinate().is_reserved_core());
}

#[test]
fn chain_and_diamond_closures_are_valid() {
    let core = core_artifact();
    // app -> util -> log -> core
    let log = cone(
        "org.other",
        "log",
        "3.1.0",
        vec![coord("scoop", "scoop.core", "0.1.0")],
    );
    let util = cone(
        "org.acme",
        "util",
        "2.0.0",
        vec![coord("org.other", "log", "3.1.0")],
    );
    let app = cone(
        "dev.example",
        "app",
        "0.1.0",
        vec![coord("org.acme", "util", "2.0.0")],
    );
    let closure =
        validate_explicit_closure::<purpose::Graph>(core, vec![app], vec![util, log]).unwrap();
    assert_eq!(closure.support().len(), 2);

    // Diamond: app -> {a, b}, b -> a.
    let core = core_artifact();
    let a = cone(
        "org.x",
        "a",
        "1.0.0",
        vec![coord("scoop", "scoop.core", "0.1.0")],
    );
    let b = cone("org.x", "b", "1.0.0", vec![coord("org.x", "a", "1.0.0")]);
    let root = cone(
        "dev.example",
        "root",
        "0.1.0",
        vec![coord("org.x", "a", "1.0.0"), coord("org.x", "b", "1.0.0")],
    );
    let closure =
        validate_explicit_closure::<purpose::Graph>(core, vec![root, a, b], vec![]).unwrap();
    assert_eq!(closure.direct().len(), 3);
    assert!(closure.support().is_empty());
}

#[test]
fn argument_order_is_irrelevant() {
    // One support artifact listed before/after re-sorted direct inputs
    // validates identically.
    let core = core_artifact();
    let util = cone(
        "org.acme",
        "util",
        "2.0.0",
        vec![coord("scoop", "scoop.core", "0.1.0")],
    );
    let app = cone(
        "dev.example",
        "app",
        "0.1.0",
        vec![coord("org.acme", "util", "2.0.0")],
    );
    let closure = validate_explicit_closure::<purpose::Graph>(core, vec![app], vec![util]).unwrap();
    // Direct/support sets come back in canonical identity order.
    assert_eq!(closure.direct().len(), 1);
    assert_eq!(closure.support().len(), 1);
}

#[test]
fn non_reserved_core_is_rejected() {
    let fake_core = cone("dev.example", "fake-core", "1.0.0", vec![]);
    assert!(matches!(
        validate_explicit_closure::<purpose::Graph>(fake_core, vec![], vec![]).unwrap_err(),
        ClosureError::NotReservedCore { .. }
    ));
}

#[test]
fn core_with_dependencies_is_rejected() {
    let mut core_template = template("scoop", "scoop.core", "0.1.0", vec![]);
    core_template.dependencies.push(DependencyRecord {
        cone_identity: ConeIdentity::of(&coord("org.x", "x", "1.0.0")),
        hir_semantic_fingerprint: Digest256::ZERO,
        mir_semantic_fingerprint: Digest256::ZERO,
        lir_semantic_fingerprint: Digest256::ZERO,
    });
    let bad_core = build(core_template);
    assert!(matches!(
        validate_explicit_closure::<purpose::Graph>(bad_core, vec![], vec![]).unwrap_err(),
        ClosureError::CoreHasDependencies(1)
    ));
}

#[test]
fn duplicate_identity_is_rejected() {
    let core = core_artifact();
    let util = cone(
        "org.acme",
        "util",
        "2.0.0",
        vec![coord("scoop", "scoop.core", "0.1.0")],
    );
    let util_again = cone(
        "org.acme",
        "util",
        "2.0.0",
        vec![coord("scoop", "scoop.core", "0.1.0")],
    );
    assert!(matches!(
        validate_explicit_closure::<purpose::Graph>(core, vec![util], vec![util_again])
            .unwrap_err(),
        ClosureError::DuplicateIdentity { .. }
    ));
}

#[test]
fn multiple_versions_of_one_cone_are_rejected() {
    let core = core_artifact();
    let v1 = cone(
        "org.acme",
        "util",
        "1.0.0",
        vec![coord("scoop", "scoop.core", "0.1.0")],
    );
    let v2 = cone(
        "org.acme",
        "util",
        "2.0.0",
        vec![coord("scoop", "scoop.core", "0.1.0")],
    );
    let root = cone(
        "dev.example",
        "root",
        "0.1.0",
        vec![
            coord("org.acme", "util", "1.0.0"),
            coord("org.acme", "util", "2.0.0"),
        ],
    );
    let error =
        validate_explicit_closure::<purpose::Graph>(core, vec![root, v1], vec![v2]).unwrap_err();
    assert!(matches!(error, ClosureError::MultipleVersions { .. }));
}

#[test]
fn executable_dependency_is_rejected() {
    let core = core_artifact();
    let mut exe_template = template(
        "org.x",
        "tool",
        "1.0.0",
        vec![coord("scoop", "scoop.core", "0.1.0")],
    );
    exe_template.kind = ConeKind::Executable;
    let exe = build(exe_template);
    let root = cone(
        "dev.example",
        "root",
        "0.1.0",
        vec![coord("org.x", "tool", "1.0.0")],
    );
    assert!(matches!(
        validate_explicit_closure::<purpose::Graph>(core, vec![root], vec![exe]).unwrap_err(),
        ClosureError::ExecutableDependency { .. }
    ));
}

#[test]
fn missing_support_artifact_is_rejected() {
    let core = core_artifact();
    let util = cone(
        "org.acme",
        "util",
        "2.0.0",
        vec![coord("org.other", "log", "3.1.0")],
    );
    let root = cone(
        "dev.example",
        "root",
        "0.1.0",
        vec![coord("org.acme", "util", "2.0.0")],
    );
    // log is required by util but not supplied.
    assert!(matches!(
        validate_explicit_closure::<purpose::Graph>(core, vec![root], vec![util]).unwrap_err(),
        ClosureError::DanglingEdge { .. }
    ));
    let core2 = core_artifact();
    let util2 = cone(
        "org.acme",
        "util",
        "2.0.0",
        vec![coord("org.other", "log", "3.1.0")],
    );
    let log2 = cone(
        "org.other",
        "log",
        "3.1.0",
        vec![coord("scoop", "scoop.core", "0.1.0")],
    );
    let root2 = cone(
        "dev.example",
        "root",
        "0.1.0",
        vec![coord("org.acme", "util", "2.0.0")],
    );
    validate_explicit_closure::<purpose::Graph>(core2, vec![root2], vec![util2, log2]).unwrap();
}

#[test]
fn unreachable_support_artifact_is_rejected() {
    let core = core_artifact();
    let util = cone(
        "org.acme",
        "util",
        "2.0.0",
        vec![coord("scoop", "scoop.core", "0.1.0")],
    );
    let root = cone(
        "dev.example",
        "root",
        "0.1.0",
        vec![coord("org.acme", "util", "2.0.0")],
    );
    let stray = cone(
        "org.zzz",
        "stray",
        "1.0.0",
        vec![coord("scoop", "scoop.core", "0.1.0")],
    );
    assert!(matches!(
        validate_explicit_closure::<purpose::Graph>(core, vec![root, util], vec![stray])
            .unwrap_err(),
        ClosureError::UnreachableArtifact { .. }
    ));
}

#[test]
fn direct_support_overlap_is_rejected() {
    let core = core_artifact();
    let util = cone(
        "org.acme",
        "util",
        "2.0.0",
        vec![coord("scoop", "scoop.core", "0.1.0")],
    );
    let util_again = cone(
        "org.acme",
        "util",
        "2.0.0",
        vec![coord("scoop", "scoop.core", "0.1.0")],
    );
    let root = cone(
        "dev.example",
        "root",
        "0.1.0",
        vec![coord("org.acme", "util", "2.0.0")],
    );
    assert!(matches!(
        validate_explicit_closure::<purpose::Graph>(core, vec![root, util, util_again], vec![])
            .unwrap_err(),
        ClosureError::DuplicateIdentity { .. }
    ));
}

#[test]
fn dangling_edge_is_rejected() {
    let core = core_artifact();
    // util depends on log, which is entirely absent.
    let util = cone(
        "org.acme",
        "util",
        "2.0.0",
        vec![coord("org.other", "log", "3.1.0")],
    );
    let root = cone(
        "dev.example",
        "root",
        "0.1.0",
        vec![coord("org.acme", "util", "2.0.0")],
    );
    let error =
        validate_explicit_closure::<purpose::Graph>(core, vec![root], vec![util]).unwrap_err();
    assert!(matches!(error, ClosureError::DanglingEdge { .. }));
}

#[test]
fn self_loop_is_rejected() {
    let core = core_artifact();
    let selfish = cone(
        "org.x",
        "selfish",
        "1.0.0",
        vec![coord("org.x", "selfish", "1.0.0")],
    );
    let error =
        validate_explicit_closure::<purpose::Graph>(core, vec![selfish], vec![]).unwrap_err();
    assert!(matches!(
        error,
        ClosureError::Cycle { .. } | ClosureError::DanglingEdge { .. }
    ));
}

#[test]
fn target_and_schema_mismatches_are_rejected() {
    // Different target profile id.
    let core = core_artifact();
    let mut other_target = template(
        "org.x",
        "x",
        "1.0.0",
        vec![coord("scoop", "scoop.core", "0.1.0")],
    );
    other_target.target_profile = TargetProfileWireId::new(
        scoop_identity::CapabilityId::new("org.scoop-lang.target-profile", "darwin-x86-64", 1)
            .unwrap(),
    );
    let x = build(other_target);
    let root = cone(
        "dev.example",
        "root",
        "0.1.0",
        vec![coord("org.x", "x", "1.0.0")],
    );
    assert!(matches!(
        validate_explicit_closure::<purpose::Graph>(core, vec![root], vec![x]).unwrap_err(),
        ClosureError::TargetMismatch { .. }
    ));

    // Different HIR wire schema.
    let core2 = core_artifact();
    let mut other_schema = template(
        "org.y",
        "y",
        "1.0.0",
        vec![coord("scoop", "scoop.core", "0.1.0")],
    );
    other_schema.hir_wire_schema = 2;
    let y = build(other_schema);
    let root2 = cone(
        "dev.example",
        "root",
        "0.1.0",
        vec![coord("org.y", "y", "1.0.0")],
    );
    assert!(matches!(
        validate_explicit_closure::<purpose::Graph>(core2, vec![root2], vec![y]).unwrap_err(),
        ClosureError::SchemaMismatch {
            field: "HIR wire schema",
            ..
        }
    ));
}

#[test]
fn link_object_member_round_trips_through_graph_view() {
    // Ensure link objects survive the envelope inside a closure member.
    let mut builder = SlibBuilder::new(template("org.x", "x", "1.0.0", vec![]));
    builder
        .add_member(
            MemberStableKey::HirMetadata,
            SlibMemberRole::HirMetadata { wire_schema: 1 },
            b"h".to_vec(),
        )
        .unwrap();
    builder
        .add_member(
            MemberStableKey::MirMetadata,
            SlibMemberRole::MirMetadata { wire_schema: 1 },
            b"m".to_vec(),
        )
        .unwrap();
    builder
        .add_member(
            MemberStableKey::LirMetadata,
            SlibMemberRole::LirMetadata { wire_schema: 1 },
            b"l".to_vec(),
        )
        .unwrap();
    builder
        .add_member(
            MemberStableKey::LinkObject {
                verifier_capability: scoop_lir_link_object_v1(),
                logical_key: plain_logical_key("unit").unwrap(),
            },
            SlibMemberRole::LinkObject {
                target_profile: TargetProfileWireId::darwin_aarch64_v1(),
                object_format: ObjectFormatId::mach_o_relocatable_v1(),
                verifier_capability: scoop_lir_link_object_v1(),
            },
            vec![0xCE, 0x11],
        )
        .unwrap();
    let data = builder.finish().unwrap();
    let view = ValidatedGraphArtifact::read(&data, &SlibDecodeLimits::strict()).unwrap();
    assert_eq!(view.members().len(), 4);
}
