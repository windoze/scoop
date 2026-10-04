use inkwell::OptimizationLevel;
use inkwell::targets::FileType;
use la_arena::Arena;
use scoop_identity::{
    CallableMaterializationContext, CallbackApplicationKey, CallbackMode, CallbackParameterIndex,
    CallbackRegistrationKey, CanonicalIdentifier, CborIdentityRecord, ConeIdentity,
    CoreBuiltinNominal, DeclarationScope, DefinitionOwnerChain, Effect, ExactTypeKey,
    FieldIdentityKey, InitializationUnitKey, LexicalCallableParent, PackagePath,
    PersistentCallbackApplicationId, PersistentExactTypeId, PersistentFieldId,
    PersistentFunctionId, PersistentPropertyId, PersistentTypeId, SignatureCallableShape,
    SignatureTypeKey, SourceCAbiFunctionSignature, SourceCAbiReturn, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};
use scoop_lir::{
    ArrayElementStorageV1, BasicBlock, CODE_PTR, CallSite, CallTargets, CoroutineAdapterState,
    DirectCallSignature, DispatchKind, DispatchSlot, EnumDef, EnumFieldRepr, EnumRepr,
    EnumVariantRepr, GcEffect, Global, GlobalInit, IndirectResultCallSignature, Layout, LayoutKind,
    LirMeta, Local, MANAGED_PTR, METADATA_PTR, MachineScalarValue, NativeBorrowedResultRoot,
    PointerKind, RAW_PTR, Temp, TypeDescriptor, TypeDescriptorRef, TypeInstanceShapeV1, TypedCall,
    VoidCallSignature, WellKnownTypeDescriptors,
};

use super::*;

mod support;

pub(crate) use support::test_physical_exact;
use support::*;

#[path = "runtime_collector_tests.rs"]
mod runtime_collector_tests;

#[path = "runtime_eh_tests.rs"]
mod runtime_eh_tests;

#[path = "runtime_eh_lifecycle_tests.rs"]
mod runtime_eh_lifecycle_tests;

#[path = "runtime_eh_personality_tests.rs"]
mod runtime_eh_personality_tests;

mod arrays;
mod arrays_zst;
mod boxing;
mod c_bridge_objects;
mod c_layout;
mod closures;
mod constants;
mod dependency_external;
mod enums;
mod exceptions;
mod external_type_descriptors;
mod image;
mod initialization;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux_runtime;
mod moving_gc;
mod object_partition;
mod objects;
mod platform;
mod pointers;
mod scoop_abi;
mod smoke;
mod statepoints;
mod strong_shapes;
mod validation_boundaries;
mod zst_places;

use enums::enum_module;
use exceptions::exceptions_module;
use objects::heap_module;

fn test_field_identity(owner_name: &str, field_name: &str) -> PersistentFieldId {
    fn identifier(value: &str) -> CanonicalIdentifier {
        let encoded = format!(
            "test{}",
            value
                .bytes()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        CanonicalIdentifier::new(&encoded).unwrap()
    }

    let owner = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        identifier(owner_name),
        SourceNominalKind::Struct,
        0,
    );
    PersistentFieldId::from_key(
        &FieldIdentityKey::source_declared(&owner, identifier(field_name)).unwrap(),
    )
    .unwrap()
}

fn initialization_unit_identity(
    cone: ConeIdentity,
    name: &str,
) -> scoop_lir::InitializationUnitIdentityRecord {
    let site = SourceDeclarationSite::new(
        cone,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let property = SourceDeclarationKey::property(site, CanonicalIdentifier::new(name).unwrap());
    let owner = PersistentPropertyId::from_source_declaration(&property).unwrap();
    CborIdentityRecord::from_key(InitializationUnitKey::TopLevelProperty(owner)).unwrap()
}

fn callback_application(ordinal: u32) -> PersistentCallbackApplicationId {
    let site = SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let function = PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        site,
        CanonicalIdentifier::new("callbackOwner").unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap();
    let unit = CoreBuiltinNominal::Unit.identity_record().id();
    let registration = CallbackRegistrationKey::new(
        LexicalCallableParent::function(function),
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::CallbackConversion, ordinal),
            [],
        ),
        SourceCAbiFunctionSignature::new(Vec::new(), SourceCAbiReturn::Void),
        CallbackParameterIndex::new(0),
        SignatureCallableShape::new(
            Effect::Ordinary,
            None,
            Vec::new(),
            SignatureTypeKey::Nominal(unit),
        ),
        CallbackMode::Reusable,
    );
    let application = CallbackApplicationKey::new(
        &registration,
        CallableMaterializationContext::NoSubstitution,
    )
    .unwrap();
    PersistentCallbackApplicationId::from_key(&application).unwrap()
}
