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
    let RootArguments::Build(destination, signature) = main.arguments else {
        return Vec::new();
    };
    let AbiReturn::Direct(array) = signature.result() else {
        unreachable!("the argument builder returns an array reference")
    };
    let signature = targets.direct_signatures.alloc(DirectCallSignature::new(
        signature.arguments().to_vec(),
        array.clone(),
        signature.calling_convention(),
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
