use super::*;
use scoop_identity::{CallableTemplateOrigin, CanonicalIdentifier};

#[test]
fn source_protocol_indexes_only_complete_matching_default_table_keys() {
    let f = Fixture::new();
    let first = template(&f, 1);
    let second = template(&f, 2);
    let owner = CallableTemplateOrigin::Function(f.function);
    let parameters = [
        ProtectedParameterCallingV1::Required,
        ProtectedParameterCallingV1::Default {
            template: first.key(),
        },
        ProtectedParameterCallingV1::Default {
            template: second.key(),
        },
    ]
    .into_iter()
    .enumerate()
    .map(|(index, calling)| {
        ProtectedSourceParameterV1::new(
            CanonicalIdentifier::new(&format!("p{index}")).unwrap(),
            f.value_type(),
            calling,
            f.origin(),
        )
    })
    .collect();
    let sources = CanonicalProtectedCallableSourceInterfacesV1::try_new(vec![
        ProtectedCallableSourceInterfaceV1::try_new(
            owner,
            CanonicalProtectedSourceParametersV1::try_new(parameters).unwrap(),
        )
        .unwrap(),
    ])
    .unwrap();
    let defaults =
        CanonicalProtectedDefaultTemplatesV1::try_new(vec![second.clone(), first.clone()]).unwrap();
    let bytes = encode(&sources.index_templates(defaults.keys()).unwrap()).unwrap();
    let decoded: DecodedCanonicalProtectedCallableSourceInterfacesV1 =
        decode_canonical(&bytes).unwrap();
    assert_eq!(
        decoded.resolve(&mut f.resolver(), defaults.keys()).unwrap(),
        sources
    );
    let missing = CanonicalProtectedDefaultTemplatesV1::try_new(vec![first]).unwrap();
    assert!(sources.validate_default_closure(missing.keys()).is_err());
    let extra = CanonicalProtectedDefaultTemplatesV1::try_new(vec![
        template(&f, 1),
        second,
        template(&f, 3),
    ])
    .unwrap();
    assert!(sources.validate_default_closure(extra.keys()).is_err());
}
