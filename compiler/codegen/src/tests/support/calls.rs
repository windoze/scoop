//! Typed test call construction across managed and native protocols.

use super::*;

pub(in crate::tests) enum TestCallProtocol {
    Managed {
        safepoint: u64,
        destination: scoop_lir::ManagedCallDestination,
    },
    NoGc {
        destination: scoop_lir::NoGcCallDestination,
    },
    NativeSafe {
        safepoint: u64,
        destination: scoop_lir::NativeSafeCallDestination,
    },
    NativeBorrowed {
        safepoint: u64,
        destination: scoop_lir::NativeBorrowedCallDestination,
        result: NativeBorrowedResultRoot,
    },
}

pub(in crate::tests) enum TestTypedCall {
    Void {
        signature: scoop_lir::VoidCallSignatureId,
        args: Vec<Value>,
    },
    Direct {
        signature: scoop_lir::DirectCallSignatureId,
        out: TempId,
        args: Vec<Value>,
    },
    IndirectResult {
        signature: scoop_lir::IndirectResultCallSignatureId,
        storage: scoop_lir::LocalId,
        args: Vec<Value>,
    },
}

pub(in crate::tests) fn bind_test_call<Destination: Copy>(
    targets: &mut scoop_lir::ProtocolCallTargets<Destination>,
    destination: Destination,
    call: TestTypedCall,
) -> scoop_lir::TypedCall<Destination> {
    match call {
        TestTypedCall::Void { signature, args } => {
            let target = targets.void.alloc(scoop_lir::CallTarget {
                destination,
                signature,
            });
            scoop_lir::TypedCall::Void { target, args }
        }
        TestTypedCall::Direct {
            signature,
            out,
            args,
        } => {
            let target = targets.direct.alloc(scoop_lir::CallTarget {
                destination,
                signature,
            });
            scoop_lir::TypedCall::Direct { target, out, args }
        }
        TestTypedCall::IndirectResult {
            signature,
            storage,
            args,
        } => {
            let target = targets.indirect_result.alloc(scoop_lir::CallTarget {
                destination,
                signature,
            });
            scoop_lir::TypedCall::IndirectResult {
                target,
                storage,
                args,
            }
        }
    }
}

pub(in crate::tests) fn test_safepoint(raw: u64) -> scoop_lir::SafepointId {
    scoop_lir::SafepointId::new(raw).expect("test safepoint ids are non-zero")
}

pub(in crate::tests) fn statepoint_value(
    source: scoop_lir::CallerRootSource,
    ty: LirType,
    offsets: &[u64],
) -> scoop_lir::StatepointLiveValue {
    scoop_lir::StatepointLiveValue {
        source,
        ty,
        leaves: scoop_lir::ManagedLeafPaths::new(
            offsets
                .iter()
                .copied()
                .map(|byte_offset| scoop_lir::ManagedLeafPath { byte_offset })
                .collect(),
        )
        .expect("test roots have at least one sorted managed leaf"),
    }
}

pub(in crate::tests) fn statepoint_live(
    values: Vec<scoop_lir::StatepointLiveValue>,
) -> scoop_lir::StatepointLiveSet {
    scoop_lir::StatepointLiveSet::new(values).expect("test roots are source ordered and unique")
}

pub(in crate::tests) fn set_managed_live(site: &mut CallSite, live: scoop_lir::StatepointLiveSet) {
    let CallSite::Managed(site) = site else {
        panic!("test root plan requires a managed call site")
    };
    site.live = live;
}

pub(in crate::tests) fn protocol_site(
    targets: &mut CallTargets,
    protocol: TestCallProtocol,
    call: TestTypedCall,
) -> CallSite {
    match protocol {
        TestCallProtocol::Managed {
            safepoint,
            destination,
        } => CallSite::Managed(scoop_lir::ManagedCallSite {
            call: bind_test_call(&mut targets.managed_targets, destination, call),
            safepoint: test_safepoint(safepoint),
            live: scoop_lir::StatepointLiveSet::default(),
        }),
        TestCallProtocol::NoGc { destination } => CallSite::NoGc(scoop_lir::NoGcCallSite {
            call: bind_test_call(&mut targets.no_gc_targets, destination, call),
        }),
        TestCallProtocol::NativeSafe {
            safepoint,
            destination,
        } => CallSite::NativeSafe(scoop_lir::NativeSafeCallSite {
            call: bind_test_call(&mut targets.native_safe_targets, destination, call),
            safepoint: test_safepoint(safepoint),
            roots: scoop_lir::NativeSafeRootSet::default(),
        }),
        TestCallProtocol::NativeBorrowed {
            safepoint,
            destination,
            result,
        } => {
            let call = bind_test_call(&mut targets.native_borrowed_targets, destination, call);
            let call = targets.bind_native_borrowed_call(call, result);
            CallSite::NativeBorrowed(scoop_lir::NativeBorrowedCallSite {
                call,
                safepoint: test_safepoint(safepoint),
                roots: scoop_lir::NativeBorrowedRootSet::default(),
            })
        }
    }
}

