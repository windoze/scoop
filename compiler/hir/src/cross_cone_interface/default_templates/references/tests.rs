use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, DeclarationScope, DefinitionOwnerChain, Effect,
    PackagePath, PersistentIdResolver, PersistentPropertyId, SignatureTypeKey,
    SourceDeclarationKey, SourceDeclarationSite,
};
use scoop_wire::{Encoder, WireEncode, WireErrorKind, decode_canonical, encode};

use super::super::body::expression_test_support::{Fixture, ResolutionError, hex};
use super::*;
use crate::{
    DefaultBoundCallableRefV1, DefaultBoundCallableSourceV1, DefaultCallableDeclarationV1,
    DefaultConstructorRefV1, DefaultFieldRefV1,
};

#[test]
fn direct_reference_preserves_target_and_definition_origin() {
    let fixture = Fixture::new();
    let reference = reference(fixture.property, &fixture);
    let bytes = encode(&reference).unwrap();
    assert_eq!(&bytes[..2], &[0xa2, 0x01]);
    let decoded: DecodedExportDefaultGlobalReferenceV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(
        decoded.resolve_with(&mut fixture.resolver(), |target, resolver| {
            resolver
                .resolve(target)
                .map_err(ExportDefaultReferenceTargetResolutionError::Global)
        }),
        Ok(reference),
    );
}

#[test]
fn callable_target_tags_round_trip() {
    let fixture = Fixture::new();
    let value_type = fixture.value_type();
    let bound = DefaultBoundCallableRefV1::new(
        SignatureTypeKey::Binder { depth: 0, index: 0 },
        DefaultBoundCallableSourceV1::Class {
            bound: value_type.clone(),
            callable: fixture.callable(),
        },
        SignatureTypeKey::Function {
            effect: Effect::Ordinary,
            parameters: Vec::new(),
            result: Box::new(value_type.clone()),
        },
    );
    let declaration = CallableTemplateOrigin::Function(fixture.function);
    let cases = [
        ExportDefaultCallableTargetV1::Callable(fixture.callable()),
        ExportDefaultCallableTargetV1::Bound(bound),
        ExportDefaultCallableTargetV1::DerivedEquality {
            owner_type: value_type,
        },
        ExportDefaultCallableTargetV1::LocalFunction { declaration },
        ExportDefaultCallableTargetV1::Lambda {
            body: fixture.generated,
        },
        ExportDefaultCallableTargetV1::AnonymousFunction {
            body: fixture.generated,
        },
        ExportDefaultCallableTargetV1::CallableReference {
            invoke: fixture.generated,
        },
        ExportDefaultCallableTargetV1::FunctionAddress {
            declaration: DefaultCallableDeclarationV1::Function(fixture.function),
        },
    ];

    for (index, expected) in cases.into_iter().enumerate() {
        let expected_tag = u8::try_from(index + 1).unwrap();
        let bytes = encode(&expected).unwrap();
        assert_eq!(bytes[0..3], [0xa2, 0x00, expected_tag]);
        let decoded: DecodedExportDefaultCallableTargetV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(decoded.resolve(&mut fixture.resolver()), Ok(expected));
    }
}

#[test]
fn reference_set_sorts_each_domain_and_round_trips() {
    let fixture = Fixture::new();
    let direct = reference(
        ExportDefaultCallableTargetV1::Callable(fixture.callable()),
        &fixture,
    );
    let address = reference(
        ExportDefaultCallableTargetV1::FunctionAddress {
            declaration: DefaultCallableDeclarationV1::Function(fixture.function),
        },
        &fixture,
    );
    let owner_type = SignatureTypeKey::Nominal(fixture.type_id);
    let expected = ExportDefaultReferenceSetV1::try_new(
        vec![address.clone(), direct.clone()],
        vec![reference(
            DefaultConstructorRefV1::Struct {
                declaration: fixture.constructor,
                owner_type: owner_type.clone(),
            },
            &fixture,
        )],
        vec![reference(owner_type.clone(), &fixture)],
        vec![reference(fixture.property, &fixture)],
        vec![reference(fixture.object, &fixture)],
        vec![reference(
            DefaultFieldRefV1::Struct {
                declaration: fixture.field,
                owner_type,
            },
            &fixture,
        )],
    )
    .unwrap();

    assert_eq!(expected.callables(), &[direct, address]);
    let bytes = encode(&expected).unwrap();
    let decoded: DecodedExportDefaultReferenceSetV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(decoded.resolve(&mut fixture.resolver()).unwrap(), expected);
}

