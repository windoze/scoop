use super::*;
use scoop_lir::*;

pub(super) fn invoke_main(
    main: LocalFunctionRef,
    signature: &ScoopAbiSignature,
    args: Vec<AbiCallArgument>,
    targets: &mut CallTargets,
    temps: &mut Arena<Temp>,
    safepoints: &mut PendingSafepointSites,
    exits: (BlockId, BlockId),
) -> (InvokeSite, Value) {
    let (normal, unwind) = exits;
    let result = match signature.result() {
        AbiReturn::UnitVoid => Value::IntegerConst(LirIntegerConstant::Signed32(0)),
        AbiReturn::Direct(result) => Value::Temp(temps.alloc(Temp {
            ty: result.storage_type().clone(),
        })),
        AbiReturn::Indirect(_) | AbiReturn::ElidedZst(_) => {
            unreachable!("source main returns Unit or Int")
        }
    };
    macro_rules! call {
        ($targets:expr, $destination:expr, $call:ident) => {
            match signature.result() {
                AbiReturn::UnitVoid => {
                    let signature = targets.void_signatures.alloc(VoidCallSignature::new(
                        signature.arguments().to_vec(),
                        signature.calling_convention(),
                    ));
                    let target = $targets.void.alloc(CallTarget {
                        destination: $destination,
                        signature,
                    });
                    $call::Void { target, args }
                }
                AbiReturn::Direct(value) => {
                    let signature = targets.direct_signatures.alloc(DirectCallSignature::new(
                        signature.arguments().to_vec(),
                        value.clone(),
                        signature.calling_convention(),
                    ));
                    let target = $targets.direct.alloc(CallTarget {
                        destination: $destination,
                        signature,
                    });
                    let Value::Temp(out) = result else {
                        unreachable!("direct main has a result temporary")
                    };
                    $call::Direct { target, out, args }
                }
                AbiReturn::Indirect(_) | AbiReturn::ElidedZst(_) => {
                    unreachable!("source main returns Unit or Int")
                }
            }
        };
    }
    let invoke = match main {
        LocalFunctionRef::Managed(main) => InvokeSite::Managed(ManagedInvokeSite {
            call: call!(
                targets.managed_targets,
                ManagedCallDestination::local(main),
                ManagedTypedCall
            ),
            safepoint: safepoints.allocate(SafepointSiteRole::ManagedInvoke),
            roots: ExceptionalRootSet::default(),
            normal,
            unwind,
        }),
        LocalFunctionRef::NoGc(main) => InvokeSite::NoGc(NoGcInvokeSite {
            call: call!(
                targets.no_gc_targets,
                NoGcCallDestination::local(main),
                NoGcTypedCall
            ),
            normal,
            unwind,
        }),
    };
    (invoke, result)
}
