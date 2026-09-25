use std::path::Path;

use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_manifest::ManifestRootLocator;
use scoop_slib::{
    ArtifactDistributionClassV1, ConeKind, ConeSourceForm, DecodedSlibEnvelope,
    SingleConeProductionOutputV1,
};
use scoop_wire::DecodeLimits;

use super::*;
use crate::{ExplicitDependencyInputs, HostArtifactLocator};

mod compile_view_pairing;
mod cross_cone;
mod default_call_domains;
mod default_callable_access;
mod default_nested_identities;
mod default_reference_closure;
mod default_type_access;
mod default_value_access;
mod dependency_preflight;
mod executable_callables;
mod executable_type_sites;
mod heap_zst;
mod image_dependencies;
mod layout_exports;
mod nominal_signatures;
mod publication;
mod runtime_layout_gates;
mod setter_domains;
mod shape_materialization;
mod shared_lir_selection;
mod shared_nominal_declarations;
mod shared_source_closure;
mod source_only_nominals;
mod support;

use support::*;

#[test]
fn formal_pipeline_publishes_manifest_library_and_executable_artifacts() {
    let Some(target) = resolved_target() else {
        return;
    };
    let sysroot = tempfile::tempdir().unwrap();
    let core = bootstrap_core(sysroot.path(), &target);
    assert_graph_dependencies(&core, &target, &[]);

    let library_root = sysroot.path().join("library");
    write_manifest_cone(
        &library_root,
        "dev.example",
        "stage3.library",
        "library",
        "fun answer(): Long = 42\n",
    );
    let library = build_manifest(
        sysroot.path(),
        &target,
        &library_root,
        &sysroot.path().join("output/library.slib"),
    );
    let library_validation = library.artifact().validation();
    assert_eq!(
        library_validation.coordinate(),
        &ConeCoordinate::new("dev.example", "stage3.library", "0.1.0").unwrap()
    );
    assert_eq!(library_validation.kind(), ConeKind::Library);
    assert_eq!(library_validation.source_form(), ConeSourceForm::Manifest);
    assert_eq!(
        library_validation.link_summary().distribution(),
        ArtifactDistributionClassV1::DistributableCone
    );
    assert_eq!(
        library_validation.link_summary().output(),
        &SingleConeProductionOutputV1::Library
    );
    assert!(library_validation.link_summary().link_object_count() > 0);
    assert_graph_dependencies(&library, &target, &[ConeIdentity::CORE]);

    let executable_root = sysroot.path().join("executable");
    write_manifest_cone(
        &executable_root,
        "dev.example",
        "stage3.executable",
        "executable",
        "fun main() {\n    val message = \"stage3\"\n    val answer = stage3CoreAnswer()\n}\n",
    );
    let executable = build_manifest(
        sysroot.path(),
        &target,
        &executable_root,
        &sysroot.path().join("output/executable.slib"),
    );
    let executable_validation = executable.artifact().validation();
    assert_eq!(executable_validation.kind(), ConeKind::Executable);
    assert_eq!(
        executable_validation.source_form(),
        ConeSourceForm::Manifest
    );
    assert_eq!(
        executable_validation.link_summary().distribution(),
        ArtifactDistributionClassV1::DistributableCone
    );
    assert!(matches!(
        executable_validation.link_summary().output(),
        SingleConeProductionOutputV1::Executable(_)
    ));
    assert!(executable_validation.link_summary().link_object_count() > 0);
    assert_graph_dependencies(&executable, &target, &[ConeIdentity::CORE]);
}

