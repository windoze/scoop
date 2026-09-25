use super::*;
use std::convert::Infallible;

pub(super) fn resolve(
    layout: &lir::CrossConeLayoutAbiSectionV1<'_>,
    semantic: &[lir::LayoutAbiDependencyV1],
    identities: &ValidatedIdentityGraph,
) -> Result<
    lir::DependencyResolvedCrossConeLayoutAbiSectionV1,
    lir::LayoutAbiSectionError<Infallible>,
> {
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_external_graph_authorities(identities)
        .unwrap();
    let mut identities = pending.finish().unwrap();
    let wire: lir::DecodedCrossConeLayoutAbiSectionV1 = decoded(&Selection { layout, semantic });
    wire.validate_layouts(layout.layouts(), &mut meter())
        .unwrap()
        .validate_callables(layout.callables(), &mut meter())
        .unwrap()
        .validate_dispatch(layout.dispatch(), &mut meter())
        .unwrap()
        .validate_descriptors(layout.descriptors(), &mut meter())
        .unwrap()
        .validate_shape_support::<Infallible>(layout.shape_support(), &mut meter())
        .unwrap()
        .resolve_dependencies(&mut identities, &mut meter())
}

pub(super) fn empty_exports(
    provider: ConeIdentity,
    target: lir::LirTargetProfile,
) -> lir::LayoutAbiExportConstituentsV1 {
    let foundation =
        lir::OdrFreeLirFoundation::try_new(provider, lir::CanonicalLirFoundation::empty()).unwrap();
    let layouts =
        lir::CanonicalExactLayoutExportsV1::try_new(target, &foundation, vec![], &mut meter())
            .unwrap();
    let descriptors =
        lir::CanonicalExactDescriptorExportsV1::try_new(target, &foundation, vec![], &mut meter())
            .unwrap();
    let shapes = lir::CanonicalParamFreeShapeSupportExportsV1::from_sources(
        &[],
        &layouts,
        &descriptors,
        &foundation,
        &mut meter(),
    )
    .unwrap();
    lir::LayoutAbiExportConstituentsV1::try_new(
        layouts,
        descriptors,
        lir::CanonicalExactDispatchExportsV1::try_new(target, &foundation, vec![], &mut meter())
            .unwrap(),
        lir::CanonicalExactCallableAbiExportsV1::try_new(target, &foundation, vec![], &mut meter())
            .unwrap(),
        shapes,
    )
    .unwrap()
}

struct Selection<'a> {
    layout: &'a lir::CrossConeLayoutAbiSectionV1<'a>,
    semantic: &'a [lir::LayoutAbiDependencyV1],
}

impl WireEncode for Selection<'_> {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        field(encoder, 1, self.layout.layouts())?;
        field(encoder, 2, self.layout.descriptors())?;
        field(encoder, 3, self.layout.dispatch())?;
        field(encoder, 4, self.layout.callables())?;
        field(encoder, 5, self.layout.shape_support())?;
        encoder.field(6)?;
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.array(self.semantic.len() as u64)?;
        for relation in self.semantic {
            relation.encode(encoder)?;
        }
        field(encoder, 2, self.layout.selected().physical_imports())
    }
}

fn field(
    encoder: &mut scoop_wire::Encoder,
    index: u32,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(index)?;
    value.encode(encoder)
}
