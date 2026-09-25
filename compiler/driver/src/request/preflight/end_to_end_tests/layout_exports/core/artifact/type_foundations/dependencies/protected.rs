//! Complete source-only and concrete protected surfaces survive artifact bytes.

use super::*;
use hir::{
    CanonicalProtectedDeclarationInterfacesV1, ProtectedDeclarationInterfaceV1,
    ProtectedNestedNominalInterfaceV1,
};
use scoop_identity::CallableTemplateOrigin;

mod dump;
mod nested;
mod properties;
mod rejections;

pub(super) fn check(
    core: CheckedSharedTypeFoundationV1<'_>,
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    fixtures: &Path,
) {
    let name = "protected-sources";
    let root = sysroot.join(name);
    write_manifest_cone(
        &root,
        "dev.example",
        name,
        "library",
        &std::fs::read_to_string(fixtures.join(format!("{name}.scoop"))).unwrap(),
    );
    let provider = lower(sysroot, target, &root, vec![], &[core]);
    let checked = provider.check(&[core]).unwrap();
    checked.with_inheritance_graph(&[core], |_| ()).unwrap();
    rejections::check(checked, core);
    properties::check(checked, core);
    nested::check(checked, core);
    let output = dump::render(checked);
    let snapshot = fixtures.join(format!("{name}.snap"));
    if std::env::var_os("SCOOP_UPDATE_SHARED_TYPE_FOUNDATIONS").is_some() {
        std::fs::write(&snapshot, &output).unwrap();
    }
    assert_eq!(output, std::fs::read_to_string(snapshot).unwrap());
}

fn reject(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
    records: Vec<ProtectedDeclarationInterfaceV1>,
) -> Error {
    let source = checked.section();
    let candidate = CrossConeTypeSemanticsSectionV1::new(
        source.exact_facts().clone(),
        source.representation_support().clone(),
        source.inheritance().clone(),
        CanonicalProtectedDeclarationInterfacesV1::try_new(records).unwrap(),
        source.protected_source_interfaces().clone(),
        source.protected_defaults().clone(),
        source.definition_sources().clone(),
        source.selected().clone(),
    );
    candidate
        .validate_shared_foundation(checked.metadata(), &[core])
        .unwrap()
        .with_inheritance_graph(&[core], |_| ())
        .expect_err("protected source payloads must agree with shared declarations")
}

fn replace_nested(
    source: &ProtectedNestedNominalInterfaceV1,
    interface: hir::ProtectedNestedSourceInterfaceV1,
) -> ProtectedNestedNominalInterfaceV1 {
    hir::ProtectedNestedNominalInterfaceV1::try_new(
        source.declaration(),
        source.declaration_access().clone(),
        hir::ProtectedNestedNominalPayloadV1::try_new(
            source.declaration(),
            interface,
            source.payload().support(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn nested_interface(
    source: &hir::ProtectedNestedSourceInterfaceV1,
    modality: hir::NominalInheritanceModalityV1,
    children: hir::CanonicalNestedNominalRefsV1,
    support: hir::CanonicalNestedSourceSupportV1,
) -> hir::ProtectedNestedSourceInterfaceV1 {
    hir::ProtectedNestedSourceInterfaceV1::try_new(
        source.kind(),
        modality,
        source.type_parameters().clone(),
        source.supertypes().clone(),
        source.constructors().clone(),
        source.members().clone(),
        children,
        source.source_shape().clone(),
        support,
    )
    .unwrap()
}
