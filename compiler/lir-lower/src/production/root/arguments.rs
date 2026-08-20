use super::*;
use scoop_lir::*;

pub(super) fn build_arguments(
    main: &RootMain<'_>,
    blocks: &mut Arena<BasicBlock>,
    targets: &mut CallTargets,
    temps: &mut Arena<Temp>,
    safepoints: &mut PendingSafepointSites,
    blocks_used: (BlockId, BlockId, BlockId),
) -> Vec<AbiCallArgument> {
    let (entry, normal, unwind) = blocks_used;
    let RootArguments::Build(destination) = main.arguments else {
        return Vec::new();
    };
    let [AbiArgument::Direct(array)] = main.signature.arguments() else {
        unreachable!("an argv entry has one direct array reference")
    };
    let signature = targets.direct_signatures.alloc(DirectCallSignature::new(
        Vec::new(),
        array.clone(),
        CallingConvention::Cdecl,
    ));
    let target = targets.managed_targets.direct.alloc(CallTarget {
        destination,
        signature,
    });
    let out = temps.alloc(Temp { ty: MANAGED_PTR });
    blocks[entry].instructions.push(Instruction::Invoke {
        site: InvokeSite::Managed(ManagedInvokeSite {
            call: ManagedTypedCall::Direct {
                target,
                out,
                args: Vec::new(),
            },
            safepoint: safepoints.allocate(SafepointSiteRole::ManagedInvoke),
            roots: ExceptionalRootSet::default(),
            normal,
            unwind,
        }),
    });
    blocks[entry].terminator = Terminator::Br(normal);
    vec![AbiCallArgument::Direct(Value::Temp(out))]
}
