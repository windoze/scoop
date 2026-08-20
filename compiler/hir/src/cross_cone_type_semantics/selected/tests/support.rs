use super::*;

pub(super) fn path() -> WirePath {
    WirePath::root().field(8)
}
pub(super) fn parsed<T: WireDecode>(value: &impl WireEncode) -> T {
    decode_canonical(&encode(value).unwrap()).unwrap()
}
pub(super) struct Sequence<'a>(pub &'a [SelectedExternalTypeUseV1]);
impl WireEncode for Sequence<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for record in self.0 {
            record.encode(encoder)?;
        }
        Ok(())
    }
}
pub(super) fn decoded(
    records: &[SelectedExternalTypeUseV1],
) -> DecodedCanonicalSelectedExternalTypeUsesV1 {
    parsed(&Sequence(records))
}
pub(super) fn id_wire(id: &[u8; 32]) -> Vec<u8> {
    let mut bytes = vec![0x58, 0x20];
    bytes.extend_from_slice(id);
    bytes
}
pub(super) fn sum(tag: u8, first: &[u8; 32], second: Option<Vec<u8>>) -> Vec<u8> {
    let mut bytes = vec![if second.is_some() { 0xa3 } else { 0xa2 }, 0, tag, 1];
    bytes.extend(id_wire(first));
    if let Some(second) = second {
        bytes.push(2);
        bytes.extend(second);
    }
    bytes
}
pub(super) fn record_wire(provider: ConeIdentity, usage: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0xa2, 1];
    bytes.extend(id_wire(provider.as_array()));
    bytes.push(2);
    bytes.extend(usage);
    bytes
}
