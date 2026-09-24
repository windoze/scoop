//! Dispatch choices are replayed from decoded shared declarations.

use super::*;
use hir::{
    CanonicalInheritanceSlotContractsV1, InheritanceCallableDeclarationV1 as Declaration,
    InheritanceSlotContractV1, InheritanceSlotImplementationV1 as Implementation,
    InheritanceSlotTargetV1, NominalInheritanceInterfaceV1,
};
use scoop_identity::{CallableTemplateOrigin, SourceDeclarationKey};

mod claims;
mod snapshot;

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
        let mut measured = meter();
        checked
            .with_inheritance_graph(&[core], &mut measured, |_, _| ())
            .unwrap();
        let mut bounded = scoop_wire::BudgetMeter::new(DecodeLimits {
            validation_work_units: measured.usage().validation_work_units,
            ..DecodeLimits::default()
        });
        checked
            .with_inheritance_graph(&[core], &mut bounded, |_, _| ())
            .unwrap();
        assert!(matches!(
            checked.with_inheritance_graph(&[core], &mut bounded, |_, _| ()),
            Err(Error::Resource(error))
                if matches!(error.kind(), scoop_wire::WireErrorKind::LimitExceeded {
                    resource: scoop_wire::ResourceKind::ValidationWorkUnits, ..
                })
        ));
        claims::check(checked, core);
        let dump = snapshot::render(checked);
        let golden = fixtures.join(format!("{case}.snap"));
        if std::env::var_os("SCOOP_UPDATE_SHARED_TYPE_FOUNDATIONS").is_some() {
            std::fs::write(&golden, &dump).unwrap();
        }
        assert_eq!(dump, std::fs::read_to_string(golden).unwrap());
    }
}

fn callable(
    checked: CheckedSharedTypeFoundationV1<'_>,
    declaration: Declaration,
) -> &hir::CallableDeclarationRecordV1 {
    let id = match declaration {
        Declaration::Function(id) => CallableTemplateOrigin::Function(id),
        Declaration::Getter(id) | Declaration::Setter(id) => CallableTemplateOrigin::Accessor(id),
    };
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
        original.slot(),
        original.declaration_owner(),
        original.declaration(),
        original.signature().clone(),
        original.domain().clone(),
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
    let replacement_slot = replacement.slot();
    *slots
        .iter_mut()
        .find(|slot| slot.slot() == replacement_slot)
        .unwrap() = replacement;
    *record = NominalInheritanceInterfaceV1::try_new(
        record.edges().clone(),
        record.domains().clone(),
        record.constructors().clone(),
        CanonicalInheritanceSlotContractsV1::try_new(slots).unwrap(),
        record.protected_members().clone(),
        record.slot_schemas().clone(),
    )
    .unwrap();
    dispatch::reject_section(checked, core, records)
}