#[test]
fn reference_set_rejects_duplicates_and_noncanonical_reader_order() {
    let fixture = Fixture::new();
    let direct = reference(
        ExportDefaultCallableTargetV1::Callable(fixture.callable()),
        &fixture,
    );
    assert_eq!(
        ExportDefaultReferenceSetV1::try_new(
            vec![direct.clone(), direct.clone()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        Err(ExportDefaultReferenceSetBuildError::Duplicate(
            ExportDefaultReferenceKindV1::Callable
        ))
    );

    let address = reference(
        ExportDefaultCallableTargetV1::FunctionAddress {
            declaration: DefaultCallableDeclarationV1::Function(fixture.function),
        },
        &fixture,
    );
    let duplicate_bytes = encode(&ReferenceSetWire {
        callables: vec![direct.clone(), direct.clone()],
    })
    .unwrap();
    let duplicate: DecodedExportDefaultReferenceSetV1 = decode_canonical(&duplicate_bytes).unwrap();
    assert!(matches!(
        duplicate.resolve(&mut fixture.resolver()),
        Err(ExportDefaultReferenceSetValidationError::Duplicate {
            kind: ExportDefaultReferenceKindV1::Callable,
            index: 1,
        })
    ));

    let bytes = encode(&ReferenceSetWire {
        callables: vec![address, direct],
    })
    .unwrap();
    let decoded: DecodedExportDefaultReferenceSetV1 = decode_canonical(&bytes).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture.resolver()),
        Err(
            ExportDefaultReferenceSetValidationError::NonCanonicalOrder {
                kind: ExportDefaultReferenceKindV1::Callable,
                index: 1,
            }
        )
    ));
}

#[test]
fn reference_set_rejects_invalid_local_function_shape() {
    let fixture = Fixture::new();
    let invalid = reference(
        ExportDefaultCallableTargetV1::LocalFunction {
            declaration: CallableTemplateOrigin::Constructor(fixture.constructor),
        },
        &fixture,
    );

    assert!(matches!(
        ExportDefaultReferenceSetV1::try_new(
            vec![invalid],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
        Err(ExportDefaultReferenceSetBuildError::CallableTarget {
            index: 0,
            error: ExportDefaultCallableTargetBuildError::UnsupportedLocalFunctionDeclaration(
                CallableTemplateOrigin::Constructor(_)
            ),
        })
    ));
}

#[test]
fn reference_set_reports_nested_resolution_errors() {
    let fixture = Fixture::new();
    let missing = PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        SourceDeclarationSite::new(
            scoop_identity::ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("missing").unwrap(),
    ))
    .unwrap();
    let set = ExportDefaultReferenceSetV1::try_new(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![reference(missing, &fixture)],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let decoded: DecodedExportDefaultReferenceSetV1 =
        decode_canonical(&encode(&set).unwrap()).unwrap();

    assert_eq!(
        decoded.resolve(&mut fixture.resolver()),
        Err(ExportDefaultReferenceSetValidationError::Record {
            kind: ExportDefaultReferenceKindV1::Global,
            index: 0,
            error: ExportDefaultReferenceResolutionError::Target(
                ExportDefaultReferenceTargetResolutionError::Global(ResolutionError)
            ),
        })
    );
}

#[test]
fn reference_wire_rejects_unknown_tags_and_non_exact_maps() {
    let fixture = Fixture::new();
    let error =
        decode_canonical::<DecodedExportDefaultCallableTargetV1>(&[0xa2, 0x00, 0x09, 0x01, 0x00])
            .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 9 });

    let error =
        decode_canonical::<DecodedExportDefaultCallableTargetV1>(&[0xa1, 0x00, 0x01]).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 1,
        }
    );

    let global = reference(fixture.property, &fixture);
    let mut record = encode(&global).unwrap();
    record[0] = 0xa3;
    record.extend([0x03, 0xa0]);
    let error = decode_canonical::<DecodedExportDefaultGlobalReferenceV1>(&record).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 3,
        }
    );

    let error = decode_canonical::<DecodedExportDefaultReferenceSetV1>(&[0xa0]).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 6,
            actual: 0,
        }
    );

    assert_eq!(
        hex(&encode(&ExportDefaultReferenceSetV1::default()).unwrap()),
        "a6018002800380048005800680"
    );
}

fn reference<T>(target: T, fixture: &Fixture) -> ExportDefaultReferenceV1<T> {
    ExportDefaultReferenceV1::new(target, fixture.origin())
}

struct ReferenceSetWire {
    callables: Vec<ExportDefaultCallableReferenceV1>,
}

impl WireEncode for ReferenceSetWire {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        encode_sequence(encoder, &self.callables)?;
        for field in 2..=6 {
            encoder.field(field)?;
            encoder.array(0)?;
        }
        Ok(())
    }
}

fn encode_sequence<T: WireEncode>(
    encoder: &mut Encoder,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}
