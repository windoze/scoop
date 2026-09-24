use super::*;

pub(super) fn check(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
) {
    let table = checked.metadata().public.nominal_interfaces();
    let class = table
        .all_records()
        .find(|record| {
            matches!(record.declaration_details().dispatch_order(),
        NominalDispatchOrderV1::Class { slots } if !slots.is_empty())
        })
        .unwrap();
    let mut missing = class.declaration_details().dispatch_order().clone();
    let NominalDispatchOrderV1::Class { slots } = &mut missing else {
        unreachable!("selected a class")
    };
    slots.pop();
    assert!(matches!(
        reject(checked, core, class, missing),
        Error::DispatchDeclarations(_)
    ));

    let interface = table
        .all_records()
        .find(|record| {
            matches!(record.declaration_details().dispatch_order(),
        NominalDispatchOrderV1::Interface { parents, .. } if parents.len() > 1)
        })
        .unwrap();
    let mut reversed = interface.declaration_details().dispatch_order().clone();
    let NominalDispatchOrderV1::Interface { parents, .. } = &mut reversed else {
        unreachable!("selected an interface")
    };
    parents.reverse();
    assert!(matches!(
        reject(checked, core, interface, reversed),
        Error::SlotSchemas(_)
    ));

    let mut unsuppressed = interface.declaration_details().dispatch_order().clone();
    let NominalDispatchOrderV1::Interface { members, .. } = &mut unsuppressed else {
        unreachable!("selected an interface")
    };
    let member = members
        .iter_mut()
        .find(|member| !member.overrides().is_empty())
        .unwrap();
    *member =
        hir::InterfaceSourceMemberV1::new(member.slot(), hir::CanonicalPersistentIdsV1::empty());
    assert!(matches!(
        reject(checked, core, interface, unsuppressed),
        Error::SlotSchemas(_)
    ));
}

fn reject(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
    nominal: &NominalInterfaceRecordV1,
    order: NominalDispatchOrderV1,
) -> Error {
    let source = checked.metadata().public;
    let details = nominal.declaration_details();
    let replacement = NominalInterfaceRecordV1::try_new(
        nominal.declaration(),
        nominal.kind(),
        nominal.type_parameters().clone(),
        nominal.exact_supertypes().clone(),
        nominal.constructors().clone(),
        nominal.members().clone(),
        nominal.nested_bindings().clone(),
        nominal.source_shape().clone(),
        hir::NominalDeclarationDetailsV1::new(
            details.modality(),
            details.declared_visibility(),
            details.constructors().clone(),
            details.members().clone(),
            details.children().clone(),
            order,
        ),
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
    );
    let mut metadata = checked.metadata();
    metadata.public = &public;
    checked
        .section()
        .validate_shared_foundation(metadata, &[core], &mut meter())
        .unwrap()
        .with_inheritance_graph(&[core], &mut meter(), |_, _| ())
        .expect_err("shared declaration changes must be reflected in the emitted schema")
}
