use super::*;
use scoop_wire::Encoder;

pub(super) fn check(
    input: LayoutAbiExportInputV1<'_>,
    expected: &lir::LayoutAbiExportConstituentsV1,
) {
    let abis = expected.callables();
    let records = abis.records();
    let reject = |rows: &[lir::ExactCallableAbiExportV1]| {
        let decoded: lir::DecodedCanonicalExactCallableAbiExportsV1 = decoded(&Rows(rows));
        assert!(decoded.validate_against(abis).is_err());
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

    let named = |name| {
        expected
            .direct_callables()
            .exports()
            .iter()
            .find(|record| assertions::callable_name(input, record.target().into()) == name)
            .unwrap()
    };
    let value = named("SharedAbiEmpty.wide");
    let donor = named("SharedAbiEmpty.drop");
    assert!(matches!(
        value.abi_signature().arguments()[1],
        scoop_identity::ScoopAbiArgument::Indirect(_)
    ));
    assert!(matches!(
        value.abi_signature().result(),
        scoop_identity::ScoopAbiReturn::Indirect(_)
    ));
    assert_eq!(
        donor.abi_signature().result(),
        scoop_identity::ScoopAbiReturn::UnitVoid
    );
    let empty = named("SharedAbiEmpty.empty");
    let parameters = empty.abi_signature().signature().parameters();
    assert_eq!(parameters.len(), 2);
    assert_eq!(parameters[0], parameters[1]);
    assert!(
        empty
            .abi_signature()
            .arguments()
            .iter()
            .all(|argument| matches!(argument, scoop_identity::ScoopAbiArgument::ElidedZst(_)))
    );
    assert!(matches!(
        empty.abi_signature().result(),
        scoop_identity::ScoopAbiReturn::ElidedZst(_)
    ));
    assert_ne!(value.abi_signature(), donor.abi_signature());
    assert_ne!(value.root_plan(), donor.root_plan());
    let donor = records
        .iter()
        .find(|record| {
            record.canonical_signature().result() == scoop_identity::ScoopAbiReturn::UnitVoid
        })
        .unwrap();
    let value = records
        .iter()
        .find(|record| {
            matches!(
                record.canonical_signature().result(),
                scoop_identity::ScoopAbiReturn::Indirect(_)
            ) && record.call_protocol() != donor.call_protocol()
        })
        .unwrap();
    for component in [
        Component::Signature,
        Component::Protocol,
        Component::Definition,
    ] {
        let decoded: lir::DecodedExactCallableAbiExportV1 = decoded(&Modified {
            value,
            donor,
            component,
        });
        assert!(decoded.validate_against(value).is_err(), "{component:?}");
    }
    let exact = donor.canonical_signature().signature().result();
    let layouts = lir::CanonicalExactLayoutExportsV1::try_new(
        expected.target_profile(),
        input.lir.foundation(),
        expected
            .layouts()
            .records()
            .iter()
            .filter(|record| record.identity().exact() != exact)
            .cloned()
            .collect(),
    )
    .unwrap();
    assert!(
        matches!(replay(input, &layouts, &[], abis), Err(Error::Abi(lir::ExactCallableAbiError::MissingValueLayout { exact: missing })) if missing == exact)
    );
}

struct Rows<'a>(&'a [lir::ExactCallableAbiExportV1]);
impl WireEncode for Rows<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for record in self.0 {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
enum Component {
    Signature,
    Protocol,
    Definition,
}
struct Modified<'a> {
    value: &'a lir::ExactCallableAbiExportV1,
    donor: &'a lir::ExactCallableAbiExportV1,
    component: Component,
}
impl WireEncode for Modified<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.value.target().encode(encoder)?;
        encoder.field(2)?;
        let value = if matches!(self.component, Component::Signature) {
            self.donor
        } else {
            self.value
        };
        value.canonical_signature().encode(encoder)?;
        encoder.field(3)?;
        self.value.calling_convention().encode(encoder)?;
        encoder.field(4)?;
        let value = if matches!(self.component, Component::Protocol) {
            self.donor
        } else {
            self.value
        };
        value.call_protocol().encode(encoder)?;
        encoder.field(6)?;
        let value = if matches!(self.component, Component::Definition) {
            self.donor
        } else {
            self.value
        };
        value.definition().encode(encoder)
    }
}
