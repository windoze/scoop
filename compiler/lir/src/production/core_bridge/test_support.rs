use crate::{CallableAbiRecordV1, CallingConvention, ConeIdentity, ExternalCallableRootPlan};
use scoop_identity::{
    CanonicalScoopAbiFunctionSignature, GcEffect as CanonicalGcEffect, PersistentFunctionId,
    StrongCallableDefinitionOwner,
};

pub(crate) fn core_lir_cycle_thrower_for_test() -> CallableAbiRecordV1 {
    let declaration = scoop_identity::SourceDeclarationKey::function(
        scoop_identity::SourceDeclarationSite::new(
            ConeIdentity::CORE,
            scoop_identity::PackagePath::root(),
            scoop_identity::DefinitionOwnerChain::top_level(),
            scoop_identity::DeclarationScope::ConeWide,
        )
        .unwrap(),
        scoop_identity::CanonicalIdentifier::new("__scoopThrowInitializationCycle").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let definition = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
    let string_declaration = scoop_identity::CborIdentityRecord::from_key(
        scoop_identity::SourceDeclarationKey::nominal(
            scoop_identity::SourceDeclarationSite::new(
                ConeIdentity::CORE,
                scoop_identity::PackagePath::root(),
                scoop_identity::DefinitionOwnerChain::top_level(),
                scoop_identity::DeclarationScope::ConeWide,
            )
            .unwrap(),
            scoop_identity::CanonicalIdentifier::new("String").unwrap(),
            scoop_identity::SourceNominalKind::Class,
            0,
        ),
    )
    .unwrap();
    let string = scoop_identity::PersistentExactTypeId::from_key(
        &scoop_identity::ExactTypeKey::Nominal(string_declaration.id()),
    )
    .unwrap();
    let unit =
        scoop_identity::PersistentExactTypeId::from_key(&scoop_identity::ExactTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ))
        .unwrap();
    let signature = scoop_identity::ExactCallableSignature::new(
        scoop_identity::Effect::Ordinary,
        None,
        vec![string],
        unit,
    );
    let string_storage = scoop_identity::CanonicalScoopStorage::new(
        string,
        8,
        std::num::NonZeroU64::new(8).unwrap(),
        scoop_identity::ScoopAbiValueShape::Scalar,
    );
    let abi = CanonicalScoopAbiFunctionSignature::new(
        signature,
        vec![scoop_identity::ScoopAbiArgument::direct(string_storage).unwrap()],
        scoop_identity::ScoopAbiReturn::unit_void(),
        CanonicalGcEffect::Managed,
    )
    .unwrap();
    CallableAbiRecordV1::new(
        scoop_identity::ConeIdentity::CORE,
        StrongCallableDefinitionOwner::Function(definition),
        abi,
        CallingConvention::Cdecl,
        ExternalCallableRootPlan::ManagedStatepoint,
    )
    .unwrap()
}
