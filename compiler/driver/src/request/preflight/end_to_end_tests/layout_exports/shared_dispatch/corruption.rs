use super::*;
use scoop_wire::Encoder;

mod entry;

pub(super) fn check(
    input: LayoutAbiExportInputV1<'_>,
    expected: &lir::LayoutAbiExportConstituentsV1,
    abis: SharedLirDispatchAbiInputsV1<'_>,
) {
    let dispatch = expected.dispatch();
    let records = dispatch.records();
    let reject = |rows: &[lir::ExactDispatchExportV1]| {
        let wire: lir::DecodedCanonicalExactDispatchExportsV1 = decoded(&Rows(rows));
        assert!(wire.validate_against(dispatch).is_err());
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
    let table = records
        .iter()
        .find(|table| !table.entries().is_empty())
        .unwrap();
    for component in [
        entry::Component::Position,
        entry::Component::Signature,
        entry::Component::Implementation,
        entry::Component::Abi,
    ] {
        let wire: lir::DecodedExactDispatchExportV1 =
            decoded(&entry::Modified { table, component });
        assert!(wire.validate_against(table).is_err());
    }
    let missing = table.entries()[0].implementation().target();
    let callables = lir::CanonicalExactCallableAbiExportsV1::try_new(
        expected.target_profile(),
        input.lir.foundation(),
        expected
            .callables()
            .records()
            .iter()
            .filter(|record| record.target() != missing)
            .cloned()
            .collect(),
    )
    .unwrap();
    let direct_callables = lir::CrossConeLirBridgeSectionV1::try_new(
        input.lir.foundation(),
        input
            .ordinary
            .exports()
            .iter()
            .filter(|record| record.target() != missing)
            .cloned()
            .collect(),
        input.ordinary.selected().to_vec(),
    )
    .unwrap();
    assert!(matches!(replay(input, SharedLirDispatchAbiInputsV1 {
            local_callables: &callables,
            local_direct_callables: &direct_callables,
            ..abis
        }),
        Err(Error::MissingCallable(target)) if target == missing));

    let receiver = records
        .iter()
        .flat_map(|table| table.entries())
        .find(|entry| {
            entry.implementation().receiver_adaptation()
                == lir::ExactDispatchReceiverAdaptationV1::ReferenceDispatch
        })
        .unwrap()
        .slot_signature()
        .exact()
        .receiver()
        .into_option()
        .unwrap();
    let layouts = lir::CanonicalExactLayoutExportsV1::try_new(
        expected.target_profile(),
        input.lir.foundation(),
        expected
            .layouts()
            .records()
            .iter()
            .filter(|record| record.identity().exact() != receiver)
            .cloned()
            .collect(),
    )
    .unwrap();
    assert!(
        matches!(replay(input, SharedLirDispatchAbiInputsV1 { local_layouts: &layouts, ..abis }),
        Err(Error::MissingValueLayout(exact)) if exact == receiver)
    );
}

struct Rows<'a>(&'a [lir::ExactDispatchExportV1]);
impl WireEncode for Rows<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for record in self.0 {
            record.encode(encoder)?;
        }
        Ok(())
    }
}
