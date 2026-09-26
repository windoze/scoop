use super::*;

pub(super) fn resolve(
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    dependencies: &[mir::MirTypeBridgeDependencyViewV1<'_>],
    selected: &[mir::MirTypeBridgeDependencyV1],
    identities: &ValidatedIdentityGraph,
) -> mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1 {
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_external_graph_authorities(identities)
        .unwrap();
    let mut identities = pending.finish().unwrap();
    let decoded: mir::DecodedCrossConeMirTypeBridgeSectionV1 =
        decoded(&Selection { section, selected });
    let authority = mir::MirTypeBridgeLocalInputV1 {
        provider: input.mir.module().cone,

        production: input.mir.production(),
        ordinary: input.ordinary,
    };
    decoded
        .resolve_types(
            authority.provider(),
            input.mir.foundation(),
            dependencies.iter().map(|section| section.exports().types()),
            &mut identities,
        )
        .unwrap()
        .resolve_callables(
            input.mir.foundation(),
            dependencies.iter().map(|section| {
                (
                    section.exports().types(),
                    section.exports().callables(),
                    section.exports().dispatch(),
                )
            }),
            &mut identities,
        )
        .unwrap()
        .resolve_dependencies(authority, &mut identities)
        .unwrap()
}

struct Selection<'a, 's> {
    section: &'a mir::CrossConeMirTypeBridgeSectionV1<'s>,
    selected: &'a [mir::MirTypeBridgeDependencyV1],
}

impl WireEncode for Selection<'_, '_> {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(7)?;
        encoder.field(1)?;
        self.section.exports().types().encode(encoder)?;
        encoder.field(2)?;
        self.section.exports().callables().encode(encoder)?;
        encoder.field(3)?;
        self.section.exports().dispatch().encode(encoder)?;
        encoder.field(4)?;
        self.section.object_values().encode(encoder)?;
        encoder.field(5)?;
        self.section.shape_support().encode(encoder)?;
        encoder.field(6)?;
        self.section.initialization_uses().encode(encoder)?;
        encoder.field(7)?;
        encoder.array(self.selected.len() as u64)?;
        for relation in self.selected {
            relation.encode(encoder)?;
        }
        Ok(())
    }
}
