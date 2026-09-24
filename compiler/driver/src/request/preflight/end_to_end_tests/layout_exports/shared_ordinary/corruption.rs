use super::*;
use scoop_wire::Encoder;

pub(super) fn check(expected: &lir::CrossConeLirBridgeSectionV1) {
    let records = expected.exports();
    for index in 0..records.len() {
        let mut missing = records.to_vec();
        missing.remove(index);
        reject(expected, &Rows(&missing));
    }
    let mut duplicate = records.to_vec();
    duplicate[1] = duplicate[0].clone();
    reject(expected, &Rows(&duplicate));
    let mut reversed = records.to_vec();
    reversed.reverse();
    reject(expected, &Rows(&reversed));
    let mut extra = records.to_vec();
    extra.push(records[0].clone());
    reject(expected, &Rows(&extra));
    let value = records
        .iter()
        .position(|record| record.root_plan() == lir::ExternalCallableRootPlan::NoGc)
        .unwrap();
    let donor = records
        .iter()
        .find(|record| record.root_plan() == lir::ExternalCallableRootPlan::ManagedStatepoint)
        .unwrap();
    for field in [0, 1, 2, 3, 5, 6] {
        reject(
            expected,
            &Modified {
                records,
                value,
                donor,
                field,
            },
        );
    }
    let bytes = encode(&Modified {
        records,
        value,
        donor,
        field: 4,
    })
    .unwrap();
    assert!(
        decode_canonical::<lir::DecodedCrossConeLirBridgeSectionV1>(
            &bytes,
            DecodeLimits::default()
        )
        .is_err()
    );
}

fn reject(expected: &lir::CrossConeLirBridgeSectionV1, candidate: &impl WireEncode) {
    let wire: lir::DecodedCrossConeLirBridgeSectionV1 = decoded(candidate);
    assert!(matches!(
        wire.validate_against(expected.clone(), &mut meter()),
        Err(lir::CrossConeLirBridgeValidationError::SectionMismatch)
    ));
}

struct Rows<'a>(&'a [lir::ParamFreeLirCallableExportV1]);
impl WireEncode for Rows<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.array(self.0.len() as u64)?;
        for record in self.0 {
            record.encode(encoder)?;
        }
        encoder.field(2)?;
        encoder.array(0)
    }
}

struct Modified<'a> {
    records: &'a [lir::ParamFreeLirCallableExportV1],
    value: usize,
    donor: &'a lir::ParamFreeLirCallableExportV1,
    field: u8,
}
impl WireEncode for Modified<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.array(self.records.len() as u64)?;
        for (index, value) in self.records.iter().enumerate() {
            if index != self.value {
                value.encode(encoder)?;
                continue;
            }
            encoder.map(2)?;
            encoder.field(1)?;
            if self.field == 0 {
                self.donor.declaration()
            } else {
                value.declaration()
            }
            .encode(encoder)?;
            encoder.field(2)?;
            encoder.map(6)?;
            for field in 1..=6 {
                encoder.field(field)?;
                let record = if u32::from(self.field) == field {
                    self.donor
                } else {
                    value
                };
                match field {
                    1 => record.target().encode(encoder)?,
                    2 => record.abi_signature().encode(encoder)?,
                    3 => record.expected_symbol().encode(encoder)?,
                    4 if self.field == 4 => encoder.unsigned(99)?,
                    4 => record.calling_convention().encode(encoder)?,
                    5 => record.root_plan().encode(encoder)?,
                    6 => record.required_definition().encode(encoder)?,
                    _ => unreachable!(),
                }
            }
        }
        encoder.field(2)?;
        encoder.array(0)
    }
}
