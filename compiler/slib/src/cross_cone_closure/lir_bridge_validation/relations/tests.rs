use scoop_hir::{
    CallableImplementationV1, CallableInfixV1, CallableInterfaceRecordV1, CallableModalityV1,
    CallableOperatorRoleV1, CallableSafetyV1, CallableSourceEffectsV1, CanonicalBinderListV1,
    CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalExportConstValuesV1, CanonicalExportDefaultTemplatesV1,
    CanonicalExportDefinitionSourcesV1, CanonicalExternalHirReferencesV1,
    CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1, CanonicalPublicExportBindingsV1,
    CanonicalSourceParameterShapesV1, CanonicalTypeAliasInterfacesV1,
    CrossConeHirInterfaceSectionV1, PublicDeclarationOwnerV1, PublicLookupAccessV1,
};
use scoop_identity::{
    CallableBodyKey, CallableTemplateOrigin, CanonicalIdentifier,
    CanonicalScoopAbiFunctionSignature, CborIdentityRecord, ConeCoordinate, ConeIdentity,
    DeclarationScope, DefinitionAtomRole, DefinitionAtomSubkey, DefinitionOwnerChain,
    DependencyCallableDeclarationId, Effect, ExactCallableSignature, ExactTypeKey, GcEffect,
    LinkageClass, ObjectDefinitionAtomKey, ObjectDefinitionPlanKey, PackagePath,
    PersistentCallableBodyId, PersistentExactTypeId, PersistentFunctionId, PersistentSymbolKey,
    PersistentSymbolRequest, PersistentSymbolRequestTable, RuntimeIdentityRecord, ScoopAbiReturn,
    SignatureTypeKey, SourceDeclarationKey, SourceDeclarationSite, StrongCallableDefinitionOwner,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    CallingConvention, CanonicalLirFoundation, CrossConeLirBridgeSectionV1,
    ExternalCallableRootPlan, OdrFreeLirFoundation, ParamFreeLirCallableExportV1,
};
use scoop_mir::{
    CallableSignatureRecord, CallableSignatureSubject, CanonicalMirFoundation,
    CrossConeMirBridgeSectionV1, OdrFreeMirFoundation, ParamFreeMirCallableExportV1,
};

use super::{CrossConeLirClosureRelationError, validate_local_projection};

#[test]
fn every_mir_export_requires_an_exact_lir_projection() {
    let fixture = Fixture::new(GcEffect::Managed);
    let empty = CrossConeLirBridgeSectionV1::try_new(
        &empty_lir_foundation(fixture.artifact),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();

    assert_eq!(
        validate(&fixture, &empty),
        Err(CrossConeLirClosureRelationError::MissingLirExport {
            declaration: fixture.declaration,
        })
    );
}

#[test]
fn lir_export_gc_effect_must_match_the_hir_callable_contract() {
    let fixture = Fixture::new(GcEffect::Managed);
    let lir = fixture.lir_bridge(GcEffect::NoGc);

    assert_eq!(
        validate(&fixture, &lir),
        Err(CrossConeLirClosureRelationError::ExportGcEffectMismatch {
            declaration: fixture.declaration,
            expected: GcEffect::Managed,
            actual: GcEffect::NoGc,
        })
    );
}

#[test]
fn exact_lir_export_projection_records_an_abi_replay_obligation() {
    let fixture = Fixture::new(GcEffect::NoGc);
    let lir = fixture.lir_bridge(GcEffect::NoGc);
    let mut expectations = Vec::new();

    validate_local_projection(
        fixture.artifact,
        &fixture.interface,
        &fixture.mir,
        &lir,
        &mut expectations,
    )
    .unwrap();

    assert_eq!(expectations.len(), 1);
    assert_eq!(expectations[0].artifact, fixture.artifact);
    assert_eq!(expectations[0].declaration, fixture.declaration);
    assert_eq!(expectations[0].signature, fixture.signature);
    assert_eq!(expectations[0].gc_effect, GcEffect::NoGc);
    assert_eq!(expectations[0].actual, fixture.abi(GcEffect::NoGc));
}

struct Fixture {
    artifact: ConeIdentity,
    declaration: DependencyCallableDeclarationId,
    target: StrongCallableDefinitionOwner,
    signature: ExactCallableSignature,
    interface: CrossConeHirInterfaceSectionV1,
    mir: CrossConeMirBridgeSectionV1,
    lir_foundation: OdrFreeLirFoundation,
}

impl Fixture {
    fn new(gc_effect: GcEffect) -> Self {
        let artifact = ConeCoordinate::new("tests", "lir-provider", "1.0.0")
            .unwrap()
            .identity()
            .unwrap();
        Self::for_provider(artifact, gc_effect)
    }

    fn for_provider(artifact: ConeIdentity, gc_effect: GcEffect) -> Self {
        let function = function(artifact);
        let declaration = DependencyCallableDeclarationId::Function(function);
        let target = StrongCallableDefinitionOwner::Function(function);
        let unit_nominal = scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id();
        let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(unit_nominal)).unwrap();
        let signature = ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit);
        let interface = interface(
            CallableInterfaceRecordV1::try_new(
                CallableTemplateOrigin::Function(function),
                PublicDeclarationOwnerV1::TopLevel,
                CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
                None,
                CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
                SignatureTypeKey::Nominal(unit_nominal),
                CallableSourceEffectsV1::try_new(
                    Effect::Ordinary,
                    CallableSafetyV1::Safe,
                    gc_effect,
                    CallableImplementationV1::Scoop,
                    CallableOperatorRoleV1::None,
                    CallableInfixV1::Ordinary,
                )
                .unwrap(),
                CallableModalityV1::Final,
                PublicLookupAccessV1::DirectOnly,
                scoop_hir::CanonicalPersistentIdsV1::empty(),
            )
            .unwrap(),
        );

        let mut mir_foundation = CanonicalMirFoundation::empty();
        mir_foundation
            .set_callable_signatures(vec![CallableSignatureRecord::new(
                CallableSignatureSubject::Strong(target.callable_owner()),
                signature.clone(),
            )])
            .unwrap();
        let mir_foundation = OdrFreeMirFoundation::try_new(mir_foundation).unwrap();
        let mir_export = ParamFreeMirCallableExportV1::try_new(
            declaration,
            target,
            signature.clone(),
            match gc_effect {
                GcEffect::Managed => scoop_mir::GcEffect::Managed,
                GcEffect::NoGc => scoop_mir::GcEffect::NoGc,
            },
        )
        .unwrap();
        let mir = CrossConeMirBridgeSectionV1::try_new(
            artifact,
            &mir_foundation,
            vec![mir_export],
            Vec::new(),
        )
        .unwrap();

        Self {
            artifact,
            declaration,
            target,
            signature,
            interface,
            mir,
            lir_foundation: lir_foundation(artifact, target),
        }
    }

    fn abi(&self, gc_effect: GcEffect) -> CanonicalScoopAbiFunctionSignature {
        CanonicalScoopAbiFunctionSignature::new(
            self.signature.clone(),
            Vec::new(),
            ScoopAbiReturn::unit_void(),
            gc_effect,
        )
        .unwrap()
    }

    fn lir_bridge(&self, gc_effect: GcEffect) -> CrossConeLirBridgeSectionV1 {
        let root_plan = match gc_effect {
            GcEffect::Managed => ExternalCallableRootPlan::ManagedStatepoint,
            GcEffect::NoGc => ExternalCallableRootPlan::NoGc,
        };
        let export = ParamFreeLirCallableExportV1::new(
            self.artifact,
            self.declaration,
            self.target,
            self.abi(gc_effect),
            CallingConvention::Cdecl,
            root_plan,
        )
        .unwrap();
        CrossConeLirBridgeSectionV1::try_new(&self.lir_foundation, vec![export], Vec::new())
            .unwrap()
    }
}

