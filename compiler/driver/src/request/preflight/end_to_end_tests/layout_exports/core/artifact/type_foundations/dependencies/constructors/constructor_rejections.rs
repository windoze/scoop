use super::*;

pub(super) fn check(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
) {
    let records = checked.section().inheritance().records();
    let index = records
        .iter()
        .position(|record| !record.constructors().records().is_empty())
        .unwrap();
    let record = &records[index];
    let constructor = &record.constructors().records()[0];
    let id = constructor.declaration();
    let table = || checked.section().protected_declarations().clone();

    let mut missing = record.constructors().records().to_vec();
    missing.remove(0);
    let mut candidates = records.to_vec();
    candidates[index] = replace(
        record,
        CanonicalInheritanceConstructorsV1::try_new(missing).unwrap(),
        record.protected_members().clone(),
    );
    assert!(
        matches!(reject(checked, core, candidates, table()), Error::ConstructorInventory(owner) if owner == record.owner())
    );

    let other = records
        .iter()
        .filter(|other| other.owner() != record.owner())
        .flat_map(|other| other.constructors().records())
        .next()
        .unwrap();
    let mut extra = record.constructors().records().to_vec();
    extra.push(other.clone());
    let mut candidates = records.to_vec();
    candidates[index] = replace(
        record,
        CanonicalInheritanceConstructorsV1::try_new(extra).unwrap(),
        record.protected_members().clone(),
    );
    assert!(
        matches!(reject(checked, core, candidates, table()), Error::ConstructorInventory(owner) if owner == record.owner())
    );

    let mut candidates = records.to_vec();
    let mut changed = record.constructors().records().to_vec();
    let source = constructor.source();
    let access = hir::DeclarationAccessSourceV1::try_new(
        source.declaration_access().declared_visibility(),
        source.declaration_access().lexical_owners().to_vec(),
        other
            .source()
            .declaration_access()
            .definition_origin()
            .clone(),
    )
    .unwrap();
    changed[0] = InheritanceConstructorInterfaceV1::try_new(
        NominalSupportConstructorInterfaceV1::try_new(id, access, source.payload().clone())
            .unwrap(),
    )
    .unwrap();
    candidates[index] = replace(
        record,
        CanonicalInheritanceConstructorsV1::try_new(changed).unwrap(),
        record.protected_members().clone(),
    );
    assert!(
        matches!(reject(checked, core, candidates, table()), Error::CallableContract(CallableTemplateOrigin::Constructor(actual)) if actual == id)
    );

    parameters(checked, core);
    protected(checked, core);
}

fn parameters(checked: CheckedSharedTypeFoundationV1<'_>, core: CheckedSharedTypeFoundationV1<'_>) {
    let records = checked.section().inheritance().records();
    let (index, constructor_index) = records
        .iter()
        .enumerate()
        .find_map(|(index, record)| {
            record
                .constructors()
                .records()
                .iter()
                .position(|constructor| {
                    !constructor
                        .source()
                        .payload()
                        .parameters()
                        .parameters()
                        .is_empty()
                })
                .map(|position| (index, position))
        })
        .unwrap();
    let record = &records[index];
    let mut constructors = record.constructors().records().to_vec();
    let source = constructors[constructor_index].source();
    let id = source.declaration();
    let payload = source.payload();
    constructors[constructor_index] = InheritanceConstructorInterfaceV1::try_new(
        NominalSupportConstructorInterfaceV1::try_new(
            id,
            source.declaration_access().clone(),
            NominalSourceCallablePayloadV1::try_new(
                CallableTemplateOrigin::Constructor(id),
                payload.owner(),
                payload.type_parameters().clone(),
                hir::CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
                payload.result().clone(),
                payload.effects(),
                payload.modality(),
                payload.slot_relations().clone(),
            )
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let mut candidates = records.to_vec();
    candidates[index] = replace(
        record,
        CanonicalInheritanceConstructorsV1::try_new(constructors).unwrap(),
        record.protected_members().clone(),
    );
    assert!(
        matches!(reject(checked, core, candidates, checked.section().protected_declarations().clone()), Error::CallableContract(CallableTemplateOrigin::Constructor(actual)) if actual == id)
    );
}

fn protected(checked: CheckedSharedTypeFoundationV1<'_>, core: CheckedSharedTypeFoundationV1<'_>) {
    let records = checked.section().protected_declarations().records();
    let index = records
        .iter()
        .position(|record| {
            let ProtectedDeclarationInterfaceV1::Constructor(constructor) = record else {
                return false;
            };
            checked
                .section()
                .inheritance()
                .records()
                .iter()
                .any(|owner| {
                    owner
                        .constructors()
                        .get(constructor.declaration())
                        .is_some()
                })
        })
        .unwrap();
    let reference = records[index].reference();
    let mut missing = records.to_vec();
    missing.remove(index);
    assert!(
        matches!(reject(checked, core, checked.section().inheritance().records().to_vec(), CanonicalProtectedDeclarationInterfacesV1::try_new(missing).unwrap()), Error::ProtectedMember(actual) if actual == reference)
    );
}
