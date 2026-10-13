use scoop_lir as lir;

use super::LoweredFunction;

/// Safepoint completion inserts the mandatory gateway poll before this call.
pub(crate) fn initialize_task(
    gateway: &mut LoweredFunction,
    descriptor: lir::TypeDescriptorRef,
    planned: &lir::ScoopAbiSignature,
) {
    let function = &mut gateway.function;
    let lir::AbiReturn::Direct(reference) = planned.result() else {
        unreachable!("task initialization returns a managed reference")
    };
    let signature = function
        .call_targets
        .direct_signatures
        .alloc(lir::DirectCallSignature::new(
            planned.arguments().to_vec(),
            reference.clone(),
            planned.calling_convention(),
        ));
    let target = function
        .call_targets
        .managed_targets
        .direct
        .alloc(lir::CallTarget {
            destination: lir::ManagedCallDestination::runtime(
                lir::ManagedRuntimeFunction::ContextEnsureRoot,
            ),
            signature,
        });
    let out = function.temps.alloc(lir::Temp {
        ty: lir::MANAGED_PTR,
    });
    function.blocks[function.entry].instructions.insert(
        0,
        lir::Instruction::Call {
            site: lir::CallSite::Managed(lir::ManagedCallSite {
                call: lir::ManagedTypedCall::Direct {
                    target,
                    args: vec![lir::AbiCallArgument::Direct(lir::Value::TypeDescriptor(
                        descriptor,
                    ))],
                    out,
                },
                live: lir::StatepointLiveSet::default(),
                safepoint: gateway
                    .pending_safepoints
                    .allocate(lir::SafepointSiteRole::ManagedCall),
            }),
        },
    );
}
