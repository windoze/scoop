use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::{
    DecodedGeneratedCallableKey, DecodedLexicalCallableParent, GeneratedCallableResolutionError,
};
use crate::{
    CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
    CborIdentityRecord, CoroutineAdapterRole, DecodedCborIdentityRecord, DecodedPersistentId,
    Effect, ExactCallableSignature, GeneratedCallableIdentityError, GeneratedCallableKey,
    InitializationCallableRole, LexicalCallableParent, LexicalCallableRole, LexicalParentError,
    PersistentCallableApplicationId, PersistentCallbackApplicationId, PersistentConstructorId,
    PersistentDispatchSlotId, PersistentEnumVariantId, PersistentExactTypeId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentIdMismatch,
    PersistentIdResolver, PersistentInitializationUnitId, PersistentKeyResolver,
    PersistentPropertyAccessorId, PersistentTypeId, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
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
    generated: Vec<GeneratedCallableKey>,
}

trait TestId {
    fn expected() -> Self;
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

test_identity!(PersistentFunctionId);
test_identity!(PersistentGenericFunctionId);
test_identity!(PersistentConstructorId);
test_identity!(PersistentPropertyAccessorId);
test_identity!(PersistentGeneratedCallableId);
test_identity!(PersistentInitializationUnitId);
test_identity!(PersistentExactTypeId);
test_identity!(PersistentCallableApplicationId);
test_identity!(PersistentCallbackApplicationId);
test_identity!(PersistentTypeId);
test_identity!(PersistentDispatchSlotId);
test_identity!(PersistentEnumVariantId);

impl PersistentKeyResolver<PersistentGeneratedCallableId, GeneratedCallableKey> for Resolver {
    type Error = ResolutionError;

    fn resolve_key(
        &mut self,
        id: DecodedPersistentId<PersistentGeneratedCallableId>,
    ) -> Result<std::sync::Arc<GeneratedCallableKey>, Self::Error> {
        self.generated
            .iter()
            .find(|key| {
                PersistentGeneratedCallableId::from_key(key)
                    .is_ok_and(|expected| expected.as_array() == id.as_array())
            })
            .map(|key| std::sync::Arc::new(key.clone()))
            .ok_or(ResolutionError)
    }
}

#[test]
fn enum_variant_constructor_is_a_typed_lexical_parent() {
    let parent = LexicalCallableParent::variant_constructor(PersistentEnumVariantId::expected());
    let decoded =
        decode_canonical::<DecodedLexicalCallableParent>(&encode(&parent).unwrap()).unwrap();
    assert_eq!(
        decoded
            .resolve(&mut Resolver { generated: vec![] })
            .unwrap(),
        parent
    );
}

#[test]
fn all_generated_callable_records_round_trip_and_resolve() {
    let exact = PersistentExactTypeId::expected();
    let function = PersistentFunctionId::expected();
    let parent = LexicalCallableParent::function(function);
    let path = StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, 2),
        [],
    );
    let signature = ExactCallableSignature::new(Effect::Ordinary, None, vec![exact], exact);
    let source_callable = CallableMaterialization::new(
        CallableTemplateOwner::Function(function),
        CallableMaterializationContext::NoSubstitution,
    );
    let keys = vec![
        GeneratedCallableKey::Lexical {
            parent,
            role: LexicalCallableRole::LambdaBody,
            path: path.clone(),
        },
        GeneratedCallableKey::Initialization {
            unit: PersistentInitializationUnitId::expected(),
            role: InitializationCallableRole::Ensure,
        },
        GeneratedCallableKey::DerivedEquality { exact_owner: exact },
        GeneratedCallableKey::FunctionAdapter {
            source: signature.clone(),
            target: signature.clone(),
        },
        GeneratedCallableKey::DynamicFunctionAdapter {
            target: signature.clone(),
        },
        GeneratedCallableKey::CallableReferenceInvoke {
            parent,
            path: path.clone(),
        },
        GeneratedCallableKey::StaticNoGcCallbackStorageBridge {
            source: source_callable,
            signature: signature.clone(),
        },
        GeneratedCallableKey::ForeignCallbackManagedAdapter {
            application: PersistentCallbackApplicationId::expected(),
        },
        GeneratedCallableKey::CoroutineDriver { source_callable },
        GeneratedCallableKey::CoroutineStart { result: exact },
        GeneratedCallableKey::CoroutineAdapter {
            source_callable,
            suspension_site: path,
            role: CoroutineAdapterRole::Success,
        },
        GeneratedCallableKey::FunctionBridge {
            environment: PersistentTypeId::expected(),
            target: signature,
        },
        GeneratedCallableKey::DispatchAdjust {
            slot: PersistentDispatchSlotId::expected(),
            implementor: exact,
            target: source_callable,
        },
        GeneratedCallableKey::BoxingAdjust {
            slot: PersistentDispatchSlotId::expected(),
            payload: exact,
            interface: exact,
        },
        GeneratedCallableKey::ZeroArgumentConstructorAdapter {
            constructor: PersistentConstructorId::expected(),
        },
    ];
    let mut resolver = Resolver { generated: vec![] };

    for key in keys {
        let record = CborIdentityRecord::<PersistentGeneratedCallableId, _>::from_key(key).unwrap();
        let decoded = decode_canonical::<
            DecodedCborIdentityRecord<PersistentGeneratedCallableId, DecodedGeneratedCallableKey>,
        >(&encode(&record).unwrap())
        .unwrap();
        assert_eq!(
            decoded.resolve(|key| key.resolve(&mut resolver)).unwrap(),
            record
        );
    }
}

