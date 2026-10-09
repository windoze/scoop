use scoop_lir as lir;

use super::LoweredFunction;

/// Safepoint completion inserts the mandatory gateway poll before this call.
pub(crate) fn initialize_task(gateway: &mut LoweredFunction, descriptor: lir::TypeDescriptorRef) {
    let function = &mut gateway.function;
    let metadata = lir::AbiValue::new(
        lir::METADATA_PTR,
        lir::AbiNonZeroLayout::new(8, 8).expect("metadata pointer layout"),
        lir::RefScan::None,
    )
    .expect("metadata pointers are GC-free");
    let reference = lir::AbiValue::new(
        lir::MANAGED_PTR,
        lir::AbiNonZeroLayout::new(8, 8).expect("managed pointer layout"),
        lir::RefScan::References(vec![0]),
    )
    .expect("Task Context has one reference leaf");
    let signature = function
        .call_targets
        .direct_signatures
        .alloc(lir::DirectCallSignature::new(
            vec![lir::AbiArgument::Direct(metadata.into())],
            reference,
            lir::CallingConvention::Cdecl,
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
