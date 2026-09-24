use super::*;
use scoop_identity::CanonicalIdentifier;

pub(super) fn check(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
) {
    let records = checked.section().protected_source_interfaces().records();
    let position = index(checked, "parameters");
    let source = &records[position];
    let required = &source.parameters().parameters()[0];
    let optional = &source.parameters().parameters()[1];
    let paired = &records[index(checked, "paired")];
    let long = paired.parameters().parameters()[1].value_type().clone();
    let key = optional.calling().template().unwrap();
    assert_ne!(required.definition_origin(), optional.definition_origin());
    let changes = [
        ProtectedSourceParameterV1::new(
            CanonicalIdentifier::new("renamed").unwrap(),
            optional.value_type().clone(),
            optional.calling().clone(),
            optional.definition_origin().clone(),
        ),
        ProtectedSourceParameterV1::new(
            optional.name().clone(),
            long,
            optional.calling().clone(),
            optional.definition_origin().clone(),
        ),
        ProtectedSourceParameterV1::new(
            optional.name().clone(),
            optional.value_type().clone(),
            ProtectedParameterCallingV1::Required,
            optional.definition_origin().clone(),
        ),
        ProtectedSourceParameterV1::new(
            optional.name().clone(),
            optional.value_type().clone(),
            ProtectedParameterCallingV1::VarargEmpty {
                element_type: optional.value_type().clone(),
            },
            optional.definition_origin().clone(),
        ),
        ProtectedSourceParameterV1::new(
            optional.name().clone(),
            optional.value_type().clone(),
            ProtectedParameterCallingV1::VarargDefault {
                element_type: optional.value_type().clone(),
                template: key,
            },
            optional.definition_origin().clone(),
        ),
        ProtectedSourceParameterV1::new(
            optional.name().clone(),
            optional.value_type().clone(),
            optional.calling().clone(),
            required.definition_origin().clone(),
        ),
    ];
    for changed in changes {
        let mut candidate = records.to_vec();
        candidate[position] = ProtectedCallableSourceInterfaceV1::try_new(
            source.owner(),
            CanonicalProtectedSourceParametersV1::try_new(vec![required.clone(), changed]).unwrap(),
        )
        .unwrap();
        assert!(
            matches!(reject(checked, core, candidate), Error::SourceProtocolContract(owner) if owner == source.owner())
        );
    }
    let mut candidate = records.to_vec();
    let mut swapped = paired.parameters().parameters().to_vec();
    swapped.swap(0, 1);
    candidate[index(checked, "paired")] = ProtectedCallableSourceInterfaceV1::try_new(
        paired.owner(),
        CanonicalProtectedSourceParametersV1::try_new(swapped).unwrap(),
    )
    .unwrap();
    assert!(
        matches!(reject(checked, core, candidate), Error::SourceProtocolContract(owner) if owner == paired.owner())
    );
}
