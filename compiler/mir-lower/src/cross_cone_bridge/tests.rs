use scoop_hir::{
    CallableImplementationV1, CallableInfixV1, CallableInterfaceRecordV1, CallableModalityV1,
    CallableOperatorRoleV1, CallableSafetyV1, CallableSourceEffectsV1, CanonicalBinderListV1,
    CanonicalSourceParameterShapesV1, PublicDeclarationOwnerV1, PublicLookupAccessV1,
};
use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, ConeIdentity,
    CoreBuiltinNominal, DeclarationScope, DefinitionOwnerChain, DependencyCallableDeclarationId,
    Effect, ExactCallableSignature, ExactTypeKey, GcEffect, PackagePath, PersistentExactTypeId,
    PersistentFunctionId, SignatureTypeKey, SourceDeclarationKey, SourceDeclarationSite,
    StrongCallableDefinitionOwner,
};
use scoop_mir::{
    CallableSignatureRecord, CallableSignatureSubject, CanonicalMirFoundation,
    OdrFreeMirFoundation, SelectedDependencyMirCallableV1, SelectedExternalMirSet,
};

use super::{
    ClassifiedCallable, CrossConeMirBridgeLoweringError, NominalCallableClassifier,
    lower_cross_cone_bridge_with_classifier,
};

#[test]
fn lowering_derives_maximal_strong_exports_and_preserves_selected_uses() {
    let artifact = cone("producer");
    let provider = cone("dependency");
    let exported = source_function(artifact, "exported");
    let missing_body = source_function(artifact, "missingBody");
    let semantic_only = source_function(artifact, "semanticOnly");
    let signature = unit_signature();
    let callable_records = vec![
        callable(exported),
        callable(missing_body),
        callable(semantic_only),
    ];
    let classifier = FixtureClassifier {
        classified: vec![
            classified(exported, signature.clone()),
            classified(missing_body, signature.clone()),
        ],
        failure: None,
    };
    let foundation = foundation(exported, signature.clone());
    let selected_declaration = source_function(provider, "selected");
    let selected_record = SelectedDependencyMirCallableV1::try_new(
        provider,
        DependencyCallableDeclarationId::Function(selected_declaration),
        StrongCallableDefinitionOwner::Function(selected_declaration),
        signature.clone(),
    )
    .unwrap();
    let selected =
        SelectedExternalMirSet::try_from_callables(artifact, vec![selected_record.clone()])
            .unwrap();

    let bridge = lower_cross_cone_bridge_with_classifier(
        artifact,
        &callable_records,
        &classifier,
        &foundation,
        &selected,
    )
    .unwrap();

    assert_eq!(bridge.exports().len(), 1);
    assert_eq!(
        bridge.exports()[0].declaration(),
        DependencyCallableDeclarationId::Function(exported)
    );
    assert_eq!(bridge.exports()[0].signature(), &signature);
    assert_eq!(bridge.selected(), &[selected_record]);
}

#[test]
fn lowering_rejects_a_strong_signature_that_disagrees_with_hir() {
    let artifact = cone("producer");
    let declaration = source_function(artifact, "exported");
    let signature = unit_signature();
    let classifier = FixtureClassifier {
        classified: vec![classified(declaration, signature.clone())],
        failure: None,
    };
    let wrong_signature = ExactCallableSignature::new(
        Effect::Ordinary,
        Some(signature.result()),
        Vec::new(),
        signature.result(),
    );
    let foundation = foundation(declaration, wrong_signature);
    let selected = SelectedExternalMirSet::empty(artifact);

    assert_eq!(
        lower_cross_cone_bridge_with_classifier(
            artifact,
            &[callable(declaration)],
            &classifier,
            &foundation,
            &selected,
        ),
        Err(CrossConeMirBridgeLoweringError::StrongSignatureMismatch {
            declaration: DependencyCallableDeclarationId::Function(declaration),
        })
    );
}

#[test]
fn lowering_rejects_a_selection_owned_by_another_consumer() {
    let artifact = cone("producer");
    let foreign = cone("foreign-consumer");
    let foundation = OdrFreeMirFoundation::try_new(CanonicalMirFoundation::empty()).unwrap();
    let selected = SelectedExternalMirSet::empty(foreign);

    assert_eq!(
        lower_cross_cone_bridge_with_classifier(
            artifact,
            &[],
            &FixtureClassifier::default(),
            &foundation,
            &selected,
        ),
        Err(CrossConeMirBridgeLoweringError::ForeignSelection {
            expected: artifact,
            actual: foreign,
        })
    );
}

