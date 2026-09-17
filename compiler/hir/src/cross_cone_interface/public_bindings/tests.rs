use scoop_identity::{
    BindableEntity, BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate,
    ConeIdentity, DeclarationScope, DefinitionOwnerChain, ExportBindingKey, PackagePath,
    PendingIdentityValidation, PersistentExportBindingId, PersistentFunctionId,
    SourceDeclarationKey, SourceDeclarationSite, ValidatedIdentityGraph,
};
use scoop_wire::{DecodeLimits, Encoder, WireEncode, decode_canonical, encode};

use super::*;
use crate::{CanonicalDirectPublicSurfaceV1, ReexportRouteHopV1, ReexportRouteV1};

#[test]
fn direct_surface_matches_exactly_the_declared_current_subset() {
    let current = ConeIdentity::SINGLE_FILE;
    let first = direct_fixture(current, "first");
    let second = direct_fixture(current, "second");
    let provider = cone("example:provider:1.0.0");
    let target = function(provider, "target");
    let provider_binding = binding(provider, "target", &target);
    let facade_binding = binding(current, "forwarded", &target);
    let reexport = PublicExportBindingRecordV1::new(
        facade_binding.id(),
        ExportBindingSourceV1::Reexport {
            routes: CanonicalReexportRoutesV1::try_new(vec![
                ReexportRouteV1::try_new(
                    provider,
                    vec![ReexportRouteHopV1::new(provider, provider_binding.id())],
                )
                .unwrap(),
            ])
            .unwrap(),
        },
    );
    let bindings =
        CanonicalPublicExportBindingsV1::try_new(vec![second.public, reexport, first.public])
            .unwrap();
    let direct =
        CanonicalDirectPublicSurfaceV1::try_new(vec![second.binding.id(), first.binding.id()])
            .unwrap();

    assert_eq!(bindings.validate_direct_surface(&direct), Ok(()));
}

#[test]
fn direct_surface_rejects_missing_and_unexpected_declared_bindings() {
    let fixture = direct_fixture(ConeIdentity::SINGLE_FILE, "entry");
    let binding = fixture.binding.id();
    let empty = CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap();
    let expected = CanonicalDirectPublicSurfaceV1::try_new(vec![binding]).unwrap();

    assert_eq!(
        empty.validate_direct_surface(&expected),
        Err(
            PublicExportBindingDirectSurfaceValidationError::MissingDeclaredCurrent {
                surface_index: 0,
                insertion_index: 0,
                binding,
            }
        )
    );

    let actual = CanonicalPublicExportBindingsV1::try_new(vec![fixture.public]).unwrap();
    let expected = CanonicalDirectPublicSurfaceV1::try_new(Vec::new()).unwrap();
    assert_eq!(
        actual.validate_direct_surface(&expected),
        Err(
            PublicExportBindingDirectSurfaceValidationError::UnexpectedDeclaredCurrent {
                record_index: 0,
                insertion_index: 0,
                binding,
            }
        )
    );
}

#[test]
fn direct_surface_rejects_a_reexport_with_a_direct_binding_id() {
    let current = ConeIdentity::SINGLE_FILE;
    let provider = cone("example:provider:1.0.0");
    let target = function(provider, "target");
    let provider_binding = binding(provider, "target", &target);
    let facade_binding = binding(current, "forwarded", &target);
    let binding = facade_binding.id();
    let reexport = PublicExportBindingRecordV1::new(
        binding,
        ExportBindingSourceV1::Reexport {
            routes: CanonicalReexportRoutesV1::try_new(vec![
                ReexportRouteV1::try_new(
                    provider,
                    vec![ReexportRouteHopV1::new(provider, provider_binding.id())],
                )
                .unwrap(),
            ])
            .unwrap(),
        },
    );
    let bindings = CanonicalPublicExportBindingsV1::try_new(vec![reexport]).unwrap();
    let direct = CanonicalDirectPublicSurfaceV1::try_new(vec![binding]).unwrap();

    assert_eq!(
        bindings.validate_direct_surface(&direct),
        Err(
            PublicExportBindingDirectSurfaceValidationError::DirectBindingIsReexport {
                surface_index: 0,
                record_index: 0,
                binding,
            }
        )
    );
}

