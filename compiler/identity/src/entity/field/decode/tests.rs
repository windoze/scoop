use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::{DecodedFieldIdentityKey, DecodedGeneratedFieldKey, FieldIdentityResolutionError};
use crate::{
    CallableAdapterEnvironmentKey, CallableMaterialization, CallableMaterializationContext,
    CallableTemplateOwner, CanonicalIdentifier, CborIdentityRecord, ClosureEnvironmentRole,
    DecodedCborIdentityRecord, DecodedPersistentId, Effect, ExactCallableSignature,
    FieldIdentityError, FieldIdentityKey, GeneratedNominalKey, PackagePath, PersistentExactTypeId,
    PersistentFieldId, PersistentFunctionId, PersistentGenericTypeId, PersistentIdMismatch,
    PersistentIdResolver, PersistentKeyResolver, PersistentLocalValueId, PersistentPropertyId,
    PersistentTypeId, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("identity is absent from the test graph")
    }
}

impl std::error::Error for ResolutionError {}

struct Resolver {
    source: Vec<SourceDeclarationKey>,
    generated: Vec<GeneratedNominalKey>,
}

trait TestId {
    fn expected() -> Self;
}

impl PersistentKeyResolver<PersistentTypeId, SourceDeclarationKey> for Resolver {
    type Error = ResolutionError;

    fn resolve_key(
        &mut self,
        id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<std::sync::Arc<SourceDeclarationKey>, Self::Error> {
        self.source
            .iter()
            .find(|key| {
                PersistentTypeId::from_source_declaration(key)
                    .is_ok_and(|expected| expected.as_array() == id.as_array())
            })
            .map(|key| std::sync::Arc::new(key.clone()))
            .ok_or(ResolutionError)
    }
}

impl PersistentKeyResolver<PersistentGenericTypeId, SourceDeclarationKey> for Resolver {
    type Error = ResolutionError;

    fn resolve_key(
        &mut self,
        id: DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<std::sync::Arc<SourceDeclarationKey>, Self::Error> {
        self.source
            .iter()
            .find(|key| {
                PersistentGenericTypeId::from_source_declaration(key)
                    .is_ok_and(|expected| expected.as_array() == id.as_array())
            })
            .map(|key| std::sync::Arc::new(key.clone()))
            .ok_or(ResolutionError)
    }
}

impl PersistentKeyResolver<PersistentTypeId, GeneratedNominalKey> for Resolver {
    type Error = ResolutionError;