pub(in crate::tests) fn void_site(
    targets: &mut CallTargets,
    protocol: TestCallProtocol,
    params: Vec<LirType>,
    args: Vec<Value>,
) -> CallSite {
    let signature = targets.void_signatures.alloc(VoidCallSignature {
        params,
        calling_convention: scoop_lir::CallingConvention::Cdecl,
    });
    protocol_site(targets, protocol, TestTypedCall::Void { signature, args })
}

pub(in crate::tests) fn direct_site(
    targets: &mut CallTargets,
    protocol: TestCallProtocol,
    params: Vec<LirType>,
    result: (LirType, RefScan),
    out: TempId,
    args: Vec<Value>,
) -> CallSite {
    let signature = targets.direct_signatures.alloc(DirectCallSignature {
        params,
        result: result.0,
        result_scan: result.1,
        calling_convention: scoop_lir::CallingConvention::Cdecl,
    });
    protocol_site(
        targets,
        protocol,
        TestTypedCall::Direct {
            signature,
            out,
            args,
        },
    )
}

pub(in crate::tests) fn indirect_result_site(
    targets: &mut CallTargets,
    protocol: TestCallProtocol,
    params: Vec<LirType>,
    result: (LirType, RefScan),
    storage: scoop_lir::LocalId,
    args: Vec<Value>,
) -> CallSite {
    let signature = targets
        .indirect_result_signatures
        .alloc(IndirectResultCallSignature {
            params,
            result: ResultStorage {
                ty: result.0,
                scan: result.1,
            },
            calling_convention: scoop_lir::CallingConvention::Cdecl,
        });
    protocol_site(
        targets,
        protocol,
        TestTypedCall::IndirectResult {
            signature,
            storage,
            args,
        },
    )
}

pub(in crate::tests) fn managed_invoke(
    site: CallSite,
    normal: scoop_lir::BlockId,
    unwind: scoop_lir::BlockId,
) -> scoop_lir::InvokeSite {
    let CallSite::Managed(site) = site else {
        panic!("test invoke helper requires a managed call site")
    };
    scoop_lir::InvokeSite::Managed(scoop_lir::ManagedInvokeSite {
        call: site.call,
        safepoint: site.safepoint,
        roots: scoop_lir::ExceptionalRootSet::default(),
        normal,
        unwind,
    })
}

pub(in crate::tests) fn dispatch_destination(
    targets: &mut CallTargets,
    table: Value,
    index: u32,
) -> scoop_lir::ManagedCallDestination {
    let slot = targets.dispatch_slots.alloc_managed(DispatchSlot {
        kind: DispatchKind::Virtual,
        index,
    });
    scoop_lir::ManagedCallDestination::dispatch(table, slot)
}

pub(in crate::tests) fn managed_runtime(
    function: scoop_lir::ManagedRuntimeFunction,
) -> scoop_lir::ManagedCallDestination {
    scoop_lir::ManagedCallDestination::runtime(function)
}

pub(in crate::tests) fn no_gc_runtime(
    function: scoop_lir::NoGcRuntimeFunction,
) -> scoop_lir::NoGcCallDestination {
    scoop_lir::NoGcCallDestination::runtime(function)
}

pub(in crate::tests) fn managed_local(index: u32) -> scoop_lir::ManagedCallDestination {
    let mut identities = scoop_lir::LocalFunctionIdentities::default();
    let mut reference = identities.alloc_managed();
    for _ in 0..index {
        reference = identities.alloc_managed();
    }
    assert_eq!(reference.declaration().into_u32(), index);
    scoop_lir::ManagedCallDestination::local(reference)
}
