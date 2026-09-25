use super::*;

fn record_bytes(owner: CallableTemplateOrigin, parameters: &[Vec<u8>]) -> Vec<u8> {
    assert!(parameters.len() < 24);
    let mut bytes = [
        vec![0x81, 0xa2, 1],
        encode(&owner).unwrap(),
        vec![2, 0x80 | parameters.len() as u8],
    ]
    .concat();
    for parameter in parameters {
        bytes.extend(parameter);
    }
    bytes
}

#[test]
fn source_parameter_wire_has_exact_fields_and_closed_calling_tags() {
    with_source(SOURCE, |output, _| {
        let table = table(output);
        let record = table
            .records()
            .iter()
            .find(|r| !r.parameters().is_empty())
            .unwrap();
        let parameter = &record.parameters()[0];
        let fields = |tag| {
            [
                vec![0xa3, 1],
                encode(parameter.shape()).unwrap(),
                vec![2, tag, 3],
                encode(parameter.definition_origin()).unwrap(),
            ]
            .concat()
        };
        for tag in 1..=4 {
            let bytes = record_bytes(record.owner(), &[fields(tag)]);
            let decoded: Decoded = decode_canonical(&bytes).unwrap();
            let restored = decoded
                .resolve(&mut source_inventory::identity_closure(output))
                .unwrap();
            assert_eq!(encode(&restored).unwrap(), bytes);
        }
        for invalid in [0, 5] {
            assert!(
                decode_canonical::<Decoded>(&record_bytes(record.owner(), &[fields(invalid)]))
                    .is_err()
            );
        }
        for count in [2, 4] {
            let mut bad = fields(1);
            bad[0] = 0xa0 | count;
            assert!(decode_canonical::<Decoded>(&record_bytes(record.owner(), &[bad])).is_err());
        }
        let mut bad = record_bytes(record.owner(), &[fields(1)]);
        bad[1] = 0xa3;
        assert!(decode_canonical::<Decoded>(&bad).is_err());
    });
}

#[test]
fn source_parameter_reader_rejects_duplicate_names_and_multiple_varargs() {
    with_source(SOURCE, |output, _| {
        let table = table(output);
        let record = table
            .records()
            .iter()
            .find(|r| r.parameters().len() == 2)
            .unwrap();
        let first = &record.parameters()[0];
        for duplicate_name in [true, false] {
            let parameters = if duplicate_name {
                vec![first.clone(), first.clone()]
            } else {
                record
                    .parameters()
                    .iter()
                    .map(|p| {
                        hir::InheritanceSourceParameterV1::new(
                            p.shape().clone(),
                            hir::ProtectedParameterCallingKindV1::VarargEmpty,
                            p.definition_origin().clone(),
                        )
                    })
                    .collect()
            };
            assert!(Record::try_new(record.owner(), parameters.clone()).is_err());
            let bytes = record_bytes(
                record.owner(),
                &parameters
                    .iter()
                    .map(|p| encode(p).unwrap())
                    .collect::<Vec<_>>(),
            );
            let decoded: Decoded = decode_canonical(&bytes).unwrap();
            assert!(matches!(
                decoded.resolve(&mut source_inventory::identity_closure(output)),
                Err(hir::SourceInventoryError::Reference(_))
            ));
        }
        let export = &output.output().export;
        let property = export.properties.iter().next().unwrap().1;
        let accessor = export.property_accessor_identities[property.capability.getter()].id();
        let owner = CallableTemplateOrigin::Accessor(accessor);
        assert!(Record::try_new(owner, vec![]).is_err());
        let decoded: Decoded = decode_canonical(&record_bytes(owner, &[])).unwrap();
        assert!(matches!(
            decoded.resolve(&mut source_inventory::identity_closure(output)),
            Err(hir::SourceInventoryError::Reference(_))
        ));
    });
}
