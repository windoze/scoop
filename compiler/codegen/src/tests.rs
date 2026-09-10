use inkwell::OptimizationLevel;
use inkwell::targets::FileType;
use la_arena::Arena;
use scoop_identity::{
    CallableMaterializationContext, CallbackApplicationKey, CallbackMode, CallbackParameterIndex,
    CallbackRegistrationKey, CanonicalIdentifier, CborIdentityRecord, ConeIdentity,
    CoreBuiltinNominal, DeclarationScope, DefinitionOwnerChain, Effect, InitializationUnitKey,
    LexicalCallableParent, PackagePath, PersistentCallbackApplicationId, PersistentFunctionId,
    PersistentPropertyId, SignatureCallableShape, SignatureTypeKey, SourceCAbiFunctionSignature,
    SourceCAbiReturn, SourceDeclarationKey, SourceDeclarationSite, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};
use scoop_lir::{
    BasicBlock, CODE_PTR, CallSite, CallTargets, CoroutineAdapterState, DirectCallSignature,
    DispatchKind, DispatchSlot, EnumDef, EnumFieldRepr, EnumRepr, EnumVariantRepr, GcEffect,
    Global, GlobalInit, IndirectResultCallSignature, ItableRecord, Layout, LayoutKind, LirMeta,
    Local, MANAGED_PTR, METADATA_PTR, MachineScalarValue, NativeBorrowedResultRoot, PointerKind,
    RAW_PTR, Temp, TypeDescriptor, TypeDescriptorRef, TypeDescriptorScan, TypedCall,
    VoidCallSignature, WellKnownLayouts, WellKnownTypeDescriptors,
};

use super::*;

mod support;

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
mod c_layout;
mod closures;
mod constants;
mod enums;
mod exceptions;
mod initialization;
mod moving_gc;
mod objects;
mod platform;
mod scoop_abi;
mod smoke;
mod statepoints;
mod validation_boundaries;

use enums::enum_module;
use exceptions::exceptions_module;
use objects::heap_module;

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
