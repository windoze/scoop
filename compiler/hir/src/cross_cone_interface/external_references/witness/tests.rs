use scoop_identity::{
    BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, DeclarationScope,
    DefinitionOwnerChain, ExportBindingKey, PackagePath, PendingIdentityValidation,
    PersistentExportBindingId, PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite,
};
use scoop_wire::{DecodeLimits, Encoder, WireEncode, decode_canonical, encode};

use super::*;
use crate::ReexportRouteHopV1;

#[test]
fn witness_is_wire_identical_to_its_route_and_resolves_typed_ids() {
    let route = route("provider", "target");
    let witness = DependencyBindingWitnessV1::new(route.clone());

    assert_eq!(witness.route(), &route);
    assert_eq!(encode(&witness).unwrap(), encode(&route).unwrap());

    let decoded: DecodedDependencyBindingWitnessV1 =
        decode_canonical(&encode(&witness).unwrap(), DecodeLimits::default()).unwrap();
    let mut authority = authority(std::slice::from_ref(&route));
    assert_eq!(decoded.resolve(&mut authority).unwrap(), witness);
}

#[test]
fn producer_sorts_witnesses_allows_empty_and_rejects_duplicates() {
    let first = DependencyBindingWitnessV1::new(route("first", "first"));
    let second = DependencyBindingWitnessV1::new(route("second", "second"));
    let witnesses =
        CanonicalDependencyBindingWitnessesV1::try_new(vec![second.clone(), first.clone()])
            .unwrap();

    assert!(witnesses.witnesses()[0] < witnesses.witnesses()[1]);
    assert!(!witnesses.is_empty());
    assert!(
        CanonicalDependencyBindingWitnessesV1::try_new(Vec::new())
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        CanonicalDependencyBindingWitnessesV1::try_new(vec![first.clone(), first.clone()]),
        Err(DependencyBindingWitnessSetBuildError::Duplicate(first))
    );
}

#[test]
fn reader_rejects_duplicate_and_noncanonical_witnesses() {
    let first = DependencyBindingWitnessV1::new(route("first", "first"));
    let second = DependencyBindingWitnessV1::new(route("second", "second"));
    let canonical = CanonicalDependencyBindingWitnessesV1::try_new(vec![first, second]).unwrap();
    let low = canonical.witnesses()[0].clone();
    let high = canonical.witnesses()[1].clone();
    let mut authority = authority(&[low.route().clone(), high.route().clone()]);

    let duplicate = decode_witnesses(&WitnessSequence(vec![low.clone(), low.clone()]));
    assert!(matches!(
        duplicate.resolve(&mut authority),
        Err(DependencyBindingWitnessSetValidationError::Duplicate { index: 1 })
    ));

    let reversed = decode_witnesses(&WitnessSequence(vec![high, low]));
    assert!(matches!(
        reversed.resolve(&mut authority),
        Err(DependencyBindingWitnessSetValidationError::NonCanonicalOrder { index: 1 })
    ));
}

fn route(artifact: &str, name: &str) -> ReexportRouteV1 {
    let provider = ConeCoordinate::new("example", artifact, "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let declaration =
        CborIdentityRecord::<PersistentFunctionId, _>::from_key(SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                provider,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap();
    let binding =
        CborIdentityRecord::<PersistentExportBindingId, _>::from_key(ExportBindingKey::new(
            provider,
            PackagePath::root(),
            CanonicalIdentifier::new(name).unwrap(),
            BindingTarget::function(declaration.key()).unwrap(),
        ))
        .unwrap();
    ReexportRouteV1::try_new(
        provider,
        vec![ReexportRouteHopV1::new(provider, binding.id())],
    )
    .unwrap()
}

fn authority(routes: &[ReexportRouteV1]) -> scoop_identity::ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    for route in routes {
        pending
            .register_authority(route.immediate_provider())
            .unwrap();
        for hop in route.hops() {
            pending.register_authority(hop.binding()).unwrap();
        }
    }
    pending.finish().unwrap()
}

struct WitnessSequence(Vec<DependencyBindingWitnessV1>);

impl WireEncode for WitnessSequence {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for witness in &self.0 {
            witness.encode(encoder)?;
        }
        Ok(())
    }
}

fn decode_witnesses(value: &impl WireEncode) -> DecodedCanonicalDependencyBindingWitnessesV1 {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}
