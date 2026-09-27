use super::*;
use hir::{
    CanonicalInheritanceSlotSchemasV1, CanonicalNominalInheritanceInterfacesV1,
    InheritanceSlotSchemaRoleV1, InheritanceSlotSchemaV1, NominalDispatchOrderV1,
    NominalInheritanceInterfaceV1, NominalInterfaceRecordV1,
};

mod declarations;
mod schemas;

pub(super) fn check(
    core: CheckedSharedTypeFoundationV1<'_>,
    sysroot: &Path,
    target: &scoop_toolchain::ResolvedTargetProfile,
    fixtures: &Path,
) {
    let root = sysroot.join("dispatch-order");
    write_manifest_cone(
        &root,
        "dev.example",
        "dispatch-order",
        "library",
        &std::fs::read_to_string(fixtures.join("dispatch-order.scoop")).unwrap(),
    );
    let mut provider = lower(sysroot, target, &root, vec![], &[core]);
    let checked = provider.check(&[core]).unwrap();
    checked.with_inheritance_graph(&[core], |_| ()).unwrap();
    schemas::check(checked, core);
    declarations::check(checked, core);

    let mut dump = String::new();
    let mut orders = Vec::new();
    for nominal in checked.metadata().public.nominal_interfaces().all_records() {
        let order = nominal.declaration_details().dispatch_order();
        orders.push(order.clone());
        dump.push_str(&format!(
            "nominal {:?} {:?}\n",
            nominal.declaration(),
            nominal.kind()
        ));
        match order {
            NominalDispatchOrderV1::NonVirtual => dump.push_str("  non-virtual\n"),
            NominalDispatchOrderV1::Class { slots } => {
                for slot in slots {
                    dump.push_str(&format!("  virtual {slot}\n"));
                }
            }
            NominalDispatchOrderV1::Interface { parents, members } => {
                for parent in parents {
                    dump.push_str(&format!("  parent {parent:?}\n"));
                }
                for member in members {
                    dump.push_str(&format!("  member {}\n", member.slot()));
                    for overridden in member.overrides().values() {
                        dump.push_str(&format!("    overrides {overridden}\n"));
                    }
                }
            }
        }
    }
    for record in checked.section().inheritance().records() {
        for schema in record.slot_schemas().records() {
            dump.push_str(&format!("schema {} {:?}\n", record.owner(), schema.role()));
            for (position, slot) in schema.slots().iter().enumerate() {
                dump.push_str(&format!("  {position}: {slot}\n"));
            }
        }
    }
    let path = fixtures.join("dispatch-order.snap");
    if std::env::var_os("SCOOP_UPDATE_SHARED_TYPE_FOUNDATIONS").is_some() {
        std::fs::write(&path, &dump).unwrap();
    }
    assert_eq!(dump, std::fs::read_to_string(path).unwrap());
    for order in &orders {
        assert_eq!(&provider.resolve_dispatch_order(order).unwrap(), order);
    }
    let mut duplicate = orders
        .into_iter()
        .find(|order| matches!(order, NominalDispatchOrderV1::Class { slots } if !slots.is_empty()))
        .unwrap();
    let NominalDispatchOrderV1::Class { slots } = &mut duplicate else {
        unreachable!("selected a class order above")
    };
    let slot = slots[0];
    slots.push(slot);
    assert!(matches!(provider.resolve_dispatch_order(&duplicate),
        Err(hir::NominalDispatchOrderResolutionError::Order(hir::NominalDispatchOrderError::DuplicateSlot(actual))) if actual == slot));
}

pub(super) fn reject_section(
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
        .expect_err("schema must agree with the actual shared declaration order")
}

pub(super) fn reject_nominal(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
    nominal: &NominalInterfaceRecordV1,
    details: hir::NominalDeclarationDetailsV1,
) -> Error {
    let source = checked.metadata().public;
    let replacement = NominalInterfaceRecordV1::try_new(
        nominal.declaration(),
        nominal.kind(),
        nominal.type_parameters().clone(),
        nominal.exact_supertypes().clone(),
        nominal.constructors().clone(),
        nominal.members().clone(),
        nominal.nested_bindings().clone(),
        nominal.source_shape().clone(),
        details,
    )
    .unwrap();
    let replace = |record: &NominalInterfaceRecordV1| {
        if record.declaration() == nominal.declaration() {
            replacement.clone()
        } else {
            record.clone()
        }
    };
    let table = hir::CanonicalNominalInterfacesV1::with_support(
        source
            .nominal_interfaces()
            .records()
            .iter()
            .map(replace)
            .collect(),
        source
            .nominal_interfaces()
            .support_records()
            .iter()
            .map(replace)
            .collect(),
    )
    .unwrap();
    let public = hir::CrossConeHirInterfaceSectionV1::new(
        source.public_bindings().clone(),
        table,
        source.callable_interfaces().clone(),
        source.property_interfaces().clone(),
        source.type_aliases().clone(),
        source.source_interfaces().clone(),
        source.default_templates().clone(),
        source.constants().clone(),
        source.definition_sources().clone(),
        source.external_references().clone(),
        source.generic_callable_bodies().clone(),
        source.generic_initializations().clone(),
    );
    let mut metadata = checked.metadata();
    metadata.public = &public;
    checked
        .section()
        .validate_shared_foundation(metadata, &[core])
        .unwrap()
        .with_inheritance_graph(&[core], |_| ())
        .expect_err("shared declaration changes must be reflected in the emitted schema")
}
