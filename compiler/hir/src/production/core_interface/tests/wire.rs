use super::*;

#[test]
fn core_interface_has_a_fixed_wire_vector_and_validates_atomically() {
    let fixture = fixture();
    let bytes = encode(&fixture.interface).unwrap();
    assert_eq!(bytes.len(), 35_055);
    assert_eq!(
        scoop_wire::sha256(&bytes).to_string(),
        "92e662ca1942f9c97501db013abbb566bec337349fca52cc336e84b2f5c8545c"
    );

    assert_eq!(
        decode_interface(&fixture.interface).validate_against(&fixture.foundation, &fixture.direct),
        Ok(fixture.interface)
    );
}

#[test]
fn interface_and_section_readers_require_closed_products_and_sums() {
    for bytes in [vec![0xa3], vec![0xa5], vec![0xa4, 0x07, 0x00]] {
        assert!(
            decode_canonical::<DecodedCoreHirInterfaceV1>(&bytes, DecodeLimits::default()).is_err()
        );
    }

    for bytes in [
        vec![0xa0],
        vec![0xa1, 0x00, 0x03],
        vec![0xa2, 0x00, 0x01, 0x01, 0x00],
    ] {
        assert!(
            decode_canonical::<DecodedCoreHirInterfaceBranchV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }

    for bytes in [
        vec![0xa2],
        vec![0xa4],
        vec![
            0xa3, 0x01, 0xa1, 0x00, 0x01, 0x02, 0xa1, 0x00, 0x01, 0x04, 0x80,
        ],
    ] {
        assert!(
            decode_canonical::<DecodedCoreBootstrapInterfaceSectionV1>(
                &bytes,
                DecodeLimits::default(),
            )
            .is_err()
        );
    }
}

#[test]
fn core_branch_and_non_core_section_have_fixed_wire_vectors() {
    assert_eq!(
        hex(&encode(&CoreHirInterfaceBranchV1::NotCore).unwrap()),
        "a10001"
    );

    let section = non_core_section();
    assert_eq!(hex(&encode(&section).unwrap()), "a301a1000102a100010380");
    assert_eq!(
        decode_section(&section)
            .validate_against(ConeIdentity::SINGLE_FILE, &CanonicalHirFoundation::empty(),),
        Ok(section)
    );
}

#[test]
fn reader_rejects_removed_core_callable_and_value_tables() {
    struct WithRemovedTable<'a> {
        interface: &'a CoreHirInterfaceV1,
        removed_field: u64,
    }

    impl scoop_wire::WireEncode for WithRemovedTable<'_> {
        fn encode(
            &self,
            encoder: &mut scoop_wire::Encoder,
        ) -> Result<(), scoop_wire::cbor::EncodeError> {
            encoder.map(5)?;
            encoder.field(1)?;
            self.interface.prelude_snapshot.encode(encoder)?;
            encoder.field(2)?;
            self.interface.string_capability.encode(encoder)?;
            if self.removed_field == 3 {
                encoder.field(3)?;
                encoder.array(0)?;
            }
            encoder.field(4)?;
            self.interface.type_targets.encode(encoder)?;
            if self.removed_field == 5 {
                encoder.field(5)?;
                encoder.array(0)?;
            }
            encoder.field(6)?;
            self.interface.compiler_protocols.encode(encoder)
        }
    }

    let fixture = fixture();
    for removed_field in [3, 5] {
        let bytes = encode(&WithRemovedTable {
            interface: &fixture.interface,
            removed_field,
        })
        .unwrap();
        assert!(
            decode_canonical::<DecodedCoreHirInterfaceV1>(&bytes, DecodeLimits::default()).is_err()
        );
    }
}
