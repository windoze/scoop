use scoop_identity::{ConeIdentity, NormalizedSourcePath};
use scoop_slib::{
    ArtifactManifestSummaryV1, ConeKind, ConeRecord, ConeSourceForm, DependencyRecord,
    read_artifact_manifest_summary,
};
use scoop_wire::sha256;

use super::*;

#[test]
fn feedback_selection_changes_compile_key_without_changing_semantic_identity() {
    let plain = input(1);
    let first = plain
        .clone()
        .with_profile_use(Some(PathBuf::from("training-a/profile.json")));
    let second = plain
        .clone()
        .with_profile_use(Some(PathBuf::from("training-b/profile.json")));
    assert_ne!(plain.key().unwrap(), first.key().unwrap());
    assert_ne!(first.key().unwrap(), second.key().unwrap());
    assert_eq!(plain.current_semantic(), first.current_semantic());
    assert_eq!(plain.dependencies(), first.dependencies());
}

#[test]
fn optimization_changes_only_the_current_compile_key() {
    let debug = input(1);
    let release = debug
        .clone()
        .with_optimization(scoop_lir::OptimizationMode::Release);
    assert_ne!(debug.key().unwrap(), release.key().unwrap());
    assert_eq!(debug.dependencies(), release.dependencies());
    assert_eq!(debug.current_semantic(), release.current_semantic());
    assert_eq!(debug.backend, release.backend);
}

#[test]
fn export_ir_changes_compile_reuse_without_changing_source_identity() {
    let default = input(1);
    let off = default
        .clone()
        .with_export_ir(scoop_protocol::IrTemplateExport::Off);
    let full = default
        .clone()
        .with_export_ir(scoop_protocol::IrTemplateExport::Full);
    assert_ne!(default.key().unwrap(), off.key().unwrap());
    assert_ne!(default.key().unwrap(), full.key().unwrap());
    assert_ne!(off.key().unwrap(), full.key().unwrap());
    assert_eq!(off.current_semantic(), full.current_semantic());
    assert_eq!(off.dependencies(), full.dependencies());
    assert_eq!(off.cone(), full.cone());
}

#[test]
fn debug_info_changes_code_reuse_without_changing_semantic_dependencies() {
    let none = input(1);
    let lines = none
        .clone()
        .with_debug_info(scoop_lir::DebugInfoLevel::LineTables);
    assert_ne!(none.key().unwrap(), lines.key().unwrap());
    let full = none
        .clone()
        .with_debug_info(scoop_lir::DebugInfoLevel::Full);
    assert_ne!(lines.key().unwrap(), full.key().unwrap());
    assert_ne!(none.key().unwrap(), full.key().unwrap());
    assert_eq!(none.dependencies(), lines.dependencies());
    assert_eq!(none.current_semantic(), lines.current_semantic());
    assert_eq!(none.backend, lines.backend);
    assert_eq!(none.optimization, lines.optimization);
}

#[test]
fn test_purpose_and_source_roles_are_independent_key_inputs() {
    let production = input(1);
    let test = production.clone().with_purpose(CompilationPurpose::Test);
    assert_ne!(production.key().unwrap(), test.key().unwrap());
    let mut changed_role = test.clone();
    changed_role.sources[0].role = SourceRole::Test;
    assert_ne!(test.key().unwrap(), changed_role.key().unwrap());
}

fn input(source_modified: u64) -> ConeCompileCacheInputV1 {
    let selection = scoop_target_data::DARWIN_AARCH64_LLVM_22_1;
    let source = SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("src/core.scoop").unwrap(),
    );
    ConeCompileCacheInputV1::new(
        ConeRecord::new(
            ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        CurrentConeSemanticProjectionV1::Manifest {
            input_root: PathBuf::from("/source").into_boxed_path(),
            coordinate: ConeCoordinate::reserved_core(),
            requested_kind: RequestedConeKind::Library,
            dependencies: Vec::new(),
            sources: SourceSelection::Default,
            tests: None,
            native: Box::new(sha256(b"native-objects")),
        },
        vec![SourceCacheInputV1 {
            source,
            path: PathBuf::from("/workspace/core/src/core.scoop"),
            byte_length: 10,
            modified: UNIX_EPOCH + std::time::Duration::from_secs(source_modified),
            role: SourceRole::Production,
        }],
        Vec::new(),
        selection.target().fingerprint().unwrap(),
        selection.backend().fingerprint().unwrap(),
    )
}

