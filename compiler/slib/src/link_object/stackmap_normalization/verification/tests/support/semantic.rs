use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionAtomRole,
    DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, ExactTypeKey, ImmortalObjectKey, ImmortalObjectOwner, LayoutKey,
    LinkageClass, ObjectDefinitionAtomId, ObjectDefinitionAtomKey, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, PackagePath, PersistentExactTypeId, PersistentFunctionId,
    PersistentImmortalObjectId, PersistentLayoutId, PersistentPropertyId,
    PersistentSafepointSiteId, PersistentScanId, PersistentStaticStorageId, PersistentSymbolKey,
    PersistentSymbolRequest, PersistentSymbolRequestTable, PersistentTypeId, PropertyOwner,
    RepresentationRole, ScanKey, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
    StrongDefinitionEntity, StrongDefinitionRole, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};
use scoop_lir::{
    AbiReturn, BasicBlock, CallTarget, CallTargets, CallableBodyIdentity, CanonicalCAbiMetadata,
    CanonicalLirFoundation, ConeLirFoundation, DecodedStrongRegistrationProductionSurfaceV1,
    DigestFinalizationPlanV1, DigestInputRefV1, DigestNodeV1, EnumDefs, ExternFunctions, Function,
    GcEffect, Global, GlobalInit, ImmortalObjectIdentity, Instruction, IntrinsicTypeRepresentation,
    Layout, LayoutIdentity, LayoutKind, LirConstantImage, LirMeta, LirStaticInitialState,
    LirTargetProfile, LirType, LocalFunctionIdentities, LocalFunctionRef, ManagedCallDestination,
    ManagedPollSite, ManagedRuntimeFunction, MaterializationRoot, Module, NativeExternalMetadata,
    NativeGlobalBridges, PointerKind, RefScan, RuntimeTypeMappingRecord, SafepointIdentities,
    SafepointIdentity, SafepointMappingRecord, SafepointSiteRef, SafepointSiteRole,
    ScoopAbiSignature, StatepointLiveSet, StaticStorageIdentity,
    StrongCallableRegistrationPlanSetV1, StrongImmortalObjectRegistrationPlanSetV1,
    StrongInitializationUnitRegistrationPlanSetV1, StrongRegistrationProductionSurfaceV1,
    StrongSafepointRegistrationPlanSetV1, StrongStaticStorageRegistrationPlanSetV1,
    StrongTypeRegistrationPlanSetV1, StructDefs, Terminator, TypeDescriptor,
    TypeDescriptorIdentity, TypeDescriptorRef, TypeInstanceShapeV1, VoidCallSignature,
    VtableRecord, WellKnownTypeDescriptors,
};

use super::Corruption;

mod artifacts;
mod foundation;
mod module;

use artifacts::*;
use foundation::foundation;
use module::semantic_module;

pub(crate) struct SemanticInputs {
    pub(crate) module: Module,
    pub(crate) foundation: ConeLirFoundation,
    pub(crate) definitions: Vec<ObjectDefinitionPlanId>,
    pub(crate) digest_plan: DigestFinalizationPlanV1,
    pub(crate) registration_plan: StrongSafepointRegistrationPlanSetV1,
    pub(crate) callable_registration_plan: StrongCallableRegistrationPlanSetV1,
    pub(crate) type_registration_plan: StrongTypeRegistrationPlanSetV1,
    pub(crate) immortal_registration_plan: StrongImmortalObjectRegistrationPlanSetV1,
    pub(crate) static_storage_registration_plan: StrongStaticStorageRegistrationPlanSetV1,
    pub(crate) initialization_registration_plan: StrongInitializationUnitRegistrationPlanSetV1,
    pub(crate) registration_production: StrongRegistrationProductionSurfaceV1,
    pub(crate) safepoint_ids: Vec<u64>,
}

