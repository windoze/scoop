//! Real ordinary libraries join constructors and protected members after decode.

use super::*;
use hir::{
    CanonicalInheritanceConstructorsV1, CanonicalNominalInheritanceInterfacesV1,
    CanonicalProtectedDeclarationInterfacesV1, CanonicalProtectedDeclarationRefsV1,
    InheritanceConstructorInterfaceV1, NominalInheritanceInterfaceV1,
    NominalSourceCallablePayloadV1, NominalSupportConstructorInterfaceV1,
    ProtectedDeclarationInterfaceV1, ProtectedDeclarationRefV1,
};
use scoop_identity::CallableTemplateOrigin;

mod constructor_rejections;
mod member_rejections;

pub(super) fn check(
    core: CheckedSharedTypeFoundationV1<'_>,
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    fixtures: &Path,
) {
    for name in ["constructors", "inheritance-members"] {
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
        constructor_rejections::check(checked, core);
        if name == "inheritance-members" {
            member_rejections::check(checked, core);
        }
        let mut dump = String::new();
        for record in checked.section().inheritance().records() {
            dump.push_str(&format!("owner {}\n", record.owner()));
            for constructor in record.constructors().records() {
                let source = constructor.source();
                dump.push_str(&format!(
                    "  constructor {} {:?} {:?}\n",
                    constructor.declaration(),
                    source.declaration_access().declared_visibility(),
                    source.payload().parameters(),
                ));
            }
            for reference in record.protected_members().values() {
                dump.push_str(&format!("  protected {reference:?}\n"));
            }
        }
        let snapshot = fixtures.join(format!("{name}.snap"));
        if std::env::var_os("SCOOP_UPDATE_SHARED_TYPE_FOUNDATIONS").is_some() {
            std::fs::write(&snapshot, &dump).unwrap();
        }
        assert_eq!(dump, std::fs::read_to_string(snapshot).unwrap());
    }
}

fn replace(
    record: &NominalInheritanceInterfaceV1,
    constructors: CanonicalInheritanceConstructorsV1,
    members: CanonicalProtectedDeclarationRefsV1,
) -> NominalInheritanceInterfaceV1 {
    NominalInheritanceInterfaceV1::try_new(
        record.edges().clone(),
        record.domains().clone(),
        constructors,
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
    protected: CanonicalProtectedDeclarationInterfacesV1,
) -> Error {
    let source = checked.section();
    let candidate = CrossConeTypeSemanticsSectionV1::new(
        source.exact_facts().clone(),
        source.representation_support().clone(),
        CanonicalNominalInheritanceInterfacesV1::try_new(records).unwrap(),
        protected,
        source.selected().clone(),
    );
    candidate
        .validate_shared_foundation(checked.metadata(), &[core])
        .unwrap()
        .with_inheritance_graph(&[core], |_| ())
        .expect_err("decoded inheritance must agree with shared declaration metadata")
}
