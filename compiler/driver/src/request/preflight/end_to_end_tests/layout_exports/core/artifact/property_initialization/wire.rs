use super::*;
use std::convert::Infallible;

pub(super) fn resolve(
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    dependencies: &[&mir::CrossConeMirTypeBridgeSectionV1<'_>],
    uses: &mir::CanonicalMirExternalInitializationUsesV1,
    identities: &ValidatedIdentityGraph,
) -> mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1 {
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_external_graph_authorities(identities)
        .unwrap();
    let mut identities = pending.finish().unwrap();
    let candidate: mir::DecodedCrossConeMirTypeBridgeSectionV1 = decoded(&Uses { section, uses });
    let authority = section.local_authority();
    candidate
        .resolve_types::<Infallible>(
            authority.provider(),
            authority.foundation(),
            dependencies.iter().map(|section| section.types()),
            &mut identities,
        )
        .unwrap()
        .resolve_callables::<Infallible>(
            authority.foundation(),
            dependencies
                .iter()
                .map(|section| (section.types(), section.callables(), section.dispatch())),
            &mut identities,
        )
        .unwrap()
        .resolve_dependencies::<Infallible>(authority, &mut identities)
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
        self.section.types().encode(encoder)?;
        encoder.field(2)?;
        self.section.callables().encode(encoder)?;
        encoder.field(3)?;
        self.section.dispatch().encode(encoder)?;
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
