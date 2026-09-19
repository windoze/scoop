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
        meter: &mut BudgetMeter,
    ) -> Result<ParamFreeMirTypeExportV1, MirTypeBridgeError> {
        let path = WirePath::root();
        meter
            .charge_work(8, &path)
            .map_err(MirTypeBridgeError::Resource)?;
        let exact = identities.resolve(self.exact)?;
        let origin = self.origin.resolve(identities)?;
        let facts = self.facts.validate()?;
        let representation = self.representation.resolve(identities, meter)?;
        let base_and_interfaces = self.base_and_interfaces.resolve(identities, meter)?;
        let authority = MirTypeBridgeAuthority {
            identities,
            foundation,
        };
        let work = 8
            * (representation.fields().len()
                + base_and_interfaces.interfaces.len()
                + representation
                    .variants()
                    .iter()
                    .map(|variant| 1 + variant.fields.len())
                    .sum::<usize>());
        meter
            .charge_work(work as u64, &path)
            .map_err(MirTypeBridgeError::Resource)?;
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
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
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
        meter: &mut BudgetMeter,
    ) -> Result<MirBaseAndInterfacesV1, MirTypeBridgeError> {
        let base = match self.base {
            DecodedBase::None => MirBaseClassV1::None,
            DecodedBase::Base(base) => MirBaseClassV1::Base(graph.resolve(base)?),
        };
        let interfaces = resolve_sequence(self.interfaces, graph, meter, |id, graph, _| {
            Ok(graph.resolve(id)?)
        })?;
        Ok(MirBaseAndInterfacesV1 { base, interfaces })
    }
}
impl WireDecode for DecodedBaseAndInterfaces {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
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
    meter: &mut BudgetMeter,
    mut resolve: impl FnMut(
        D,
        &mut ValidatedIdentityGraph,
        &mut BudgetMeter,
    ) -> Result<T, MirTypeBridgeError>,
) -> Result<Vec<T>, MirTypeBridgeError> {
    let mut result = Vec::new();
    let path = WirePath::root();
    meter
        .try_reserve_collection_slots(&mut result, input.len(), &path)
        .map_err(MirTypeBridgeError::Resource)?;
    for (index, input) in input.into_iter().enumerate() {
        let path = path.clone().index(index as u64);
        meter
            .charge_nodes(1, &path)
            .map_err(MirTypeBridgeError::Resource)?;
        meter
            .charge_work(2, &path)
            .map_err(MirTypeBridgeError::Resource)?;
        result.push(resolve(input, graph, meter)?);
    }
    Ok(result)
}
