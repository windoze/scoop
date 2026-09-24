use super::*;

pub(super) fn check(records: &[lir::ExactDescriptorExportV1]) {
    let value = records
        .iter()
        .find(|record| {
            record.object_scan().contains_reference() && record.ancestry().parent().is_some()
        })
        .unwrap();
    let donor = records
        .iter()
        .find(|record| {
            !record.object_scan().contains_reference() && record.ancestry().parent().is_none()
        })
        .unwrap();
    for component in 1..=10 {
        let wire: lir::DecodedExactDescriptorExportV1 = decoded(&Modified {
            value,
            donor,
            component,
        });
        assert!(
            wire.validate_against(value, &mut meter()).is_err(),
            "descriptor field {component}"
        );
    }
}

struct Modified<'a> {
    value: &'a lir::ExactDescriptorExportV1,
    donor: &'a lir::ExactDescriptorExportV1,
    component: u32,
}
impl Modified<'_> {
    fn at(&self, component: u32) -> &lir::ExactDescriptorExportV1 {
        if self.component == component {
            self.donor
        } else {
            self.value
        }
    }
}
impl WireEncode for Modified<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(10)?;
        encoder.field(1)?;
        self.at(1).exact().encode(encoder)?;
        encoder.field(2)?;
        self.at(2)
            .value_layout()
            .identity()
            .layout()
            .encode(encoder)?;
        encoder.field(3)?;
        self.at(3)
            .instance_layout()
            .identity()
            .layout()
            .encode(encoder)?;
        encoder.field(4)?;
        self.at(4).shape().encode(encoder)?;
        encoder.field(5)?;
        lir::CheckedRefScanV1::from_canonical(self.at(5).object_scan().clone())
            .unwrap()
            .encode(encoder)?;
        encoder.field(6)?;
        self.at(6).ancestry().encode(encoder)?;
        encoder.field(7)?;
        self.at(7).dispatch().encode(encoder)?;
        encoder.field(8)?;
        encoder.text(self.at(8).diagnostic_name().as_str())?;
        encoder.field(9)?;
        self.at(9).definition().encode(encoder)?;
        encoder.field(10)?;
        self.at(10).registration().encode(encoder)
    }
}
