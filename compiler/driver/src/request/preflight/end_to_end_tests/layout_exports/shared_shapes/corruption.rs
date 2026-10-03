use super::*;
use scoop_identity::RepresentationRole;
use scoop_wire::Encoder;

mod roles;

pub(super) fn check(
    input: LayoutAbiExportInputV1<'_>,
    expected: &lir::LayoutAbiExportConstituentsV1,
) {
    let shapes = expected.shape_support();
    let records = shapes.records();
    let reject = |rows: &[lir::ParamFreeShapeSupportExportV1]| {
        let wire: lir::DecodedCanonicalParamFreeShapeSupportExportsV1 = decoded(&Rows(rows));
        assert!(wire.validate_against(shapes).is_err());
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
    roles::check(records);

    let source = records
        .iter()
        .find(|record| record.roles().boxed_value().available().is_some())
        .unwrap();
    for helper in [
        source.roles().boxed_value().available().unwrap(),
        source.roles().coroutine_step().available().unwrap(),
        source.roles().coroutine_slot().available().unwrap(),
    ] {
        let exact = helper.exact();
        let descriptors = lir::CanonicalExactDescriptorExportsV1::try_new(
            expected.target_profile(),
            input.lir.foundation(),
            expected
                .descriptors()
                .records()
                .iter()
                .filter(|record| record.exact() != exact)
                .cloned()
                .collect(),
        )
        .unwrap();
        assert!(matches!(replay(input, expected.layouts(), &descriptors),
            Err(Error::Replay(lir::ParamFreeShapeSupportTableError::Record(lir::ParamFreeShapeSupportExportError::MissingDescriptor(missing)))) if missing == exact));
        let layouts = lir::CanonicalExactLayoutExportsV1::try_new(
            expected.target_profile(),
            input.lir.foundation(),
            expected
                .layouts()
                .records()
                .iter()
                .filter(|record| {
                    record.identity().exact() != exact
                        || record.identity().layout_key().representation()
                            != RepresentationRole::ManagedValue
                })
                .cloned()
                .collect(),
        )
        .unwrap();
        assert!(matches!(replay(input, &layouts, expected.descriptors()),
            Err(Error::Replay(lir::ParamFreeShapeSupportTableError::Record(lir::ParamFreeShapeSupportExportError::MissingValueLayout(missing)))) if missing == exact));
    }
    let empty = scoop_identity::PendingIdentityValidation::new()
        .finish()
        .unwrap();
    assert!(matches!(
        scoop_slib::replay_shared_mir_shape_support(
            input.bridge.shapes(),
            expected.layouts(),
            expected.descriptors(),
            &empty,
            input.lir.foundation()
        ),
        Err(Error::Identity(_))
    ));
}

struct Rows<'a>(&'a [lir::ParamFreeShapeSupportExportV1]);
impl WireEncode for Rows<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for record in self.0 {
            record.encode(encoder)?;
        }
        Ok(())
    }
}
