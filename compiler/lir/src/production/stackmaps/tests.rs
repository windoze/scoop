use scoop_identity::{
    CanonicalIdentifier, DeclarationScope, DefinitionOwnerChain, PackagePath, PersistentFunctionId,
    SourceDeclarationKey, SourceDeclarationSite,
};

use super::*;
use crate::{
    AbiReturn, Arena, BasicBlock, CallTarget, CallTargets, CallableBodyIdentity, CallingConvention,
    LirType, ManagedCallDestination, ManagedLeafPath, ManagedLeafPaths, ManagedPollSite,
    ManagedRuntimeFunction, SafepointIdentities, SafepointIdentity, ScoopAbiSignature,
    StatepointLiveValue, Terminator, VoidCallSignature,
};

#[test]
fn derives_canonical_site_semantics_and_managed_leaf_count() {
    let function = poll_function(
        "roots",
        GcEffect::Managed,
        SafepointSiteRef::from_u32(7),
        SafepointSiteRole::ManagedPoll,
        live_roots(&[0, 8]),
        IdentityMode::Matching,
        0,
    );

    let plans =
        StrongSafepointSemanticPlanSetV1::from_functions(ConeIdentity::SINGLE_FILE, &[function])
            .unwrap();

    assert_eq!(plans.producer(), ConeIdentity::SINGLE_FILE);
    assert_eq!(plans.sites().len(), 1);
    let site = plans.sites()[0];
    assert_eq!(site.owner(), callable_body("roots").id());
    assert_eq!(site.role(), SafepointSiteRole::ManagedPoll);
    assert_eq!(site.root_pair_count(), 2);
    assert_eq!(site.safepoint(), SafepointId::derive(site.site()).unwrap());
}

#[test]
fn plans_are_sorted_by_persistent_site_not_function_order() {
    let first = poll_function(
        "zeta",
        GcEffect::Managed,
        SafepointSiteRef::from_u32(0),
        SafepointSiteRole::ManagedPoll,
        StatepointLiveSet::default(),
        IdentityMode::Matching,
        0,
    );
    let second = poll_function(
        "alpha",
        GcEffect::Managed,
        SafepointSiteRef::from_u32(0),
        SafepointSiteRole::ManagedPoll,
        StatepointLiveSet::default(),
        IdentityMode::Matching,
        0,
    );

    let plans = StrongSafepointSemanticPlanSetV1::from_functions(
        ConeIdentity::SINGLE_FILE,
        &[first, second],
    )
    .unwrap();

    assert!(
        plans
            .sites()
            .windows(2)
            .all(|pair| pair[0].site() < pair[1].site())
    );
}

#[test]
fn rejects_missing_unused_and_reused_identity_relations() {
    let missing = poll_function(
        "missing",
        GcEffect::Managed,
        SafepointSiteRef::from_u32(4),
        SafepointSiteRole::ManagedPoll,
        StatepointLiveSet::default(),
        IdentityMode::Missing,
        0,
    );
    assert!(matches!(
        StrongSafepointSemanticPlanSetV1::from_functions(ConeIdentity::SINGLE_FILE, &[missing]),
        Err(StrongSafepointSemanticPlanError::MissingIdentity { .. })
    ));

    let unused = function_with_unused_identity("unused");
    assert!(matches!(
        StrongSafepointSemanticPlanSetV1::from_functions(ConeIdentity::SINGLE_FILE, &[unused]),
        Err(StrongSafepointSemanticPlanError::UnreferencedIdentity { .. })
    ));

    let reused = function_with_reused_reference("reused");
    assert!(matches!(
        StrongSafepointSemanticPlanSetV1::from_functions(ConeIdentity::SINGLE_FILE, &[reused]),
        Err(StrongSafepointSemanticPlanError::ReusedFunctionReference { .. })
    ));
}

#[test]
fn rejects_nogc_owner_and_role_mismatches() {
    let no_gc = poll_function(
        "nogc",
        GcEffect::NoGc,
        SafepointSiteRef::from_u32(0),
        SafepointSiteRole::ManagedPoll,
        StatepointLiveSet::default(),
        IdentityMode::Matching,
        0,
    );
    assert!(matches!(
        StrongSafepointSemanticPlanSetV1::from_functions(ConeIdentity::SINGLE_FILE, &[no_gc]),
        Err(StrongSafepointSemanticPlanError::SafepointInNoGcFunction { .. })
    ));

    let wrong_role = poll_function(
        "wrongRole",
        GcEffect::Managed,
        SafepointSiteRef::from_u32(0),
        SafepointSiteRole::ManagedPoll,
        StatepointLiveSet::default(),
        IdentityMode::WrongRole,
        0,
    );
    assert!(matches!(
        StrongSafepointSemanticPlanSetV1::from_functions(ConeIdentity::SINGLE_FILE, &[wrong_role]),
        Err(StrongSafepointSemanticPlanError::RoleMismatch { .. })
    ));

    let wrong_owner = poll_function(
        "wrongOwner",
        GcEffect::Managed,
        SafepointSiteRef::from_u32(0),
        SafepointSiteRole::ManagedPoll,
        StatepointLiveSet::default(),
        IdentityMode::WrongOwner,
        0,
    );
    assert!(matches!(
        StrongSafepointSemanticPlanSetV1::from_functions(ConeIdentity::SINGLE_FILE, &[wrong_owner]),
        Err(StrongSafepointSemanticPlanError::OwnerMismatch { .. })
    ));
}

