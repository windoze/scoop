//! Real ordinary libraries join constructors and protected members after decode.

use super::*;
use hir::{
    CanonicalNominalInheritanceInterfacesV1, CanonicalProtectedDeclarationRefsV1,
    NominalInheritanceInterfaceV1, ProtectedDeclarationRefV1,
};
use scoop_identity::{CallableTemplateOrigin, ExactTypeKey};
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
        if name == "inheritance-members" {
            member_rejections::check(checked, core);
        }
        let metadata = checked.metadata();
        let constructors = hir::select_param_free_source_constructors(
            metadata.provider,
            metadata.public,
            metadata.identities,
        )
        .unwrap();
        let mut dump = String::new();
        for record in checked.section().inheritance().records() {
            dump.push_str(&format!("owner {}\n", record.owner()));
            let key = metadata
                .identities
                .canonical_key::<_, ExactTypeKey>(record.owner())
                .unwrap();
            let ExactTypeKey::Nominal(owner) = *key else {
                panic!("source nominal")
            };
            let nominal = metadata
                .public
                .nominal_interfaces()
                .declaration(hir::SourceNominalId::Concrete(owner))
                .unwrap();
            for id in nominal.declaration_details().constructors().values() {
                let Some(source) = constructors.get(id) else {
                    continue;
                };
                dump.push_str(&format!(
                    "  constructor {} {:?} {:?}\n",
                    id,
                    source.declared_visibility(),
                    source.parameters()
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
