//! Dispatch choices are replayed from decoded shared declarations.

use super::*;
use hir::{
    CanonicalInheritanceSlotContractsV1, InheritanceCallableDeclarationV1 as Declaration,
    InheritanceSlotContractV1, InheritanceSlotImplementationV1 as Implementation,
    InheritanceSlotTargetV1, NominalInheritanceInterfaceV1,
};

mod claims;
mod shared;

pub(super) fn check(
    core: CheckedSharedTypeFoundationV1<'_>,
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    fixtures: &Path,
) {
    for case in ["slot-selections", "slot-combinations"] {
        let root = sysroot.join(case);
        write_manifest_cone(
            &root,
            "dev.example",
            case,
            "library",
            &std::fs::read_to_string(fixtures.join(format!("{case}.scoop"))).unwrap(),
        );
        let provider = lower(sysroot, target, &root, vec![], &[core]);
        let checked = provider.check(&[core]).unwrap();

        checked.with_inheritance_graph(&[core], |_| ()).unwrap();

        claims::check(checked, core);
        shared::check(checked, core);
    }
}

fn callable(
    checked: CheckedSharedTypeFoundationV1<'_>,
    declaration: Declaration,
) -> &hir::CallableDeclarationRecordV1 {
    let id = declaration
        .origin()
        .expect("this fixture selects an ordinary source callable");
    checked
        .metadata()
        .public
        .callable_interfaces()
        .declaration(id)
        .unwrap()
}

fn replace(
    original: &InheritanceSlotContractV1,
    implementation: Implementation,
) -> InheritanceSlotContractV1 {
    InheritanceSlotContractV1::try_new(
        original.role(),
        original.slot(),
        original.declaration(),
        original.signature().clone(),
        implementation,
        original.declaration_access().clone(),
    )
    .unwrap()
}

fn reject(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
    owner: scoop_identity::PersistentExactTypeId,
    replacement: InheritanceSlotContractV1,
) -> Error {
    let mut records = checked.section().inheritance().records().to_vec();
    let record = records
        .iter_mut()
        .find(|record| record.owner() == owner)
        .unwrap();
    let mut slots = record.slots().records().to_vec();
    let replacement_key = replacement.key();
    *slots
        .iter_mut()
        .find(|slot| slot.key() == replacement_key)
        .unwrap() = replacement;
    *record = NominalInheritanceInterfaceV1::try_new(
        record.edges().clone(),
        CanonicalInheritanceSlotContractsV1::try_new(slots).unwrap(),
        record.protected_members().clone(),
        record.slot_schemas().clone(),
    )
    .unwrap();
    dispatch::reject_section(checked, core, records)
}
