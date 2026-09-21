use super::*;

#[test]
fn core_interface_has_a_fixed_wire_vector_and_validates_atomically() {
    let fixture = fixture();
    let bytes = encode(&fixture.interface).unwrap();
    assert_eq!(
        (bytes.len(), scoop_wire::sha256(&bytes).to_string()),
        (
            34_681,
            "554cf9138e0dc7c5d530d575378457ab708d24be77c43fc74cb9c49629a4b1b1".to_owned()
        )
    );

    assert_eq!(
        decode_interface(&fixture.interface).validate_against(&fixture.foundation),
        Ok(fixture.interface)
    );
}

#[test]
fn interface_and_section_readers_require_closed_products_and_sums() {
    for bytes in [vec![0xa1], vec![0xa3], vec![0xa2, 0x07, 0x00]] {
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
fn reader_rejects_removed_core_snapshot_type_callable_and_value_fields() {
    struct WithRemovedTable<'a> {
        interface: &'a CoreHirInterfaceV1,
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
        assert!(
            decode_canonical::<DecodedCoreHirInterfaceV1>(&bytes, DecodeLimits::default()).is_err()
        );
    }
}
