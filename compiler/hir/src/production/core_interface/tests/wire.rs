use super::*;

#[test]
fn protocol_definitions_have_a_fixed_wire_vector_and_validates_atomically() {
    let fixture = fixture();
    let bytes = encode(&fixture.interface).unwrap();
    assert_eq!(
        (bytes.len(), scoop_wire::sha256(&bytes).to_string()),
        (
            6_039,
            "1be6b85e7a4f2d72141b847fb5447c7b42bd60739be87e02fba3a11b1a48ae1e".to_owned()
        )
    );

    assert_eq!(
        decode_interface(&fixture.interface).validate_against(&fixture.foundation),
        Ok(fixture.interface)
    );
}

#[test]
fn definition_and_section_readers_require_closed_products_and_definition_cardinality() {
    for bytes in [vec![0xa1], vec![0xa3], vec![0xa2, 0x07, 0x00]] {
        assert!(decode_canonical::<DecodedCompilerProtocolDefinitionsV1>(&bytes).is_err());
    }

    for bytes in [
        vec![0xa2],
        vec![0xa4],
        // A current section with two protocol definitions is invalid before reading either one.
        vec![0xa3, 0x02, 0xa1, 0x00, 0x01, 0x03, 0x80, 0x04, 0x82],
        // An old qualification sum cannot replace the new definitions array either.
        vec![
            0xa3, 0x02, 0xa1, 0x00, 0x01, 0x03, 0x80, 0x04, 0xa1, 0x00, 0x01,
        ],
        vec![
            0xa3, 0x02, 0xa1, 0x00, 0x01, 0x03, 0x80, 0x04, 0xa2, 0x00, 0x02, 0x01, 0xa2,
        ],
        // The retired field 1 and both of its old sum tags remain invalid.
        vec![
            0xa3, 0x01, 0xa1, 0x00, 0x01, 0x02, 0xa1, 0x00, 0x01, 0x03, 0x80,
        ],
        vec![0xa3, 0x01, 0xa2, 0x00, 0x02, 0x01, 0xa2],
        vec![
            0xa3, 0x01, 0xa1, 0x00, 0x01, 0x02, 0xa1, 0x00, 0x01, 0x04, 0x80,
        ],
    ] {
        assert!(decode_canonical::<DecodedCoreBootstrapInterfaceSectionV1>(&bytes,).is_err());
    }
}

#[test]
fn section_without_local_protocol_definitions_has_a_fixed_wire_vector() {
    let section = non_core_section();
    assert_eq!(hex(&encode(&section).unwrap()), "a302a1000103800480");
    assert_eq!(
        decode_section(&section)
            .validate_against(ConeIdentity::SINGLE_FILE, &CanonicalHirFoundation::empty(),),
        Ok(section)
    );
}

#[test]
fn definitions_preserve_the_string_relation_after_resolving_both_valid_declarations() {
    let mut fixture = fixture();
    let source = fixture
        .foundation
        .type_source_nominal_records()
        .iter()
        .find(|record| {
            record.key().declaration_kind() == scoop_identity::SourceDeclarationKind::Class
                && record.id() != fixture.interface.string_capability().source_type()
        })
        .unwrap()
        .id();
    let exact = CborIdentityRecord::from_key(ExactTypeKey::Nominal(source)).unwrap();
    let mut exact_types = fixture.foundation.type_source_exact_records().to_vec();
    if !exact_types.iter().any(|record| record.id() == exact.id()) {
        exact_types.push(exact.clone());
    }
    fixture.foundation.set_exact_types(exact_types).unwrap();
    fixture.interface.string_capability = RuntimeCoreCapabilityV1::String {
        source_type: source,
        exact_type: exact.id(),
    };
    assert!(matches!(
        decode_interface(&fixture.interface).validate_against(&fixture.foundation),
        Err(CompilerProtocolDefinitionsValidationError::Relation(
            CompilerProtocolDefinitionsRelationError::ProtocolStringMismatch
        ))
    ));
}

#[test]
fn reader_rejects_removed_core_snapshot_type_callable_and_value_fields() {
    struct WithRemovedTable<'a> {
        interface: &'a CompilerProtocolDefinitionsV1,
        removed_field: u64,
    }

    impl scoop_wire::WireEncode for WithRemovedTable<'_> {
        fn encode(
            &self,
            encoder: &mut scoop_wire::Encoder,
        ) -> Result<(), scoop_wire::cbor::EncodeError> {
            encoder.map(3)?;
            if self.removed_field == 1 {
                encoder.field(1)?;
                encoder.array(0)?;
            }
            encoder.field(2)?;
            self.interface.string_capability.encode(encoder)?;
            if self.removed_field == 3 {
                encoder.field(3)?;
                encoder.array(0)?;
            }
            if self.removed_field == 4 {
                encoder.field(4)?;
                encoder.array(0)?;
            }
            if self.removed_field == 5 {
                encoder.field(5)?;
                encoder.array(0)?;
            }
            encoder.field(6)?;
            self.interface.compiler_protocols.encode(encoder)
        }
    }

    let fixture = fixture();
    for removed_field in [1, 3, 4, 5] {
        let bytes = encode(&WithRemovedTable {
            interface: &fixture.interface,
            removed_field,
        })
        .unwrap();
        assert!(decode_canonical::<DecodedCompilerProtocolDefinitionsV1>(&bytes).is_err());
    }
}