#[derive(Clone, Copy)]
enum IdentityMode {
    Matching,
    Missing,
    WrongRole,
    WrongOwner,
}

fn poll_function(
    name: &str,
    effect: GcEffect,
    reference: SafepointSiteRef,
    role: SafepointSiteRole,
    live: StatepointLiveSet,
    identity_mode: IdentityMode,
    ordinal: u32,
) -> Function {
    let body = callable_body(name);
    let mut call_targets = CallTargets::default();
    let signature = call_targets
        .void_signatures
        .alloc(VoidCallSignature::new(Vec::new(), CallingConvention::Cdecl));
    let target = call_targets.managed_targets.void.alloc(CallTarget {
        destination: ManagedCallDestination::runtime(ManagedRuntimeFunction::Safepoint),
        signature,
    });
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![Instruction::ManagedPoll {
            site: ManagedPollSite {
                target,
                safepoint: reference,
                live,
            },
        }],
        terminator: Terminator::Return { value: None },
    });
    let identity = match identity_mode {
        IdentityMode::Matching => Some(SafepointIdentity::new(body.id(), role, ordinal)),
        IdentityMode::Missing => None,
        IdentityMode::WrongRole => Some(SafepointIdentity::new(
            body.id(),
            SafepointSiteRole::ManagedCall,
            ordinal,
        )),
        IdentityMode::WrongOwner => Some(SafepointIdentity::new(
            callable_body("anotherOwner").id(),
            role,
            ordinal,
        )),
    };
    let safepoints = identity.map_or_else(SafepointIdentities::default, |identity| {
        SafepointIdentities::checked(vec![(reference, identity.unwrap())]).unwrap()
    });
    Function {
        callable_body: body,
        gc_effect: effect,
        signature: ScoopAbiSignature::new(
            Vec::new(),
            AbiReturn::UnitVoid,
            CallingConvention::Cdecl,
        ),
        call_targets,
        safepoints,
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    }
}

fn function_with_unused_identity(name: &str) -> Function {
    let callable_body = callable_body(name);
    let reference = SafepointSiteRef::from_u32(0);
    let identity =
        SafepointIdentity::new(callable_body.id(), SafepointSiteRole::ManagedPoll, 0).unwrap();
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Return { value: None },
    });
    Function {
        callable_body,
        gc_effect: GcEffect::Managed,
        signature: ScoopAbiSignature::new(
            Vec::new(),
            AbiReturn::UnitVoid,
            CallingConvention::Cdecl,
        ),
        call_targets: CallTargets::default(),
        safepoints: SafepointIdentities::checked(vec![(reference, identity)]).unwrap(),
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    }
}

fn function_with_reused_reference(name: &str) -> Function {
    let mut function = poll_function(
        name,
        GcEffect::Managed,
        SafepointSiteRef::from_u32(0),
        SafepointSiteRole::ManagedPoll,
        StatepointLiveSet::default(),
        IdentityMode::Matching,
        0,
    );
    let second = match &function.blocks[function.entry].instructions[0] {
        Instruction::ManagedPoll { site } => Instruction::ManagedPoll {
            site: ManagedPollSite {
                target: site.target,
                safepoint: site.safepoint,
                live: StatepointLiveSet::default(),
            },
        },
        _ => panic!("poll fixture must start with a managed poll"),
    };
    function.blocks[function.entry].instructions.push(second);
    function
}

fn live_roots(offsets: &[u64]) -> StatepointLiveSet {
    StatepointLiveSet::new(vec![StatepointLiveValue {
        source: crate::CallerRootSource::Param(0),
        ty: LirType::Aggregate(vec![crate::MANAGED_PTR; offsets.len()]),
        leaves: ManagedLeafPaths::new(
            offsets
                .iter()
                .copied()
                .map(|byte_offset| ManagedLeafPath { byte_offset })
                .collect(),
        )
        .unwrap(),
    }])
    .unwrap()
}

fn callable_body(name: &str) -> CallableBodyIdentity {
    CallableBodyIdentity::for_function(function_id(name)).unwrap()
}

fn function_id(name: &str) -> PersistentFunctionId {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let declaration = SourceDeclarationKey::function(
        site,
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    );
    PersistentFunctionId::from_source_declaration(&declaration).unwrap()
}
