use std::collections::{BTreeMap, HashSet};

use super::*;

pub(super) fn assign_safepoint_identities(
    function: &mut lir::Function,
    pending: &PendingSafepointSites,
) {
    let block_order = canonical_reverse_postorder(function);
    let owner = function.callable_body.id();
    let mut ordinals = BTreeMap::<lir::SafepointSiteRole, usize>::new();
    let mut consumed = HashSet::new();
    let mut identities = Vec::new();

    for block_id in block_order {
        for instruction in &function.blocks[block_id].instructions {
            let Some((expected_role, site)) = instruction.safepoint() else {
                continue;
            };
            let provisional = site;
            assert_eq!(
                pending.role(provisional),
                expected_role,
                "a provisional safepoint role must match its LIR instruction"
            );
            assert!(
                consumed.insert(provisional),
                "a provisional safepoint site cannot belong to multiple instructions"
            );
            let next_ordinal = ordinals.entry(expected_role).or_default();
            let ordinal = u32::try_from(*next_ordinal)
                .expect("one callable cannot contain more than u32::MAX sites of one role");
            *next_ordinal = next_ordinal
                .checked_add(1)
                .expect("a function cannot contain usize::MAX instructions");
            let identity = lir::SafepointIdentity::new(owner, expected_role, ordinal)
                .expect("a canonical safepoint key derives one nonzero runtime identity");
            identities.push((provisional, identity));
        }
    }

    function.safepoints = lir::SafepointIdentities::checked(identities)
        .expect("canonical safepoint sites and runtime ids are unique within one function");
}

fn canonical_reverse_postorder(function: &lir::Function) -> Vec<lir::BlockId> {
    let mut visited = vec![false; function.blocks.len()];
    let mut postorder = Vec::with_capacity(function.blocks.len());
    let mut stack = vec![(function.entry, false)];
    while let Some((block, expanded)) = stack.pop() {
        let index = arena_index(block);
        if expanded {
            postorder.push(block);
            continue;
        }
        if visited[index] {
            continue;
        }
        visited[index] = true;
        stack.push((block, true));
        let successors = semantic_successors(&function.blocks[block]);
        stack.extend(
            successors
                .into_iter()
                .rev()
                .map(|successor| (successor, false)),
        );
    }
    assert!(
        visited.into_iter().all(|reachable| reachable),
        "unreachable LIR blocks must be pruned before safepoint identity assignment"
    );
    postorder.reverse();
    postorder
}

