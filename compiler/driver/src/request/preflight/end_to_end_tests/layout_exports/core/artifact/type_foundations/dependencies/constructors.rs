//! Real ordinary libraries join constructors and protected members after decode.

use super::*;
use hir::{
    CanonicalNominalInheritanceInterfacesV1, CanonicalProtectedDeclarationRefsV1,
    NominalInheritanceInterfaceV1, ProtectedDeclarationRefV1,
};
use scoop_identity::CallableTemplateOrigin;
mod member_rejections;

pub(super) fn check(
    core: CheckedSharedTypeFoundationV1<'_>,
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    fixtures: &Path,
) {
    let root = sysroot.join("inheritance-members");
    write_manifest_cone(
        &root,
        "dev.example",
        "inheritance-members",
        "library",
        &std::fs::read_to_string(fixtures.join("inheritance-members.scoop")).unwrap(),
    );
    let provider = lower(sysroot, target, &root, vec![], &[core]);
    let checked = provider.check(&[core]).unwrap();
    checked.with_inheritance_graph(&[core], |_| ()).unwrap();
    member_rejections::check(checked, core);
}

fn replace(
    record: &NominalInheritanceInterfaceV1,
    members: CanonicalProtectedDeclarationRefsV1,
) -> NominalInheritanceInterfaceV1 {
    NominalInheritanceInterfaceV1::try_new(
        record.edges().clone(),
        record.slots().clone(),
        members,
        record.slot_schemas().clone(),
    )
    .unwrap()
}

fn reject(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
    records: Vec<NominalInheritanceInterfaceV1>,
) -> Error {
    let source = checked.section();
    let candidate = CrossConeTypeSemanticsSectionV1::new(
        source.exact_facts().clone(),
        source.representation_support().clone(),
        CanonicalNominalInheritanceInterfacesV1::try_new(records).unwrap(),
        source.selected().clone(),
    );
    candidate
        .validate_shared_foundation(checked.metadata(), &[core])
        .unwrap()
        .with_inheritance_graph(&[core], |_| ())
        .expect_err("decoded inheritance must agree with shared declaration metadata")
}
