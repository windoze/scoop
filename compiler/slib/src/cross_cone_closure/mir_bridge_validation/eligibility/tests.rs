use scoop_hir::{
    CallableImplementationV1, CallableInfixV1, CallableInterfaceRecordV1, CallableModalityV1,
    CallableOperatorRoleV1, CallableSafetyV1, CallableSourceEffectsV1, CanonicalBinderListV1,
    CanonicalSourceParameterShapesV1, CoreClosedCallableClassificationError,
    PublicDeclarationOwnerV1, PublicLookupAccessV1,
};
use scoop_identity::{
    CallableOwner, CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate,
    CoreBuiltinNominal, DeclarationScope, DefinitionOwnerChain, DependencyCallableDeclarationId,
    Effect, ExactCallableSignature, ExactTypeKey, GcEffect, PackagePath, PersistentFunctionId,
    SignatureTypeKey, SourceDeclarationKey, SourceDeclarationSite, StrongCallableDefinitionOwner,
};
use scoop_mir::{
    CallableSignatureRecord, CallableSignatureSubject, CanonicalMirFoundation,
    CrossConeMirBridgeSectionV1, OdrFreeMirFoundation, ParamFreeMirCallableExportV1,
    StrongCallableBridgeSurfaceV1, StrongCallableBridgeV1,
};

use super::{
    ClassifiedCallable, CoreClosedCallableClassifier, CrossConeMirClosureRelationError,
    validate_export_relation,
};

#[test]
fn eligible_callable_is_required_in_the_maximal_export_set() {
    let fixture = fixture();
    let empty = CrossConeMirBridgeSectionV1::try_new(
        fixture.artifact,
        &fixture.foundation,
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        validate_export_relation(
            std::slice::from_ref(&fixture.callable),
            &fixture.strong,
            &empty,
            &fixture.classifier,
        ),
        Err(CrossConeMirClosureRelationError::MissingMaximalExport {
            declaration: fixture.declaration,
        })
    );
}

#[test]
fn bridge_cannot_export_a_callable_absent_from_the_public_hir_surface() {
    let fixture = fixture();
    let export = ParamFreeMirCallableExportV1::try_new(
        fixture.declaration,
        fixture.declaration.implementation(),
        fixture.signature.clone(),
    )
    .unwrap();
    let bridge = CrossConeMirBridgeSectionV1::try_new(
        fixture.artifact,
        &fixture.foundation,
        vec![export],
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        validate_export_relation(&[], &fixture.strong, &bridge, &fixture.classifier,),
        Err(CrossConeMirClosureRelationError::UnexpectedExport {
            declaration: fixture.declaration,
        })
    );
}

struct Fixture {
    artifact: scoop_identity::ConeIdentity,
    declaration: DependencyCallableDeclarationId,
    callable: CallableInterfaceRecordV1,
    signature: ExactCallableSignature,
    foundation: OdrFreeMirFoundation,
    strong: StrongCallableBridgeSurfaceV1,
    classifier: TestClassifier,
}

struct TestClassifier {
    declaration: DependencyCallableDeclarationId,
    signature: ExactCallableSignature,
}

impl CoreClosedCallableClassifier for TestClassifier {
    fn classify_callable(
        &self,
        _callable: &CallableInterfaceRecordV1,
    ) -> Result<Option<ClassifiedCallable>, CoreClosedCallableClassificationError> {
        Ok(Some(ClassifiedCallable {
            declaration: self.declaration,
            signature: self.signature.clone(),
        }))
    }
}

fn fixture() -> Fixture {
    let artifact = ConeCoordinate::new("tests", "provider", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let function =
        CborIdentityRecord::<PersistentFunctionId, _>::from_key(SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                artifact,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("run").unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap();
    let declaration = DependencyCallableDeclarationId::Function(function.id());
    let unit = CoreBuiltinNominal::Unit.identity_record().id();
    let exact =
        scoop_identity::PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(unit)).unwrap();
    let signature = ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), exact);
    let callable = CallableInterfaceRecordV1::try_new(
        CallableTemplateOrigin::Function(function.id()),
        PublicDeclarationOwnerV1::TopLevel,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
        SignatureTypeKey::Nominal(unit),
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
    )
    .unwrap();
    let implementation = StrongCallableDefinitionOwner::Function(function.id());
    let mut mir = CanonicalMirFoundation::empty();
    mir.set_callable_signatures(vec![CallableSignatureRecord::new(
        CallableSignatureSubject::Strong(implementation.callable_owner()),
        signature.clone(),
    )])
    .unwrap();
    let foundation = OdrFreeMirFoundation::try_new(mir).unwrap();
    let strong = StrongCallableBridgeSurfaceV1::try_new(vec![StrongCallableBridgeV1::new(
        CallableOwner::Function(function.id()),
        signature.clone(),
    )])
    .unwrap();
    let classifier = TestClassifier {
        declaration,
        signature: signature.clone(),
    };
    Fixture {
        artifact,
        declaration,
        callable,
        signature,
        foundation,
        strong,
        classifier,
    }
}