    fn resolve_key(
        &mut self,
        id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<std::sync::Arc<GeneratedNominalKey>, Self::Error> {
        self.generated
            .iter()
            .find(|key| {
                PersistentTypeId::from_generated_key(key)
                    .is_ok_and(|expected| expected.as_array() == id.as_array())
            })
            .map(|key| std::sync::Arc::new(key.clone()))
            .ok_or(ResolutionError)
    }
}

macro_rules! test_identity {
    ($id:ty) => {
        impl TestId for $id {
            fn expected() -> Self {
                Self([7; 32])
            }
        }

        impl PersistentIdResolver<$id> for Resolver {
            type Error = ResolutionError;

            fn resolve(&mut self, id: DecodedPersistentId<$id>) -> Result<$id, Self::Error> {
                id.verify(<$id>::expected())
                    .map_err(|_: PersistentIdMismatch<$id>| ResolutionError)
            }
        }
    };
}

test_identity!(PersistentPropertyId);
test_identity!(PersistentLocalValueId);

#[test]
fn every_field_shape_resolves_through_its_canonical_owner_key() {
    let exact = PersistentExactTypeId([7; 32]);
    let property = PersistentPropertyId([7; 32]);
    let local = PersistentLocalValueId([7; 32]);
    let source_struct = source_nominal("SourceStruct", SourceNominalKind::Struct, 0);
    let source_class = source_nominal("SourceClass", SourceNominalKind::Class, 1);
    let materialization = CallableMaterialization::new(
        CallableTemplateOwner::Function(PersistentFunctionId([7; 32])),
        CallableMaterializationContext::NoSubstitution,
    );
    let generated = vec![
        GeneratedNominalKey::BoxedValue { payload: exact },
        GeneratedNominalKey::ClosureEnvironment {
            callable: materialization,
            role: ClosureEnvironmentRole::Lambda,
        },
        GeneratedNominalKey::CoroutineFrame {
            source_callable: materialization,
        },
        GeneratedNominalKey::ContinuationAdapterEnvironment {
            source_callable: materialization,
            suspension_site: StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::CoroutineTransform, 0),
                [],
            ),
        },
        GeneratedNominalKey::CallableAdapterEnvironment {
            key: CallableAdapterEnvironmentKey::Dynamic {
                target: ExactCallableSignature::new(Effect::Ordinary, None, vec![], exact),
            },
        },
        GeneratedNominalKey::ObjectBackingClass {
            object: PersistentTypeId([7; 32]),
        },
    ];
    let fields = vec![
        FieldIdentityKey::source_declared(
            &source_struct,
            CanonicalIdentifier::new("payload").unwrap(),
        )
        .unwrap(),
        FieldIdentityKey::source_property_backing(&source_class, property).unwrap(),
        FieldIdentityKey::source_property_delegate(&source_class, property).unwrap(),
        FieldIdentityKey::box_payload(&generated[0]).unwrap(),
        FieldIdentityKey::closure_capture(&generated[1], local).unwrap(),
        FieldIdentityKey::callable_reference_receiver(&generated[1], local).unwrap(),
        FieldIdentityKey::coroutine_frame_state(&generated[2]).unwrap(),
        FieldIdentityKey::coroutine_frame_completion(&generated[2]).unwrap(),
        FieldIdentityKey::coroutine_frame_task(&generated[2]).unwrap(),
        FieldIdentityKey::coroutine_frame_saved(&generated[2], local).unwrap(),
        FieldIdentityKey::coroutine_frame_failure(&generated[2]).unwrap(),
        FieldIdentityKey::coroutine_adapter_frame(&generated[3]).unwrap(),
        FieldIdentityKey::coroutine_adapter_state(&generated[3]).unwrap(),
        FieldIdentityKey::coroutine_adapter_result(&generated[3]).unwrap(),
        FieldIdentityKey::coroutine_adapter_failure(&generated[3]).unwrap(),
        FieldIdentityKey::function_adapter_source(&generated[4]).unwrap(),
        FieldIdentityKey::object_backing_property(&generated[5], property).unwrap(),
    ];
    let mut resolver = Resolver {
        source: vec![source_struct, source_class],
        generated,
    };

    for key in fields {
        let record = CborIdentityRecord::<PersistentFieldId, _>::from_key(key).unwrap();
        let decoded = decode_canonical::<
            DecodedCborIdentityRecord<PersistentFieldId, DecodedFieldIdentityKey>,
        >(&encode(&record).unwrap())
        .unwrap();
        assert_eq!(
            decoded.resolve(|key| key.resolve(&mut resolver)).unwrap(),
            record
        );
    }
}

#[test]
fn field_resolution_rejects_a_role_that_does_not_match_its_owner() {
    let invalid_owner = GeneratedNominalKey::CoroutineStep {
        result: PersistentExactTypeId([7; 32]),
    };
    let owner = PersistentTypeId::from_generated_key(&invalid_owner).unwrap();
    let decoded = DecodedFieldIdentityKey::Generated {
        owner: DecodedPersistentId::from_unvalidated_bytes(*owner.as_array()),
        key: DecodedGeneratedFieldKey::BoxPayload,
    };
    let mut resolver = Resolver {
        source: vec![],
        generated: vec![invalid_owner],
    };

    assert_eq!(
        decoded.resolve(&mut resolver),
        Err(FieldIdentityResolutionError::Key(
            FieldIdentityError::GeneratedOwnerMismatch
        ))
    );
}

#[test]
fn field_decoder_rejects_unknown_outer_and_generated_tags() {
    let outer = decode_canonical::<DecodedFieldIdentityKey>(b"\xa1\x00\x03").unwrap_err();
    assert_eq!(outer.kind(), &WireErrorKind::UnknownTag { tag: 3 });

    let generated = decode_canonical::<DecodedGeneratedFieldKey>(b"\xa1\x00\x10").unwrap_err();
    assert_eq!(generated.kind(), &WireErrorKind::UnknownTag { tag: 16 });
}

fn source_nominal(
    name: &str,
    kind: SourceNominalKind,
    type_parameter_count: u32,
) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            crate::ConeIdentity::CORE,
            PackagePath::root(),
            crate::DefinitionOwnerChain::top_level(),
            crate::DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        kind,
        type_parameter_count,
    )
}
