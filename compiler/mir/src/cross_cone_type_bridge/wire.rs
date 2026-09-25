use super::*;
use scoop_identity::PersistentIdResolver;

impl WireEncode for ParamFreeMirTypeExportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.exact().encode(encoder)?;
        encoder.field(2)?;
        self.origin().encode(encoder)?;
        encoder.field(3)?;
        self.facts().encode(encoder)?;
        encoder.field(4)?;
        self.representation().encode(encoder)?;
        encoder.field(5)?;
        self.base_and_interfaces().encode(encoder)
    }
}
impl WireEncode for MirBaseAndInterfacesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        match self.base {
            MirBaseClassV1::None => tag(encoder, 1, 1)?,
            MirBaseClassV1::Base(base) => {
                tag(encoder, 2, 2)?;
                encoder.field(1)?;
                base.encode(encoder)?;
            }
        }
        encoder.field(2)?;
        sequence(encoder, &self.interfaces)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedParamFreeMirTypeExportV1 {
    exact: DecodedPersistentId<PersistentExactTypeId>,
    origin: DecodedMirTypeOriginV1,
    facts: DecodedMirTypeFactsV1,
    representation: DecodedMirTypeRepresentationV1,
    base_and_interfaces: DecodedBaseAndInterfaces,
}
impl DecodedParamFreeMirTypeExportV1 {
    pub fn validate(
        self,
        identities: &mut ValidatedIdentityGraph,
        foundation: &crate::OdrFreeMirFoundation,
    ) -> Result<ParamFreeMirTypeExportV1, MirTypeBridgeError> {
        let exact = identities.resolve(self.exact)?;
        let origin = self.origin.resolve(identities)?;
        let facts = self.facts.validate()?;
        let representation = self.representation.resolve(identities)?;
        let base_and_interfaces = self.base_and_interfaces.resolve(identities)?;
        let authority = MirTypeBridgeAuthority {
            identities,
            foundation,
        };

        ParamFreeMirTypeExportV1::try_new(
            authority,
            exact,
            origin,
            facts,
            representation,
            base_and_interfaces,
        )
    }
}
impl WireDecode for DecodedParamFreeMirTypeExportV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            exact: decoder.field(1, DecodedPersistentId::decode)?,
            origin: decoder.field(2, DecodedMirTypeOriginV1::decode)?,
            facts: decoder.field(3, DecodedMirTypeFactsV1::decode)?,
            representation: decoder.field(4, DecodedMirTypeRepresentationV1::decode)?,
            base_and_interfaces: decoder.field(5, DecodedBaseAndInterfaces::decode)?,
        })
    }
}
impl WireEncode for DecodedParamFreeMirTypeExportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.exact.encode(encoder)?;
        encoder.field(2)?;
        self.origin.encode(encoder)?;
        encoder.field(3)?;
        self.facts.encode(encoder)?;
        encoder.field(4)?;
        self.representation.encode(encoder)?;
        encoder.field(5)?;
        self.base_and_interfaces.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedBase {
    None,
    Base(DecodedPersistentId<PersistentExactTypeId>),
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedBaseAndInterfaces {
    base: DecodedBase,
    interfaces: Vec<DecodedPersistentId<PersistentExactTypeId>>,
}
impl DecodedBaseAndInterfaces {
    fn resolve(
        self,
        graph: &mut ValidatedIdentityGraph,
    ) -> Result<MirBaseAndInterfacesV1, MirTypeBridgeError> {
        let base = match self.base {
            DecodedBase::None => MirBaseClassV1::None,
            DecodedBase::Base(base) => MirBaseClassV1::Base(graph.resolve(base)?),
        };
        let interfaces =
            resolve_sequence(self.interfaces, graph, |id, graph| Ok(graph.resolve(id)?))?;
        Ok(MirBaseAndInterfacesV1 { base, interfaces })
    }
}
impl WireDecode for DecodedBaseAndInterfaces {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            base: decoder.field(1, |decoder| {
                let count = decoder.map()?;
                match decoder.field(0, Decoder::unsigned)? {
                    1 => {
                        fields(decoder, count, 1)?;
                        Ok(DecodedBase::None)
                    }
                    2 => {
                        fields(decoder, count, 2)?;
                        Ok(DecodedBase::Base(
                            decoder.field(1, DecodedPersistentId::decode)?,
                        ))
                    }
                    tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
                }
            })?,
            interfaces: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| DecodedPersistentId::decode(decoder))
            })?,
        })
    }
}
impl WireEncode for DecodedBaseAndInterfaces {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        match self.base {
            DecodedBase::None => tag(encoder, 1, 1)?,
            DecodedBase::Base(base) => {
                tag(encoder, 2, 2)?;
                encoder.field(1)?;
                base.encode(encoder)?;
            }
        }
        encoder.field(2)?;
        sequence(encoder, &self.interfaces)
    }
}

pub(super) fn resolve_sequence<D, T>(
    input: Vec<D>,
    graph: &mut ValidatedIdentityGraph,

    mut resolve: impl FnMut(D, &mut ValidatedIdentityGraph) -> Result<T, MirTypeBridgeError>,
) -> Result<Vec<T>, MirTypeBridgeError> {
    let mut result = Vec::new();
    let path = WirePath::root();
    scoop_wire::allocation::try_reserve(&mut result, input.len(), &path)
        .map_err(MirTypeBridgeError::Resource)?;
    for input in input.into_iter() {
        result.push(resolve(input, graph)?);
    }
    Ok(result)
}
