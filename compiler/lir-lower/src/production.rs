use la_arena::Arena;

mod context;
mod root;
pub(super) use context::initialize_task;

use root::root_gateway;
pub(super) use root::{RootArguments, RootMain};

use super::{function::LoweredFunction, safepoints::PendingSafepointSites};

/// Projects the validated MIR entry branch into the LIR production source.
/// The complete HIR proof remains intact through this stage boundary.
pub fn lower_entry_production_source(
    entry: &scoop_mir::EntryMirBridgeBranchV1,
) -> scoop_lir::EntryProductionSourceV1 {
    match entry {
        scoop_mir::EntryMirBridgeBranchV1::Library => scoop_lir::EntryProductionSourceV1::Library,
        scoop_mir::EntryMirBridgeBranchV1::Executable(bridge) => {
            scoop_lir::EntryProductionSourceV1::executable(bridge.source().clone())
        }
    }
}

pub(super) fn lower_root_artifacts(
    producer: scoop_identity::ConeIdentity,
    source: &scoop_identity::ExecutableSourceEntryIdentity,
    main: RootMain<'_>,
    layout: scoop_lir::StaticStorageLayout,
    globals: &mut Arena<scoop_lir::Global>,
) -> LoweredFunction {
    let failure_root = globals.alloc(scoop_lir::Global {
        address_kind: scoop_lir::PointerKind::Raw,
        scan: scoop_lir::RefScan::References(vec![0]),
        init: scoop_lir::GlobalInit::Storage {
            identity: scoop_lir::StaticStorageIdentity::root_entry_failure_root(
                producer,
                source.main(),
                scoop_lir::MaterializationRoot::cone_owned(),
            )
            .expect("a validated root entry derives one failure-storage identity"),
            layout,
            ty: scoop_lir::MANAGED_PTR,
            initial_state: scoop_lir::LirStaticInitialState::ZeroedForRuntimeUnit,
        },
    });

    root_gateway(producer, source.main(), main, failure_root)
}

pub(super) fn lower_initialization_startup_gateway(
    unit: scoop_identity::PersistentInitializationUnitId,
    ensure: scoop_lir::ManagedLocalFunctionRef,
) -> LoweredFunction {
    let callable_body = scoop_lir::CallableBodyIdentity::for_initialization_startup_gateway(unit)
        .expect("a validated eager initialization unit derives one startup-gateway identity");
    let mut call_targets = scoop_lir::CallTargets::default();
    let signature = call_targets
        .void_signatures
        .alloc(scoop_lir::VoidCallSignature::new(
            Vec::new(),
            scoop_lir::CallingConvention::Cdecl,
        ));
    let target = call_targets
        .managed_targets
        .void
        .alloc(scoop_lir::CallTarget {
            destination: scoop_lir::ManagedCallDestination::local(ensure),
            signature,
        });
    let mut pending_safepoints = PendingSafepointSites::default();
    let mut blocks = Arena::new();
    let entry = blocks.alloc(scoop_lir::BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: scoop_lir::Terminator::Unreachable,
    });
    let success = blocks.alloc(scoop_lir::BasicBlock {
        name: "success".to_string(),
        instructions: Vec::new(),
        terminator: scoop_lir::Terminator::Return {
            value: Some(scoop_lir::Value::IntegerConst(
                scoop_lir::LirIntegerConstant::Unsigned32(0),
            )),
        },
    });
    let failure = blocks.alloc(scoop_lir::BasicBlock {
        name: "failure".to_string(),
        instructions: Vec::new(),
        terminator: scoop_lir::Terminator::Return {
            value: Some(scoop_lir::Value::IntegerConst(
                scoop_lir::LirIntegerConstant::Unsigned32(1),
            )),
        },
    });
    blocks[entry]
        .instructions
        .push(scoop_lir::Instruction::Invoke {
            site: scoop_lir::InvokeSite::Managed(scoop_lir::ManagedInvokeSite {
                call: scoop_lir::ManagedTypedCall::Void {
                    target,
                    args: Vec::new(),
                },
                safepoint: pending_safepoints.allocate(scoop_lir::SafepointSiteRole::ManagedInvoke),
                roots: scoop_lir::ExceptionalRootSet::default(),
                normal: success,
                unwind: failure,
            }),
        });
    blocks[entry].terminator = scoop_lir::Terminator::Br(success);
    let mut temps = Arena::new();
    let exception_record = temps.alloc(scoop_lir::Temp {
        ty: scoop_lir::LirType::ExceptionRecord,
    });
    let raw_exception = temps.alloc(scoop_lir::Temp {
        ty: scoop_lir::RAW_PTR,
    });
    let managed_exception = temps.alloc(scoop_lir::Temp {
        ty: scoop_lir::MANAGED_PTR,
    });
    blocks[failure].instructions.extend([
        scoop_lir::Instruction::LandingPad {
            record: exception_record,
            raw: raw_exception,
        },
        scoop_lir::Instruction::BeginCatch {
            out: managed_exception,
            raw: scoop_lir::Value::Temp(raw_exception),
        },
        scoop_lir::Instruction::EndCatch,
    ]);
    let result = scoop_lir::AbiValue::new(
        scoop_lir::LirType::I32,
        scoop_lir::AbiNonZeroLayout::new(4, 4)
            .expect("the startup-gateway result has a valid uint32 layout"),
        scoop_lir::RefScan::None,
    )
    .expect("the startup-gateway result layout matches uint32 storage");
    LoweredFunction {
        function: scoop_lir::Function {
            callable_body,
            gc_effect: scoop_lir::GcEffect::Managed,
            signature: scoop_lir::ScoopAbiSignature::new(
                Vec::new(),
                scoop_lir::AbiReturn::Direct(result.into()),
                scoop_lir::CallingConvention::Cdecl,
            ),
            call_targets,
            safepoints: scoop_lir::SafepointIdentities::default(),
            locals: Arena::new(),
            temps,
            blocks,
            entry,
        },
        loop_header_polls: Vec::new(),
        pending_safepoints,
    }
}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CallableOwner, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, CoreBuiltinNominal,
        DeclarationScope, DefinitionOwnerChain, ExactCallableSignature, ExactTypeKey,
        ExecutableSourceEntryIdentity, PackagePath, PersistentExactTypeId, SourceDeclarationKey,
        SourceDeclarationSite,
    };

    use super::lower_entry_production_source;

    #[test]
    fn lowering_preserves_the_complete_mir_entry_proof() {
        let source = source();
        let implementation = CallableOwner::Function(source.declaration());
        let mir = scoop_mir::EntryMirBridgeBranchV1::Executable(Box::new(
            scoop_mir::EntryMirBridgeV1::new(source.clone(), implementation).unwrap(),
        ));

        assert_eq!(
            lower_entry_production_source(&mir),
            scoop_lir::EntryProductionSourceV1::executable(source)
        );
        assert_eq!(
            lower_entry_production_source(&scoop_mir::EntryMirBridgeBranchV1::Library),
            scoop_lir::EntryProductionSourceV1::Library
        );
    }

    fn source() -> ExecutableSourceEntryIdentity {
        let declaration = CborIdentityRecord::from_key(SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                ConeIdentity::SINGLE_FILE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("main").unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap();
        let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap();
        ExecutableSourceEntryIdentity::try_new(
            &declaration,
            ExactCallableSignature::new(scoop_identity::Effect::Ordinary, None, Vec::new(), unit),
        )
        .unwrap()
    }
}
