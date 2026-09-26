use super::*;
use scoop_identity::*;
use scoop_wire::{WirePath, decode_canonical, encode};

mod fixture;

mod wire;
use fixture::*;

fn decoded(section: &CrossConeTypeSemanticsSectionV1) -> DecodedCrossConeTypeSemanticsSectionV1 {
    decode_canonical(&encode(section).unwrap()).unwrap()
}
fn empty() -> CrossConeTypeSemanticsSectionV1 {
    CrossConeTypeSemanticsSectionV1::new(
        CanonicalExactTypeFactsV1::default(),
        CanonicalNominalRepresentationSupportV1::default(),
        CanonicalNominalInheritanceInterfacesV1::default(),
        CanonicalSelectedExternalTypeUsesV1::try_new(vec![]).unwrap(),
    )
}