#[test]
fn core_and_ordinary_providers_reject_the_same_noncanonical_abi() {
    for provider in [ConeIdentity::CORE, ConeIdentity::SINGLE_FILE] {
        let fixture = Fixture::for_provider(provider, GcEffect::Managed);
        let lir = fixture.lir_bridge(GcEffect::Managed);
        let mut expectations = Vec::new();

        validate_local_projection(
            provider,
            &fixture.interface,
            &fixture.mir,
            &lir,
            &mut expectations,
        )
        .unwrap();
        let expected = fixture.abi(GcEffect::Managed);
        expectations[0].check_canonical(&expected).unwrap();
        let unit_value = scoop_identity::CanonicalScoopStorage::new(
            fixture.signature.result(),
            0,
            std::num::NonZeroU64::MIN,
            scoop_identity::ScoopAbiValueShape::Aggregate,
        );
        expectations[0].actual = CanonicalScoopAbiFunctionSignature::new(
            fixture.signature.clone(),
            vec![],
            ScoopAbiReturn::elided_zst(unit_value).unwrap(),
            GcEffect::Managed,
        )
        .unwrap();
        assert_eq!(
            expectations[0].check_canonical(&expected),
            Err(CrossConeLirClosureRelationError::NonCanonicalExportAbi {
                declaration: fixture.declaration
            })
        );
    }
}

fn validate(
    fixture: &Fixture,
    lir: &CrossConeLirBridgeSectionV1,
) -> Result<(), CrossConeLirClosureRelationError> {
    let mut expectations = Vec::new();
    validate_local_projection(
        fixture.artifact,
        &fixture.interface,
        &fixture.mir,
        lir,
        &mut expectations,
    )
}

fn function(artifact: ConeIdentity) -> PersistentFunctionId {
    PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
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
    .unwrap()
}

fn interface(callable: CallableInterfaceRecordV1) -> CrossConeHirInterfaceSectionV1 {
    CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap(),
        CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableInterfacesV1::try_new(vec![callable]).unwrap(),
        CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
        CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
        Default::default(),
    )
}

fn lir_foundation(
    artifact: ConeIdentity,
    target: StrongCallableDefinitionOwner,
) -> OdrFreeLirFoundation {
    let body_key = CallableBodyKey::strong(target);
    let body = PersistentCallableBodyId::from_key(&body_key).unwrap();
    let definition = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            artifact,
            StrongDefinitionEntity::callable_body(body),
            StrongDefinitionRole::CallableBody,
        )
        .unwrap(),
    )
    .unwrap();
    let atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        definition.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::CallableBody(body),
        LinkageClass::ConeStrong,
    )
    .unwrap();
    let mut foundation = CanonicalLirFoundation::empty();
    foundation
        .set_callable_bodies(vec![RuntimeIdentityRecord::from_key(&body_key).unwrap()])
        .unwrap();
    foundation.set_definition_plans(vec![definition]).unwrap();
    foundation.set_definition_atoms(vec![atom]).unwrap();
    foundation.set_symbol_requests(PersistentSymbolRequestTable::new(vec![symbol]).unwrap());
    OdrFreeLirFoundation::try_new(artifact, foundation).unwrap()
}

fn empty_lir_foundation(artifact: ConeIdentity) -> OdrFreeLirFoundation {
    OdrFreeLirFoundation::try_new(artifact, CanonicalLirFoundation::empty()).unwrap()
}
