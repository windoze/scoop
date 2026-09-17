use scoop_identity::PendingIdentityValidation;
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;

#[test]
fn empty_section_has_fixed_wire_and_resolves() {
    let mut section = empty_section();
    let bytes = encode(&section.index_for_wire().unwrap()).unwrap();

    assert_eq!(hex(&bytes), "aa0180028003800480058006800780088009800a80");

    let decoded: DecodedCrossConeHirInterfaceSectionV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);

    let mut identities = PendingIdentityValidation::new().finish().unwrap();
    let resolved = decoded.resolve(&mut identities).unwrap();
    assert_eq!(resolved, section);
    assert!(resolved.public_bindings().records().is_empty());
    assert!(resolved.nominal_interfaces().is_empty());
    assert!(resolved.callable_interfaces().is_empty());
    assert!(resolved.property_interfaces().is_empty());
    assert!(resolved.type_aliases().is_empty());
    assert!(resolved.source_interfaces().is_empty());
    assert!(resolved.default_templates().is_empty());
    assert!(resolved.constants().is_empty());
    assert!(resolved.definition_sources().is_empty());
    assert!(resolved.external_references().is_empty());
}

#[test]
fn reader_rejects_open_or_reordered_top_level_maps() {
    let reordered = vec![
        0xaa, 0x02, 0x80, 0x01, 0x80, 0x03, 0x80, 0x04, 0x80, 0x05, 0x80, 0x06, 0x80, 0x07, 0x80,
        0x08, 0x80, 0x09, 0x80, 0x0a, 0x80,
    ];

    for bytes in [vec![0xa9], vec![0xab], reordered] {
        assert!(
            decode_canonical::<DecodedCrossConeHirInterfaceSectionV1>(
                &bytes,
                DecodeLimits::default(),
            )
            .is_err()
        );
    }
}

fn empty_section() -> CrossConeHirInterfaceSectionV1 {
    CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap(),
        CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
        CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
    )
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