pub(crate) fn inputs(corruption: Corruption) -> SemanticInputs {
    let (module, body, safepoints) = semantic_module(corruption);
    let (
        foundation,
        registrations,
        callable_registration,
        type_registration,
        immortal_registration,
        static_storage,
    ) = foundation(&module, &body, &safepoints, corruption);
    let digest_plan = digest_plan(
        &foundation,
        body.id(),
        &registrations,
        &callable_registration,
        &type_registration,
        &static_storage,
    );
    let registration_production = StrongRegistrationProductionSurfaceV1::from_semantics(
        module.meta.target_profile,
        &foundation,
        &digest_plan,
        scoop_lir::RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap(),
        scoop_lir::StrongCallableRuntimeScanPlanSetV1::from_module(&module).unwrap(),
        scoop_lir::StrongTypeDescriptorSemanticPlanSetV1::from_module(&module).unwrap(),
        scoop_lir::StrongSafepointSemanticPlanSetV1::from_module(&module).unwrap(),
        scoop_lir::StrongImmortalObjectSemanticPlanSetV1::from_module(&module).unwrap(),
        scoop_lir::StrongInitializationUnitSemanticPlanSetV1::from_module(&module).unwrap(),
    )
    .unwrap();
    let registration_plan = registration_production.safepoints().clone();
    let callable_registration_plan = registration_production.callables().clone();
    let type_registration_plan = registration_production.types().clone();
    let immortal_registration_plan = registration_production.immortal_objects().clone();
    let static_storage_registration_plan = registration_production.static_storages().clone();
    let initialization_registration_plan = registration_production.initialization_units().clone();
    let definitions = std::iter::once(definition_plan(body.id()))
        .chain(std::iter::once(callable_registration.plan.id()))
        .chain([
            type_registration.descriptor_plan.id(),
            type_registration.layout_plan.id(),
            type_registration.registration_plan.id(),
        ])
        .chain([
            immortal_registration.object_plan.id(),
            immortal_registration.registration_plan.id(),
        ])
        .chain([
            static_storage.storage_plan.id(),
            static_storage.registration_plan.id(),
            static_storage.layout_plan.id(),
            static_storage.scan_plan.id(),
        ])
        .chain(
            registrations
                .iter()
                .map(|registration| registration.plan.id()),
        )
        .collect();
    let safepoint_ids = safepoints
        .iter()
        .map(|identity| identity.runtime_id().get())
        .collect();
    SemanticInputs {
        module,
        foundation,
        definitions,
        digest_plan,
        registration_plan,
        callable_registration_plan,
        type_registration_plan,
        immortal_registration_plan,
        static_storage_registration_plan,
        initialization_registration_plan,
        registration_production,
        safepoint_ids,
    }
}

#[test]
fn complete_registration_production_uses_the_closed_eight_field_shape() {
    let inputs = inputs(Corruption::None);
    let encoded = scoop_wire::encode(&inputs.registration_production).unwrap();
    assert_eq!(encoded[0], 0xa8);
    let decoded =
        scoop_wire::decode_canonical::<DecodedStrongRegistrationProductionSurfaceV1>(&encoded)
            .unwrap();
    assert_eq!(scoop_wire::encode(&decoded).unwrap(), encoded);

    let validated = decoded
        .validate(
            inputs.module.meta.target_profile,
            &inputs.foundation,
            &inputs.digest_plan,
        )
        .unwrap();
    assert_eq!(validated, inputs.registration_production);

    let old_identity_only =
        scoop_wire::encode(inputs.registration_production.identities()).unwrap();
    assert!(
        scoop_wire::decode_canonical::<DecodedStrongRegistrationProductionSurfaceV1>(
            &old_identity_only,
        )
        .is_err()
    );
}

#[test]
fn registration_production_rejects_a_stale_derived_plan_identity() {
    let inputs = inputs(Corruption::None);

    let stale =
        inputs.registration_production.safepoints().registrations()[0].normalized_stackmap_patch();
    let mut encoded = scoop_wire::encode(&inputs.registration_production).unwrap();
    let offset = encoded
        .windows(stale.as_array().len())
        .position(|bytes| bytes == stale.as_array())
        .unwrap();
    encoded[offset] ^= 1;
    let decoded =
        scoop_wire::decode_canonical::<DecodedStrongRegistrationProductionSurfaceV1>(&encoded)
            .unwrap();

    assert!(
        decoded
            .validate(
                inputs.module.meta.target_profile,
                &inputs.foundation,
                &inputs.digest_plan,
            )
            .is_err()
    );
}