#[test]
fn producer_sorts_records_rejects_duplicates_and_has_stable_wire() {
    let first = direct_fixture(ConeIdentity::SINGLE_FILE, "first");
    let second = direct_fixture(ConeIdentity::SINGLE_FILE, "second");
    let bindings =
        CanonicalPublicExportBindingsV1::try_new(vec![second.public.clone(), first.public.clone()])
            .unwrap();

    assert!(bindings.records()[0].binding() < bindings.records()[1].binding());
    assert_eq!(
        CanonicalPublicExportBindingsV1::try_new(vec![first.public.clone(), first.public.clone(),]),
        Err(PublicExportBindingBuildError::DuplicateBinding(
            first.binding.id()
        ))
    );

    assert_eq!(
        hex(&encode(&bindings).unwrap()),
        "82a2015820e399bdf5c411f8f6c0357039fad469f41346889085e50c79d14d93f1572caa0f02a2000101a200040158200f57e5fcf2bc467924c46ed29a667cd13b08465e639acb3d5b6cbdfede9d32f9a2015820f816cc7648e159d4dd22c9965b73323108bf60901cdb9354bd02c5435c7e1d4202a2000101a20004015820a0ad82e902616121130e76c04ed8c451293c0bee2263ec04f07f9f85ace1ea02"
    );
}

#[test]
fn declared_current_round_trips_through_canonical_identity_authority() {
    let fixture = direct_fixture(ConeIdentity::SINGLE_FILE, "entry");
    let expected = CanonicalPublicExportBindingsV1::try_new(vec![fixture.public.clone()]).unwrap();
    let decoded = decode_bindings(&expected);
    let mut authority = authority(
        std::slice::from_ref(&fixture.function),
        std::slice::from_ref(&fixture.binding),
        &[ConeIdentity::SINGLE_FILE],
    );

    assert_eq!(decoded.resolve(&mut authority).unwrap(), expected);
}

#[test]
fn declared_current_requires_the_binding_canonical_key() {
    let fixture = direct_fixture(ConeIdentity::SINGLE_FILE, "entry");
    let expected = CanonicalPublicExportBindingsV1::try_new(vec![fixture.public.clone()]).unwrap();
    let decoded = decode_bindings(&expected);
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(fixture.binding.id()).unwrap();
    pending
        .register_external_canonical_authority(fixture.function)
        .unwrap();
    let mut authority = pending.finish().unwrap();

    assert!(matches!(
        decoded.resolve(&mut authority),
        Err(PublicExportBindingSetValidationError::Record {
            index: 0,
            error: PublicExportBindingResolutionError::BindingKey(_),
        })
    ));
}

#[test]
fn declared_current_rejects_a_different_declaration_target() {
    let expected = direct_fixture(ConeIdentity::SINGLE_FILE, "expected");
    let actual = function(ConeIdentity::SINGLE_FILE, "actual");
    let invalid = PublicExportBindingRecordV1::new(
        expected.binding.id(),
        ExportBindingSourceV1::DeclaredCurrent {
            declaration: BindableEntity::Function(actual.id()),
        },
    );
    let decoded =
        decode_bindings(&CanonicalPublicExportBindingsV1::try_new(vec![invalid]).unwrap());
    let mut authority = authority(
        &[expected.function, actual],
        &[expected.binding],
        &[ConeIdentity::SINGLE_FILE],
    );

    assert!(matches!(
        decoded.resolve(&mut authority),
        Err(PublicExportBindingSetValidationError::Record {
            index: 0,
            error: PublicExportBindingResolutionError::DeclarationMismatch { .. },
        })
    ));
}

#[test]
fn reexport_source_round_trips_with_typed_route_authority() {
    let current = cone("example:facade:1.0.0");
    let provider = cone("example:provider:1.0.0");
    let function = function(provider, "target");
    let provider_binding = binding(provider, "target", &function);
    let facade_binding = binding(current, "forwarded", &function);
    let routes = CanonicalReexportRoutesV1::try_new(vec![
        ReexportRouteV1::try_new(
            provider,
            vec![ReexportRouteHopV1::new(provider, provider_binding.id())],
        )
        .unwrap(),
    ])
    .unwrap();
    let record = PublicExportBindingRecordV1::new(
        facade_binding.id(),
        ExportBindingSourceV1::Reexport { routes },
    );
    let expected = CanonicalPublicExportBindingsV1::try_new(vec![record]).unwrap();
    let decoded = decode_bindings(&expected);
    let mut authority = authority(
        &[function],
        &[provider_binding, facade_binding],
        &[current, provider],
    );

    assert_eq!(decoded.resolve(&mut authority).unwrap(), expected);
}