fn dependency_summaries() -> (ArtifactManifestSummaryV1, ArtifactManifestSummaryV1) {
    let selection = scoop_target_data::DARWIN_AARCH64_LLVM_22_1;
    let core = crate::test_artifacts::manifest_archive(
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
        scoop_slib::ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        ConeRecord::new(
            ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        "cache-key-core",
        Vec::new(),
    );
    let core = read_artifact_manifest_summary(core.as_bytes(), selection).unwrap();
    let semantic = core.semantic_fingerprints();
    let dependency = crate::test_artifacts::manifest_archive(
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
        scoop_slib::ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        ConeRecord::new(
            ConeCoordinate::new("dev.example", "dependency", "1.0.0").unwrap(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        "cache-key-dependency",
        vec![
            DependencyRecord::new(
                core.cone().coordinate().clone(),
                semantic.hir(),
                semantic.mir(),
                semantic.lir(),
            )
            .unwrap(),
        ],
    );
    let dependency = read_artifact_manifest_summary(dependency.as_bytes(), selection).unwrap();
    (core, dependency)
}

fn rich_input() -> ConeCompileCacheInputV1 {
    let selection = scoop_target_data::DARWIN_AARCH64_LLVM_22_1;
    let (core, dependency) = dependency_summaries();
    let current_coordinate = ConeCoordinate::new("dev.example", "current", "1.0.0").unwrap();
    let current_identity = current_coordinate.identity().unwrap();
    let source = SourceIdentity::new(
        current_identity,
        NormalizedSourcePath::new("src/main.scoop").unwrap(),
    );
    let semantic = core.semantic_fingerprints();
    ConeCompileCacheInputV1::new(
        ConeRecord::new(
            current_coordinate.clone(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        CurrentConeSemanticProjectionV1::Manifest {
            input_root: PathBuf::from("/source").into_boxed_path(),
            coordinate: current_coordinate,
            requested_kind: RequestedConeKind::Library,
            dependencies: vec![dependency.cone().coordinate().clone()],
            sources: SourceSelection::Default,
            tests: None,
            native: Box::new(sha256(b"native-objects")),
        },
        vec![SourceCacheInputV1 {
            source,
            path: PathBuf::from("/workspace/current/src/main.scoop"),
            byte_length: 23,
            modified: UNIX_EPOCH,
            role: SourceRole::Production,
        }],
        vec![CompileDependencyInputV1::new(
            core.cone().clone(),
            semantic.hir(),
            semantic.mir(),
            semantic.lir(),
            CodeFingerprint::from_array([10; 32]),
        )],
        selection.target().fingerprint().unwrap(),
        selection.backend().fingerprint().unwrap(),
    )
}

#[track_caller]
fn assert_mutation_misses(
    baseline: &ConeCompileCacheInputV1,
    mutate: impl FnOnce(&mut ConeCompileCacheInputV1),
) {
    let baseline_key = baseline.key().unwrap();
    let mut changed = baseline.clone();
    mutate(&mut changed);
    assert_ne!(baseline_key, changed.key().unwrap());
}

#[test]
fn source_metadata_changes_the_compile_key() {
    assert_ne!(input(1).key().unwrap(), input(2).key().unwrap());
}

#[test]
fn dependency_code_changes_the_cache_key_without_changing_semantic_requirements() {
    let original = rich_input();
    let mut changed = original.clone();
    changed.dependencies[0].code = CodeFingerprint::from_array([11; 32]);
    assert_ne!(original.key().unwrap(), changed.key().unwrap());
    assert_eq!(
        original.dependencies[0].hir(),
        changed.dependencies[0].hir()
    );
    assert_eq!(
        original.dependencies[0].mir(),
        changed.dependencies[0].mir()
    );
    assert_eq!(
        original.dependencies[0].lir(),
        changed.dependencies[0].lir()
    );
}

#[test]
fn every_currently_variable_cache_key_dimension_misses_independently() {
    let baseline = rich_input();
    let (_, alternate_dependency) = dependency_summaries();
    assert_mutation_misses(&baseline, |input| {
        let CurrentConeSemanticProjectionV1::Manifest { input_root, .. } =
            &mut input.current_semantic
        else {
            unreachable!()
        };
        *input_root = PathBuf::from("/other-alias").into_boxed_path();
    });

    assert_mutation_misses(&baseline, |input| {
        input.cone = ConeRecord::new(
            ConeCoordinate::new("dev.example", "other", "1.0.0").unwrap(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap();
    });
    assert_mutation_misses(&baseline, |input| {
        let CurrentConeSemanticProjectionV1::Manifest { coordinate, .. } =
            &mut input.current_semantic
        else {
            unreachable!()
        };
        *coordinate = ConeCoordinate::new("dev.example", "current", "2.0.0").unwrap();
    });
    assert_mutation_misses(&baseline, |input| {
        let CurrentConeSemanticProjectionV1::Manifest { requested_kind, .. } =
            &mut input.current_semantic
        else {
            unreachable!()
        };
        *requested_kind = RequestedConeKind::Executable;
    });
    assert_mutation_misses(&baseline, |input| {
        let CurrentConeSemanticProjectionV1::Manifest { dependencies, .. } =
            &mut input.current_semantic
        else {
            unreachable!()
        };
        dependencies.clear();
    });
    assert_mutation_misses(&baseline, |input| {
        let CurrentConeSemanticProjectionV1::Manifest { sources, .. } = &mut input.current_semantic
        else {
            unreachable!()
        };
        let manifest = scoop_toolchain::parse_cone_manifest(
            "schema = 1\n[[sources]]\npath = 'src'\n[cone]\ngroup = 'test'\nname = 'selection'\nversion = '1.0.0'\nkind = 'library'\n",
        ).unwrap();
        *sources = manifest.semantic().sources().clone();
    });
    assert_mutation_misses(&baseline, |input| {
        input.current_semantic = CurrentConeSemanticProjectionV1::SingleFile;
    });
    assert_mutation_misses(&baseline, |input| {
        input.sources[0].source = SourceIdentity::new(
            input.cone.identity(),
            NormalizedSourcePath::new("src/other.scoop").unwrap(),
        );
    });
    assert_mutation_misses(&baseline, |input| {
        input.sources[0].modified += std::time::Duration::from_secs(1);
    });
    assert_mutation_misses(&baseline, |input| {
        input.sources[0].byte_length += 1;
    });
    assert_mutation_misses(&baseline, |input| {
        input.sources[0].path = PathBuf::from("/other/current/src/main.scoop");
    });
    assert_mutation_misses(&baseline, |input| {
        input.dependencies[0].cone = alternate_dependency.cone().clone();
    });
    assert_mutation_misses(&baseline, |input| {
        input.dependencies[0].hir = HirFingerprint::from_array([7; 32]);
    });
    assert_mutation_misses(&baseline, |input| {
        input.dependencies[0].mir = MirFingerprint::from_array([8; 32]);
    });
    assert_mutation_misses(&baseline, |input| {
        input.dependencies[0].lir = LirFingerprint::from_array([9; 32]);
    });
}

#[test]
fn instrumentation_changes_code_reuse_without_changing_language_identity() {
    let ordinary = input(1);
    let instrumented = ordinary
        .clone()
        .with_instrumentation(scoop_profile::Instrumentation::Generate);
    assert_ne!(ordinary.key().unwrap(), instrumented.key().unwrap());
    assert_eq!(ordinary.current_semantic(), instrumented.current_semantic());
    assert_eq!(ordinary.cone(), instrumented.cone());
}
