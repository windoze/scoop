use super::*;
use scoop_identity::PersistentIdResolver;

/// A transport relation or an independently committed source request.
/// Constructing one never grants an imported-use handle.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MirTypeBridgeDependencyV1 {
    provider: ConeIdentity,
    target: MirTypeBridgeTargetV1,
}
impl MirTypeBridgeDependencyV1 {
    pub const fn new(provider: ConeIdentity, target: MirTypeBridgeTargetV1) -> Self {
        Self { provider, target }
    }
    pub const fn provider(self) -> ConeIdentity {
        self.provider
    }
    pub const fn target(self) -> MirTypeBridgeTargetV1 {
        self.target
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DecodedMirTypeBridgeDependencyV1 {
    provider: DecodedPersistentId<ConeIdentity>,
    target: DecodedMirTypeBridgeTargetV1,
}
impl DecodedMirTypeBridgeDependencyV1 {
    pub(super) fn resolve<E>(
        self,
        graph: &mut ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<MirTypeBridgeDependencyV1, MirTypeBridgeSectionError<E>> {
        meter.charge_work(1, &WirePath::root())?;
        Ok(MirTypeBridgeDependencyV1::new(
            graph.resolve(self.provider)?,
            self.target.resolve(graph, meter)?,
        ))
    }
}
macro_rules! encode_dependency {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(2)?;
                encoder.field(1)?;
                self.provider.encode(encoder)?;
                encoder.field(2)?;
                self.target.encode(encoder)
            }
        }
    };
}
encode_dependency!(MirTypeBridgeDependencyV1);
encode_dependency!(DecodedMirTypeBridgeDependencyV1);
impl WireDecode for DecodedMirTypeBridgeDependencyV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            provider: decoder.field(1, DecodedPersistentId::decode)?,
            target: decoder.field(2, DecodedMirTypeBridgeTargetV1::decode)?,
        })
    }
}
