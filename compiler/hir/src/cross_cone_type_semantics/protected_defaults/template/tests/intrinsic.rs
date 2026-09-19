use scoop_identity::{CallableTemplateOrigin, LocalValueSelector, SignatureTypeKey};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::support::*;
use crate::{
    DecodedCanonicalTemplateLocalTableV1, DecodedProtectedDefaultReferenceSetV1,
    ExportDefaultTemplateIndexError,
};

#[test]
fn producer_rejects_result_body_receiver_and_parameter_local_mismatches() {
    let f = Fixture::new();
    let mut candidate = template(&f);
    candidate.result = SignatureTypeKey::Binder { depth: 0, index: 1 };
    assert!(matches!(
        rebuild(candidate),
        Err(ProtectedDefaultTemplateBuildError::ResultType { .. })
    ));
    let mut candidate = template(&f);
    candidate.body = ExportDefaultBodyV1::try_new(
        vec![],
        expression(
            &f,
            DefaultExpressionKindV1::Local(LocalValueSelector::Parameter {
                declaration_index: 7,
            }),
        ),
    )
    .unwrap();
    assert!(matches!(
        rebuild(candidate),
        Err(ProtectedDefaultTemplateBuildError::LocalIndices(
            ExportDefaultTemplateIndexError::Body(_)
        ))
    ));
    let mut candidate = template(&f);
    candidate.locals = CanonicalTemplateLocalTableV1::try_new(vec![local(&f, f.local())]).unwrap();
    assert!(matches!(
        rebuild(candidate),
        Err(ProtectedDefaultTemplateBuildError::LocalIndices(
            ExportDefaultTemplateIndexError::Receiver(_)
        ))
    ));
    let mut candidate = template(&f);
    candidate.locals =
        CanonicalTemplateLocalTableV1::try_new(vec![local(&f, LocalValueSelector::This)]).unwrap();
    assert!(matches!(
        rebuild(candidate),
        Err(ProtectedDefaultTemplateBuildError::LocalIndices(
            ExportDefaultTemplateIndexError::ValueParameters(_)
        ))
    ));
}

#[test]
fn both_witness_profiles_require_the_template_owner() {
    let f = Fixture::new();
    let actual = CallableTemplateOrigin::Constructor(f.constructor);
    for metadata in [false, true] {
        let mut candidate = template(&f);
        candidate.references = if metadata {
            ProtectedDefaultReferenceSetV1::try_new(
                vec![],
                vec![],
                vec![],
                vec![ProtectedDefaultReferenceV1::new(
                    f.property,
                    f.origin(),
                    ProtectedDefaultAccessWitnessV1::generic_source_metadata(actual).unwrap(),
                    CanonicalProtectedDefaultExpressionUsesV1::try_new(vec![]).unwrap(),
                )],
                vec![],
                vec![],
            )
            .unwrap()
        } else {
            references(&f, actual)
        };
        assert!(
            matches!(rebuild(candidate), Err(ProtectedDefaultTemplateBuildError::ReferenceOwner {
            kind: ProtectedDefaultReferenceKindV1::Global, index: 0, actual: owner, ..
        }) if owner == actual)
        );
    }
}

#[test]
fn reader_replays_result_local_and_witness_owner_invariants() {
    let f = Fixture::new();
    let value = template(&f);
    let mut input = decoded(&value);
    input.result = decode_canonical(
        &encode(&SignatureTypeKey::Binder { depth: 0, index: 1 }).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        input.resolve(&mut f.resolver(), &mut meter()),
        Err(ProtectedDefaultTemplateResolutionError::Record(
            ProtectedDefaultTemplateBuildError::ResultType { .. }
        ))
    ));
    let mut input = decoded(&value);
    input.locals =
        decode_canonical::<DecodedCanonicalTemplateLocalTableV1>(&[0x80], DecodeLimits::default())
            .unwrap();
    assert!(matches!(
        input.resolve(&mut f.resolver(), &mut meter()),
        Err(ProtectedDefaultTemplateResolutionError::Receiver(_))
    ));
    let mut input = decoded(&value);
    input.references = decode_canonical::<DecodedProtectedDefaultReferenceSetV1>(
        &encode(&references(
            &f,
            CallableTemplateOrigin::Constructor(f.constructor),
        ))
        .unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        input.resolve(&mut f.resolver(), &mut meter()),
        Err(ProtectedDefaultTemplateResolutionError::Record(
            ProtectedDefaultTemplateBuildError::ReferenceOwner { .. }
        ))
    ));
}
