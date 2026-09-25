use super::*;

pub(super) fn check(records: &[lir::ParamFreeShapeSupportExportV1]) {
    let value = records
        .iter()
        .find(|record| record.roles().boxed_value().available().is_some())
        .unwrap();
    let donor = records
        .iter()
        .find(|record| record.roles().boxed_value().available().is_none())
        .unwrap();
    for role in 1..=8 {
        let wire: lir::DecodedParamFreeShapeSupportExportV1 =
            decoded(&Modified { value, donor, role });
        assert!(wire.validate_against(value).is_err(), "shape role {role}");
    }
}

struct Modified<'a> {
    value: &'a lir::ParamFreeShapeSupportExportV1,
    donor: &'a lir::ParamFreeShapeSupportExportV1,
    role: u32,
}
impl Modified<'_> {
    fn at(&self, role: u32) -> &lir::ParamFreeShapeSupportRolesV1 {
        if role == self.role {
            self.donor.roles()
        } else {
            self.value.roles()
        }
    }
}
impl WireEncode for Modified<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
        encoder.field(1)?;
        self.at(1).source_nominal().encode(encoder)?;
        encoder.field(2)?;
        self.at(2).value_layout().encode(encoder)?;
        encoder.field(3)?;
        self.at(3).ref_scan().encode(encoder)?;
        encoder.field(4)?;
        self.at(4).type_descriptor().encode(encoder)?;
        encoder.field(5)?;
        self.at(5).type_registration().encode(encoder)?;
        encoder.field(6)?;
        self.at(6).boxed_value().encode(encoder)?;
        encoder.field(7)?;
        self.at(7).coroutine_step().encode(encoder)?;
        encoder.field(8)?;
        self.at(8).coroutine_slot().encode(encoder)
    }
}