#[test]
fn reader_rejects_duplicate_and_noncanonical_record_order() {
    let first = direct_fixture(ConeIdentity::SINGLE_FILE, "first");
    let second = direct_fixture(ConeIdentity::SINGLE_FILE, "second");
    let (low, high) = if first.binding.id() < second.binding.id() {
        (&first, &second)
    } else {
        (&second, &first)
    };
    let mut duplicate_authority = authority(
        &[first.function.clone(), second.function.clone()],
        &[first.binding.clone(), second.binding.clone()],
        &[ConeIdentity::SINGLE_FILE],
    );
    let duplicate = decode_bindings(&RecordSequence(vec![
        low.public.clone(),
        low.public.clone(),
    ]));

    assert!(matches!(
        duplicate.resolve(&mut duplicate_authority),
        Err(PublicExportBindingSetValidationError::DuplicateBinding { index: 1, .. })
    ));

    let mut reversed_authority = authority(
        &[first.function.clone(), second.function.clone()],
        &[first.binding.clone(), second.binding.clone()],
        &[ConeIdentity::SINGLE_FILE],
    );
    let reversed = decode_bindings(&RecordSequence(vec![
        high.public.clone(),
        low.public.clone(),
    ]));

    assert_eq!(
        reversed
            .resolve(&mut reversed_authority)
            .unwrap_err()
            .to_string(),
        "non-canonical public export binding order at index 1"
    );
}

struct DirectFixture {
    function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
    binding: CborIdentityRecord<PersistentExportBindingId, ExportBindingKey>,
    public: PublicExportBindingRecordV1,
}

fn direct_fixture(exporter: ConeIdentity, name: &str) -> DirectFixture {
    let function = function(exporter, name);
    let binding = binding(exporter, name, &function);
    let public = PublicExportBindingRecordV1::new(
        binding.id(),
        ExportBindingSourceV1::DeclaredCurrent {
            declaration: BindableEntity::Function(function.id()),
        },
    );
    DirectFixture {
        function,
        binding,
        public,
    }
}

fn function(
    cone: ConeIdentity,
    name: &str,
) -> CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            cone,
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
    .unwrap()
}

fn binding(
    exporter: ConeIdentity,
    name: &str,
    function: &CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
) -> CborIdentityRecord<PersistentExportBindingId, ExportBindingKey> {
    CborIdentityRecord::from_key(ExportBindingKey::new(
        exporter,
        PackagePath::root(),
        CanonicalIdentifier::new(name).unwrap(),
        BindingTarget::function(function.key()).unwrap(),
    ))
    .unwrap()
}

fn authority(
    functions: &[CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>],
    bindings: &[CborIdentityRecord<PersistentExportBindingId, ExportBindingKey>],
    cones: &[ConeIdentity],
) -> ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    for cone in cones {
        pending.register_authority(*cone).unwrap();
    }
    for function in functions {
        pending
            .register_external_canonical_authority(function.clone())
            .unwrap();
    }
    for binding in bindings {
        pending
            .register_external_canonical_authority(binding.clone())
            .unwrap();
    }
    pending.finish().unwrap()
}

fn decode_bindings<T: WireEncode>(value: &T) -> DecodedCanonicalPublicExportBindingsV1 {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}

struct RecordSequence(Vec<PublicExportBindingRecordV1>);

impl WireEncode for RecordSequence {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for record in &self.0 {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

fn cone(value: &str) -> ConeIdentity {
    let mut parts = value.split(':');
    let group = parts.next().unwrap();
    let name = parts.next().unwrap();
    let version = parts.next().unwrap();
    assert!(parts.next().is_none());
    ConeCoordinate::new(group, name, version)
        .unwrap()
        .identity()
        .unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