fn digest_plan(
    foundation: &ConeLirFoundation,
    body: scoop_identity::PersistentCallableBodyId,
    registrations: &[RegistrationArtifacts],
    callable_registration: &CallableRegistrationArtifacts,
    type_registration: &TypeRegistrationArtifacts,
    static_storage: &StaticStorageArtifacts,
) -> DigestFinalizationPlanV1 {
    let mut nodes = Vec::new();
    let mut image_inputs = Vec::new();
    let mut body_inputs = Vec::new();
    for registration in registrations {
        let stackmap_key = DigestNodeKey::stackmap_record(registration.site);
        let stackmap_source = DigestNodeId::from_key(&stackmap_key).unwrap();
        let stackmap = DigestNodeV1::new(
            stackmap_key,
            Vec::new(),
            vec![DigestPatchIntentKey::new(
                stackmap_source,
                registration.plan.id(),
                DefinitionAtomRole::Primary,
                DigestSemanticFieldRole::NormalizedStackmap,
            )],
        )
        .unwrap();
        body_inputs.push(DigestInputRefV1::from_node(&stackmap));
        image_inputs.push(DigestInputRefV1::from_node(&stackmap));
        nodes.push(stackmap);
    }
    let body_definition = definition_plan(body);
    let body_primary = ObjectDefinitionAtomId::from_key(&ObjectDefinitionAtomKey::new(
        body_definition,
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let body_key = DigestNodeKey::object_definition(body_primary);
    let body_source = DigestNodeId::from_key(&body_key).unwrap();
    let body_node = DigestNodeV1::new(
        body_key,
        body_inputs,
        vec![DigestPatchIntentKey::new(
            body_source,
            callable_registration.plan.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::CallableBodyDefinition,
        )],
    )
    .unwrap();
    image_inputs.push(DigestInputRefV1::from_node(&body_node));
    nodes.push(body_node);

    let descriptor_key =
        DigestNodeKey::object_definition(type_registration.descriptor_primary.id());
    let descriptor_source = DigestNodeId::from_key(&descriptor_key).unwrap();
    let descriptor = DigestNodeV1::new(
        descriptor_key,
        Vec::new(),
        vec![DigestPatchIntentKey::new(
            descriptor_source,
            type_registration.registration_plan.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::DescriptorDefinition,
        )],
    )
    .unwrap();
    let layout_key = DigestNodeKey::layout(type_registration.layout.id());
    let layout_source = DigestNodeId::from_key(&layout_key).unwrap();
    let layout = DigestNodeV1::new(
        layout_key,
        Vec::new(),
        vec![DigestPatchIntentKey::new(
            layout_source,
            type_registration.registration_plan.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::Layout,
        )],
    )
    .unwrap();
    image_inputs.extend([
        DigestInputRefV1::from_node(&descriptor),
        DigestInputRefV1::from_node(&layout),
    ]);
    nodes.extend([descriptor, layout]);

    let static_layout_key = DigestNodeKey::layout(static_storage.layout.id());
    let static_layout_source = DigestNodeId::from_key(&static_layout_key).unwrap();
    let static_layout = DigestNodeV1::new(
        static_layout_key,
        Vec::new(),
        vec![DigestPatchIntentKey::new(
            static_layout_source,
            static_storage.registration_plan.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::Layout,
        )],
    )
    .unwrap();
    let static_scan_key = DigestNodeKey::scan(static_storage.scan.id());
    let static_scan_source = DigestNodeId::from_key(&static_scan_key).unwrap();
    let static_scan = DigestNodeV1::new(
        static_scan_key,
        Vec::new(),
        vec![DigestPatchIntentKey::new(
            static_scan_source,
            static_storage.registration_plan.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::Scan,
        )],
    )
    .unwrap();
    image_inputs.extend([
        DigestInputRefV1::from_node(&static_layout),
        DigestInputRefV1::from_node(&static_scan),
    ]);
    nodes.extend([static_layout, static_scan]);
    nodes.push(
        DigestNodeV1::new(
            DigestNodeKey::runtime_image(foundation.producer()),
            image_inputs,
            Vec::new(),
        )
        .unwrap(),
    );
    DigestFinalizationPlanV1::new(nodes, foundation).unwrap()
}

fn definition_plan(owner: scoop_identity::PersistentCallableBodyId) -> ObjectDefinitionPlanId {
    let key = ObjectDefinitionPlanKey::strong(
        ConeIdentity::SINGLE_FILE,
        StrongDefinitionEntity::callable_body(owner),
        StrongDefinitionRole::CallableBody,
    )
    .unwrap();
    ObjectDefinitionPlanId::from_key(&key).unwrap()
}

fn function_id(name: &str) -> PersistentFunctionId {
    PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        source_site(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap()
}

fn exact_type(name: &str) -> PersistentExactTypeId {
    let nominal = PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
        source_site(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    ))
    .unwrap();
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap()
}

fn immortal_key() -> ImmortalObjectKey {
    let property = property_id("text");
    ImmortalObjectKey::string_constant(
        ImmortalObjectOwner::Property(PropertyOwner::Property(property)),
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::StringConstant, 0),
            [],
        ),
    )
}

fn property_id(name: &str) -> PersistentPropertyId {
    PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        source_site(),
        CanonicalIdentifier::new(name).unwrap(),
    ))
    .unwrap()
}

fn source_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