fn semantic_successors(block: &lir::BasicBlock) -> Vec<lir::BlockId> {
    assert!(
        !block.instructions[..block.instructions.len().saturating_sub(1)]
            .iter()
            .any(|instruction| matches!(instruction, lir::Instruction::Invoke { .. })),
        "an LIR invoke must be the final instruction of its block"
    );
    if let Some(lir::Instruction::Invoke { site }) = block.instructions.last() {
        assert!(
            matches!(block.terminator, lir::Terminator::Br(target) if target == site.normal()),
            "an LIR invoke block must end in the redundant branch to its normal successor"
        );
        return vec![site.normal(), site.unwind()];
    }
    match block.terminator {
        lir::Terminator::Br(target) => vec![target],
        lir::Terminator::CondBr {
            then_block,
            else_block,
            ..
        } => vec![then_block, else_block],
        lir::Terminator::Return { .. }
        | lir::Terminator::Resume { .. }
        | lir::Terminator::Unreachable => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use la_arena::RawIdx;

    use super::*;

    fn callable_body() -> lir::CallableBodyIdentity {
        let site = scoop_identity::SourceDeclarationSite::new(
            scoop_identity::ConeIdentity::SINGLE_FILE,
            scoop_identity::PackagePath::root(),
            scoop_identity::DefinitionOwnerChain::top_level(),
            scoop_identity::DeclarationScope::ConeWide,
        )
        .unwrap();
        let declaration = scoop_identity::SourceDeclarationKey::function(
            site,
            scoop_identity::CanonicalIdentifier::new("safepointIdentityTest").unwrap(),
            0,
            None,
            Vec::new(),
        );
        let function =
            scoop_identity::PersistentFunctionId::from_source_declaration(&declaration).unwrap();
        lir::CallableBodyIdentity::for_function(function).unwrap()
    }

    fn placeholder(name: &str) -> lir::BasicBlock {
        lir::BasicBlock {
            name: name.to_string(),
            instructions: Vec::new(),
            terminator: lir::Terminator::Unreachable,
        }
    }

    fn identity_fixture(
        reverse_arena_order: bool,
    ) -> Vec<(
        &'static str,
        lir::PersistentSafepointSiteId,
        lir::SafepointId,
        lir::SafepointSiteRole,
        u32,
    )> {
        let mut blocks = Arena::new();
        let (entry, then_block, else_block) = if reverse_arena_order {
            let else_block = blocks.alloc(placeholder("else"));
            let then_block = blocks.alloc(placeholder("then"));
            let entry = blocks.alloc(placeholder("entry"));
            (entry, then_block, else_block)
        } else {
            let entry = blocks.alloc(placeholder("entry"));
            let then_block = blocks.alloc(placeholder("then"));
            let else_block = blocks.alloc(placeholder("else"));
            (entry, then_block, else_block)
        };

        let mut pending = PendingSafepointSites::default();
        let (entry_site, then_site, else_site) = if reverse_arena_order {
            let then_site = pending.allocate(lir::SafepointSiteRole::NativeSafeTransition);
            let else_site = pending.allocate(lir::SafepointSiteRole::NativeSafeTransition);
            let entry_site = pending.allocate(lir::SafepointSiteRole::ManagedCall);
            (entry_site, then_site, else_site)
        } else {
            let entry_site = pending.allocate(lir::SafepointSiteRole::ManagedCall);
            let then_site = pending.allocate(lir::SafepointSiteRole::NativeSafeTransition);
            let else_site = pending.allocate(lir::SafepointSiteRole::NativeSafeTransition);
            (entry_site, then_site, else_site)
        };

        let mut temps = Arena::new();
        let array = temps.alloc(lir::Temp {
            ty: lir::MANAGED_PTR,
        });
        let then_address = temps.alloc(lir::Temp { ty: lir::RAW_PTR });
        let else_address = temps.alloc(lir::Temp { ty: lir::RAW_PTR });
        let array_type = lir::ArrayTypeId::from_raw(RawIdx::from_u32(0));
        let native_global = lir::NativeGlobalId::from_raw(RawIdx::from_u32(0));
        blocks[entry] = lir::BasicBlock {
            name: "entry".to_string(),
            instructions: vec![lir::Instruction::ArrayAlloc {
                out: array,
                elements: Vec::new(),
                array_type,
                safepoint: entry_site,
                live: lir::StatepointLiveSet::default(),
            }],
            terminator: lir::Terminator::CondBr {
                cond: lir::Value::BoolConst(true),
                then_block,
                else_block,
            },
        };
        blocks[then_block] = lir::BasicBlock {
            name: "then".to_string(),
            instructions: vec![lir::Instruction::NativeGlobalAddress {
                out: then_address,
                global: native_global,
                safepoint: then_site,
                roots: lir::NativeSafeRootSet::default(),
            }],
            terminator: lir::Terminator::Return { value: None },
        };
        blocks[else_block] = lir::BasicBlock {
            name: "else".to_string(),
            instructions: vec![lir::Instruction::NativeGlobalAddress {
                out: else_address,
                global: native_global,
                safepoint: else_site,
                roots: lir::NativeSafeRootSet::default(),
            }],
            terminator: lir::Terminator::Return { value: None },
        };

        let mut function = lir::Function {
            callable_body: callable_body(),
            gc_effect: lir::GcEffect::Managed,
            signature: lir::ScoopAbiSignature::new(
                Vec::new(),
                lir::AbiReturn::UnitVoid,
                lir::CallingConvention::Cdecl,
            ),
            call_targets: lir::CallTargets::default(),
            safepoints: lir::SafepointIdentities::default(),
            locals: Arena::new(),
            temps,
            blocks,
            entry,
        };
        assign_safepoint_identities(&mut function, &pending);

        [
            ("entry", entry_site),
            ("then", then_site),
            ("else", else_site),
        ]
        .into_iter()
        .map(|(name, reference)| {
            let identity = &function.safepoints[reference];
            (
                name,
                identity.site_id(),
                identity.runtime_id(),
                identity.role(),
                identity.ordinal(),
            )
        })
        .collect()
    }

    #[test]
    fn final_cfg_order_not_arena_or_provisional_order_defines_site_identity() {
        let forward = identity_fixture(false);
        let reversed = identity_fixture(true);

        assert_eq!(forward, reversed);
        assert_eq!(forward[0].3, lir::SafepointSiteRole::ManagedCall);
        assert_eq!(forward[0].4, 0);
        assert_eq!(forward[1].3, lir::SafepointSiteRole::NativeSafeTransition);
        assert_eq!(forward[1].4, 1);
        assert_eq!(forward[2].3, lir::SafepointSiteRole::NativeSafeTransition);
        assert_eq!(forward[2].4, 0);
    }
}
