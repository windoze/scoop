use super::*;

pub(super) fn resolve(
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    dependencies: &[mir::MirTypeBridgeDependencyViewV1<'_>],
    uses: &mir::CanonicalMirExternalInitializationUsesV1,
    identities: &ValidatedIdentityGraph,
) -> mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1 {
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_external_graph_authorities(identities)
        .unwrap();
    let mut identities = pending.finish().unwrap();
    let candidate: mir::DecodedCrossConeMirTypeBridgeSectionV1 = decoded(&Uses { section, uses });
    let authority = mir::MirTypeBridgeLocalInputV1 {
        provider: input.mir.module().cone,

        production: input.mir.production(),
        ordinary: input.ordinary,
    };
    let direct = std::iter::once(input.ordinary)
        .chain(
            dependencies
                .iter()
                .map(|section| section.direct_callables()),
        )
        .collect::<Vec<_>>();
    candidate
        .resolve_types(
            authority.provider(),
            input.mir.foundation(),
            dependencies.iter().map(|section| section.exports().types()),
            &mut identities,
        )
        .unwrap()
        .resolve_callables(
            input.mir.foundation(),
            &direct,
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

struct Uses<'a, 's> {
    section: &'a mir::CrossConeMirTypeBridgeSectionV1<'s>,
    uses: &'a mir::CanonicalMirExternalInitializationUsesV1,
}

impl WireEncode for Uses<'_, '_> {
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
        self.uses.encode(encoder)?;
        encoder.field(7)?;
        let selected = self.section.selected().relations().collect::<Vec<_>>();
        encoder.array(selected.len() as u64)?;
        for relation in selected {
            relation.encode(encoder)?;
        }
        Ok(())
    }
}
