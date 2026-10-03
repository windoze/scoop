use super::*;

pub(in super::super::super) fn resolve(
    layout: &lir::CrossConeLayoutAbiSectionV1<'_>,
    semantic: &[lir::LayoutAbiDependencyV1],
    identities: &ValidatedIdentityGraph,
) -> Result<lir::DependencyResolvedCrossConeLayoutAbiSectionV1, lir::LayoutAbiSectionError> {
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_external_graph_authorities(identities)
        .unwrap();
    let mut identities = pending.finish().unwrap();
    let wire: lir::DecodedCrossConeLayoutAbiSectionV1 = decoded(&Selection { layout, semantic });
    wire.validate_layouts(layout.layouts())
        .unwrap()
        .validate_callables(layout.callables())
        .unwrap()
        .validate_dispatch(layout.dispatch())
        .unwrap()
        .validate_descriptors(layout.descriptors())
        .unwrap()
        .validate_shape_support(layout.shape_support(), layout.exports().direct_callables())
        .unwrap()
        .resolve_dependencies(&mut identities)
}

pub(in super::super::super) fn empty_exports(
    provider: ConeIdentity,
    target: lir::LirTargetProfile,
) -> lir::LayoutAbiExportConstituentsV1 {
    let foundation =
        lir::ConeLirFoundation::try_new(provider, lir::CanonicalLirFoundation::empty()).unwrap();
    let layouts = lir::CanonicalExactLayoutExportsV1::try_new(target, &foundation, vec![]).unwrap();
    let descriptors =
        lir::CanonicalExactDescriptorExportsV1::try_new(target, &foundation, vec![]).unwrap();
    let shapes = lir::CanonicalParamFreeShapeSupportExportsV1::from_sources(
        &[],
        &layouts,
        &descriptors,
        &foundation,
    )
    .unwrap();
    lir::LayoutAbiExportConstituentsV1::try_new(
        layouts,
        descriptors,
        lir::CanonicalExactDispatchExportsV1::try_new(target, &foundation, vec![]).unwrap(),
        lir::CanonicalExactCallableAbiExportsV1::try_new(target, &foundation, vec![]).unwrap(),
        shapes,
        lir::CrossConeLirBridgeSectionV1::try_new(&foundation, vec![], vec![]).unwrap(),
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