#[test]
fn generated_callable_resolution_rejects_invalid_target_receiver() {
    let exact = PersistentExactTypeId::expected();
    let invalid = GeneratedCallableKey::DynamicFunctionAdapter {
        target: ExactCallableSignature::new(Effect::Ordinary, Some(exact), vec![], exact),
    };
    let decoded =
        decode_canonical::<DecodedGeneratedCallableKey>(&encode(&invalid).unwrap()).unwrap();

    assert_eq!(
        decoded.resolve(&mut Resolver { generated: vec![] }),
        Err(GeneratedCallableResolutionError::Key(
            GeneratedCallableIdentityError::TargetReceiverMustBeAbsent
        ))
    );
}

#[test]
fn generated_callable_resolution_rejects_nonlexical_generated_parent() {
    let parent_key = GeneratedCallableKey::DerivedEquality {
        exact_owner: PersistentExactTypeId::expected(),
    };
    let parent_id = PersistentGeneratedCallableId::from_key(&parent_key).unwrap();
    let decoded = DecodedGeneratedCallableKey::Lexical {
        parent: DecodedLexicalCallableParent::Generated(
            DecodedPersistentId::from_unvalidated_bytes(*parent_id.as_array()),
        ),
        role: LexicalCallableRole::LambdaBody,
        path: StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, 0),
            [],
        ),
    };

    assert_eq!(
        decoded.resolve(&mut Resolver {
            generated: vec![parent_key]
        }),
        Err(GeneratedCallableResolutionError::Parent(
            LexicalParentError::GeneratedRoleNotLexical
        ))
    );
}

#[test]
fn generated_callable_decoder_rejects_unknown_tags_and_roles() {
    let outer = decode_canonical::<DecodedGeneratedCallableKey>(b"\xa1\x00\x12").unwrap_err();
    assert_eq!(outer.kind(), &WireErrorKind::UnknownTag { tag: 18 });

    let parent_bytes = [b"\xa2\x00\x07\x01\x58\x20".as_slice(), &[0; 32]].concat();
    let parent = decode_canonical::<DecodedLexicalCallableParent>(&parent_bytes).unwrap_err();
    assert_eq!(parent.kind(), &WireErrorKind::UnknownTag { tag: 7 });

    let role = decode_canonical::<LexicalCallableRole>(b"\x03").unwrap_err();
    assert_eq!(role.kind(), &WireErrorKind::UnknownTag { tag: 3 });
}

#[test]
fn retired_continuation_shell_tag_is_rejected() {
    let error = decode_canonical::<DecodedGeneratedCallableKey>(b"\xa1\x00\x0a").unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 10 });
}
