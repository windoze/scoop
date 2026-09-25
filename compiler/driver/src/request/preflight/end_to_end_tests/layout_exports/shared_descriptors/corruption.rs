use super::*;
use scoop_wire::Encoder;
mod record;

pub(super) fn check(
    input: LayoutAbiExportInputV1<'_>,
    expected: &lir::LayoutAbiExportConstituentsV1,
) {
    let descriptors = expected.descriptors();
    let records = descriptors.records();
    let reject = |rows: &[lir::ExactDescriptorExportV1]| {
        let wire: lir::DecodedCanonicalExactDescriptorExportsV1 = decoded(&Rows(rows));
        assert!(wire.validate_against(descriptors).is_err());
    };
    for index in 0..records.len() {
        let mut missing = records.to_vec();
        missing.remove(index);
        reject(&missing);
    }
    let mut duplicate = records.to_vec();
    duplicate[1] = duplicate[0].clone();
    reject(&duplicate);
    let mut reversed = records.to_vec();
    reversed.reverse();
    reject(&reversed);
    let mut extra = records.to_vec();
    extra.push(records[0].clone());
    reject(&extra);
    record::check(records);

    let exact = input
        .lir
        .module()
        .meta
        .layouts
        .iter()
        .find(|(_, layout)| layout.name == "SharedTdEmpty")
        .unwrap()
        .1
        .identity
        .layout_record()
        .key()
        .exact_type();
    for role in [
        scoop_identity::RepresentationRole::ManagedValue,
        scoop_identity::RepresentationRole::ManagedObject,
    ] {
        let layouts = lir::CanonicalExactLayoutExportsV1::try_new(
            expected.target_profile(),
            input.lir.foundation(),
            expected
                .layouts()
                .records()
                .iter()
                .filter(|record| {
                    record.identity().exact() != exact
                        || record.identity().layout_key().representation() != role
                })
                .cloned()
                .collect(),
        )
        .unwrap();
        let error = replay(input, &layouts, expected.dispatch(), &[]).unwrap_err();
        assert!(
            matches!(error, Error::Replay { exact: missing, source } if missing == exact && matches!(
                *source, lir::ExactDescriptorError::MissingValueLayout(_) | lir::ExactDescriptorError::MissingInstanceLayout(_)
            ))
        );
    }
    let vtable = records
        .iter()
        .find(|record| record.exact() == exact)
        .unwrap()
        .dispatch()
        .vtable();
    let dispatch = lir::CanonicalExactDispatchExportsV1::try_new(
        expected.target_profile(),
        input.lir.foundation(),
        expected
            .dispatch()
            .records()
            .iter()
            .filter(|record| record.table() != vtable)
            .cloned()
            .collect(),
    )
    .unwrap();
    assert!(matches!(replay(input, expected.layouts(), &dispatch, &[]),
        Err(Error::Replay { exact: missing, source }) if missing == exact && matches!(*source, lir::ExactDescriptorError::DispatchInventory(_))));
}

struct Rows<'a>(&'a [lir::ExactDescriptorExportV1]);
impl WireEncode for Rows<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for record in self.0 {
            record.encode(encoder)?;
        }
        Ok(())
    }
}