#[test]
fn formal_pipeline_is_byte_reproducible_for_every_stage3_input_form() {
    let Some(target) = resolved_target() else {
        return;
    };
    let workspace = tempfile::tempdir().unwrap();
    let first_sysroot = workspace.path().join("checkout-a/sysroot");
    let second_sysroot = workspace.path().join("checkout-b/sysroot");
    let first_core = bootstrap_core(&first_sysroot, &target);
    let second_core = bootstrap_core(&second_sysroot, &target);
    assert_artifacts_equal(&first_core, &second_core);

    let first_root = first_sysroot.join("cones/library");
    let second_root = second_sysroot.join("other/cones/library");
    for root in [&first_root, &second_root] {
        write_manifest_cone(
            root,
            "dev.example",
            "stage3.reproducible",
            "library",
            "fun answer(): Long = 42\n",
        );
    }
    std::fs::write(first_root.join("src/z.scoop"), "fun last(): Long = 2\n").unwrap();
    std::fs::write(first_root.join("src/a.scoop"), "fun first(): Long = 1\n").unwrap();
    std::fs::write(second_root.join("src/a.scoop"), "fun first(): Long = 1\n").unwrap();
    std::fs::write(second_root.join("src/z.scoop"), "fun last(): Long = 2\n").unwrap();

    let first = build_manifest(
        &first_sysroot,
        &target,
        &first_root,
        &first_sysroot.join("output-a/library.slib"),
    );
    let second = build_manifest(
        &second_sysroot,
        &target,
        &second_root,
        &second_sysroot.join("output-b/library.slib"),
    );
    assert_artifacts_equal(&first, &second);

    let first_root = first_sysroot.join("cones/executable");
    let second_root = second_sysroot.join("other/cones/executable");
    for root in [&first_root, &second_root] {
        write_manifest_cone(
            root,
            "dev.example",
            "stage3.reproducible-executable",
            "executable",
            "fun main() { val message = \"stage3\" }\n",
        );
    }
    let first = build_manifest(
        &first_sysroot,
        &target,
        &first_root,
        &first_sysroot.join("output-a/executable.slib"),
    );
    let second = build_manifest(
        &second_sysroot,
        &target,
        &second_root,
        &second_sysroot.join("output-b/executable.slib"),
    );
    assert_artifacts_equal(&first, &second);

    let first_source = first_sysroot.join("inputs/first-name.scoop");
    let second_source = second_sysroot.join("other/inputs/second-name.scoop");
    std::fs::create_dir_all(first_source.parent().unwrap()).unwrap();
    std::fs::create_dir_all(second_source.parent().unwrap()).unwrap();
    std::fs::write(&first_source, "fun main() {}\n").unwrap();
    std::fs::write(&second_source, "fun main() {}\n").unwrap();
    let first = build_single_file(
        &first_sysroot,
        &target,
        &first_source,
        &first_sysroot.join("output-a/single-file.slib"),
    );
    let second = build_single_file(
        &second_sysroot,
        &target,
        &second_source,
        &second_sysroot.join("output-b/single-file.slib"),
    );
    assert_artifacts_equal(&first, &second);
}

#[test]
fn formal_pipeline_preserves_source_extern_without_attempting_final_link() {
    let Some(target) = resolved_target() else {
        return;
    };
    let sysroot = tempfile::tempdir().unwrap();
    bootstrap_core(sysroot.path(), &target);

    let executable_root = sysroot.path().join("source-extern");
    write_manifest_cone(
        &executable_root,
        "dev.example",
        "stage3.source-extern",
        "executable",
        r#"@Extern(lib = "stage3_library_that_does_not_exist", name = "stage3_add")
fun stage3Add(left: Int, right: Int): Int

fun main() {
    @Unsafe {
        val result = stage3Add(1, 2)
    }
}
"#,
    );
    let artifact = build_manifest(
        sysroot.path(),
        &target,
        &executable_root,
        &sysroot.path().join("output/source-extern.slib"),
    );

    assert!(artifact.artifact().path().is_file());
    assert_eq!(
        artifact.artifact().validation().kind(),
        ConeKind::Executable
    );
    assert!(matches!(
        artifact.artifact().validation().link_summary().output(),
        SingleConeProductionOutputV1::Executable(_)
    ));
    assert_graph_dependencies(&artifact, &target, &[ConeIdentity::CORE]);
}

#[test]
fn formal_pipeline_preserves_combined_stage3_language_features() {
    let Some(target) = resolved_target() else {
        return;
    };
    let sysroot = tempfile::tempdir().unwrap();
    bootstrap_core(sysroot.path(), &target);

    let executable_root = sysroot.path().join("combined");
    write_manifest_cone(
        &executable_root,
        "dev.example",
        "stage3.combined",
        "executable",
        r#"val initializedMessage: String = makeMessage()

fun makeMessage(): String = "stage3"

fun apply(operation: () -> String): String = operation()

fun main() {
    val render: () -> String = { initializedMessage }
    val rendered = apply(render)
    val answer = stage3CoreAnswer()
    stage3CoreFailure()
}
"#,
    );
    let artifact = build_manifest(
        sysroot.path(),
        &target,
        &executable_root,
        &sysroot.path().join("output/combined.slib"),
    );

    assert!(artifact.artifact().path().is_file());
    assert_eq!(
        artifact.artifact().validation().kind(),
        ConeKind::Executable
    );
    assert!(matches!(
        artifact.artifact().validation().link_summary().output(),
        SingleConeProductionOutputV1::Executable(_)
    ));
    assert_graph_dependencies(&artifact, &target, &[ConeIdentity::CORE]);
}