#[test]
fn core_publishes_the_same_eligible_callable_exports_as_other_libraries() {
    let declaration = source_function(ConeIdentity::CORE, "coreCallable");
    let signature = unit_signature();
    let classifier = FixtureClassifier {
        classified: vec![classified(declaration, signature.clone())],
        failure: None,
    };
    let foundation = foundation(declaration, signature);
    let selected = SelectedExternalMirSet::empty(ConeIdentity::CORE);
    let bridge = lower_cross_cone_bridge_with_classifier(
        ConeIdentity::CORE,
        &[callable(declaration)],
        &classifier,
        &foundation,
        &selected,
    )
    .unwrap();
    assert_eq!(bridge.exports().len(), 1);
    assert_eq!(
        bridge.exports()[0].declaration(),
        DependencyCallableDeclarationId::Function(declaration)
    );
    assert!(bridge.selected().is_empty());
}

#[test]
fn lowering_preserves_classifier_failures_with_the_declaration() {
    let artifact = cone("producer");
    let declaration = source_function(artifact, "exported");
    let foundation = OdrFreeMirFoundation::try_new(CanonicalMirFoundation::empty()).unwrap();
    let selected = SelectedExternalMirSet::empty(artifact);
    let source = scoop_hir::NominalCallableClassificationError::Allocation { requested_slots: 7 };
    let classifier = FixtureClassifier {
        classified: Vec::new(),
        failure: Some((
            CallableTemplateOrigin::Function(declaration),
            source.clone(),
        )),
    };

    assert_eq!(
        lower_cross_cone_bridge_with_classifier(
            artifact,
            &[callable(declaration)],
            &classifier,
            &foundation,
            &selected,
        ),
        Err(CrossConeMirBridgeLoweringError::Classification {
            declaration: CallableTemplateOrigin::Function(declaration),
            source,
        })
    );
}

#[derive(Default)]
struct FixtureClassifier {
    classified: Vec<ClassifiedCallable>,
    failure: Option<(
        CallableTemplateOrigin,
        scoop_hir::NominalCallableClassificationError,
    )>,
}

impl NominalCallableClassifier for FixtureClassifier {
    fn classify_callable(
        &self,
        callable: &CallableInterfaceRecordV1,
    ) -> Result<Option<ClassifiedCallable>, scoop_hir::NominalCallableClassificationError> {
        if let Some((declaration, ref source)) = self.failure
            && declaration == callable.declaration()
        {
            return Err(source.clone());
        }
        Ok(self
            .classified
            .iter()
            .find(|classified| {
                classified.declaration
                    == DependencyCallableDeclarationId::Function(function(callable))
            })
            .cloned())
    }
}

fn classified(
    declaration: PersistentFunctionId,
    signature: ExactCallableSignature,
) -> ClassifiedCallable {
    ClassifiedCallable {
        declaration: DependencyCallableDeclarationId::Function(declaration),
        implementation: StrongCallableDefinitionOwner::Function(declaration),
        signature,
    }
}

fn foundation(
    declaration: PersistentFunctionId,
    signature: ExactCallableSignature,
) -> OdrFreeMirFoundation {
    let mut foundation = CanonicalMirFoundation::empty();
    foundation
        .set_callable_signatures(vec![CallableSignatureRecord::new(
            CallableSignatureSubject::Strong(
                StrongCallableDefinitionOwner::Function(declaration).callable_owner(),
            ),
            signature,
        )])
        .unwrap();
    OdrFreeMirFoundation::try_new(foundation).unwrap()
}

fn callable(declaration: PersistentFunctionId) -> CallableInterfaceRecordV1 {
    CallableInterfaceRecordV1::try_new(
        CallableTemplateOrigin::Function(declaration),
        PublicDeclarationOwnerV1::TopLevel,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
        SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
        CallableSourceEffectsV1::try_new(
            Effect::Ordinary,
            CallableSafetyV1::Safe,
            GcEffect::Managed,
            CallableImplementationV1::Scoop,
            CallableOperatorRoleV1::None,
            CallableInfixV1::Ordinary,
        )
        .unwrap(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
        scoop_hir::CanonicalPersistentIdsV1::empty(),
    )
    .unwrap()
}

fn function(callable: &CallableInterfaceRecordV1) -> PersistentFunctionId {
    let CallableTemplateOrigin::Function(function) = callable.declaration() else {
        panic!("the test fixture creates only ordinary functions")
    };
    function
}

fn unit_signature() -> ExactCallableSignature {
    let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap();
    ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), exact)
}

fn source_function(cone: ConeIdentity, name: &str) -> PersistentFunctionId {
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
    .id()
}

fn cone(name: &str) -> ConeIdentity {
    ConeCoordinate::new("tests", name, "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}
