use super::*;
use scoop_identity::*;
use scoop_wire::{
    BudgetMeter, DecodeLimits, ResourceKind, WireErrorKind, WirePath, decode_canonical, encode,
};

mod fixture;
mod resources;
mod wire;
use fixture::*;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
fn decoded(section: &CrossConeTypeSemanticsSectionV1) -> DecodedCrossConeTypeSemanticsSectionV1 {
    decode_canonical(
        &encode(&section.index_for_wire(&mut meter()).unwrap()).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap()
}
fn empty() -> CrossConeTypeSemanticsSectionV1 {
    CrossConeTypeSemanticsSectionV1::new(
        CanonicalExactTypeFactsV1::default(),
        CanonicalNominalRepresentationSupportV1::default(),
        CanonicalNominalInheritanceInterfacesV1::default(),
        CanonicalProtectedDeclarationInterfacesV1::default(),
        CanonicalProtectedCallableSourceInterfacesV1::default(),
        CanonicalProtectedDefaultTemplatesV1::try_new(vec![]).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(vec![]).unwrap(),
        CanonicalSelectedExternalTypeUsesV1::try_new(vec![]).unwrap(),
    )
}
