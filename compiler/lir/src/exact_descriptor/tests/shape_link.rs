use super::*;

#[test]
fn shape_link_descriptor_and_registration_share_one_complete_semantic_contract() {
    let fixture = Fixture::new();
    let record = fixture.replay(fixture.semantic()).unwrap();
    let contract = ShapeLinkContractV1::Type {
        descriptor_projection: &record,
    };
    for subject in [
        ExternalStrongShapeSubjectV1::TypeDescriptor(fixture.exact()),
        ExternalStrongShapeSubjectV1::TypeRegistration(fixture.exact()),
    ] {
        assert!(contract.matches_subject(subject));
    }
    assert!(
        !contract.matches_subject(ExternalStrongShapeSubjectV1::Layout(fixture.value_layout()))
    );
    let bytes = encode(&contract).unwrap();
    let raw: DecodedShapeLinkContractV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&raw).unwrap(), bytes);
    raw.validate_against(&contract, &mut fixture.meter())
        .unwrap();
    let mut altered = bytes;
    let spelling = fixture.name().as_bytes();
    let index = altered
        .windows(spelling.len())
        .position(|part| part == spelling)
        .unwrap();
    altered[index] ^= 1;
    let raw: DecodedShapeLinkContractV1 =
        decode_canonical(&altered, DecodeLimits::default()).unwrap();
    assert!(
        raw.validate_against(&contract, &mut fixture.meter())
            .is_err()
    );
}
