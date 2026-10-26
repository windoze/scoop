//! Reusable facts about a stable template; no call-site decisions are cached.

use std::collections::{HashMap, HashSet};

use super::{exception_paths, flow, mir, policy, simplify};

pub(super) struct Summary {
    pub original: policy::Cost,
    pub baseline: policy::Cost,
    pub blocks: Vec<(mir::BlockId, mir::BasicBlock)>,
    pub parameter_copies: usize,
    pub return_copy: usize,
    pub indirect_calls: bool,
    pub array_access: bool,
    parameters_read: Vec<bool>,
    block_sizes: HashMap<mir::BlockId, usize>,
    cold: HashSet<mir::BlockId>,
}

impl Summary {
    pub fn new(
        module: &mir::Module,
        function: &mir::Function,
        effects: &exception_paths::Effects<'_>,
    ) -> Self {
        let blocks = simplify::blocks(module, function, &[]);
        let blocks = effects.resolve_calls(function, &[], blocks);
        Self::from_blocks(&module.structs, function, blocks)
    }

    pub fn from_blocks(
        structs: &la_arena::Arena<mir::StructDef>,
        function: &mir::Function,
        blocks: Vec<(mir::BlockId, mir::BasicBlock)>,
    ) -> Self {
        let mut reads = HashSet::new();
        let mut indirect_calls = false;
        let mut array_access = false;
        for (_, block) in &blocks {
            for statement in &block.statements {
                if let mir::StatementKind::Call(effect) = &statement.kind {
                    indirect_calls |= !matches!(
                        crate::optimization::call(effect).target,
                        mir::CallTarget::Direct(_)
                    );
                }
                array_access |= matches!(statement.kind, mir::StatementKind::ArraySet { .. });
            }
            mir::visit_block_exprs(block, &mut |expr| {
                if let mir::ExprKind::Local(local) | mir::ExprKind::AddressOf { local, .. } =
                    expr.kind
                {
                    reads.insert(local);
                }
                array_access |= matches!(expr.kind, mir::ExprKind::ArrayGet { .. });
            });
        }
        let refs = blocks
            .iter()
            .map(|(id, block)| (*id, block))
            .collect::<Vec<_>>();
        let baseline =
            policy::blocks_cost(structs, refs.iter().copied()).with_parameters(structs, function);
        Self {
            original: policy::function_cost(structs, function),
            baseline,
            cold: policy::cold_blocks(&refs),
            block_sizes: refs
                .iter()
                .map(|(id, block)| (*id, policy::block_cost(structs, block)))
                .collect(),
            blocks,
            parameter_copies: function
                .params
                .iter()
                .map(|p| policy::copy_cost(structs, &p.ty))
                .sum(),
            return_copy: policy::copy_cost(structs, &function.return_ty),
            parameters_read: function
                .params
                .iter()
                .map(|p| reads.contains(&p.local))
                .collect(),
            indirect_calls,
            array_access,
        }
    }

    pub fn argument_sensitive(&self, arguments: &[flow::ValueFacts]) -> bool {
        self.parameters_read
            .iter()
            .zip(arguments)
            .any(|(&read, facts)| {
                read && (facts.constant.is_some() || facts.actual != flow::ActualType::Unknown)
            })
    }

    pub fn savings(
        &self,
        structs: &la_arena::Arena<mir::StructDef>,
        blocks: &[(mir::BlockId, mir::BasicBlock)],
    ) -> usize {
        // An untaken branch contributes no executed work. Only surviving
        // blocks may earn a simplification credit; all blocks affect size.
        blocks
            .iter()
            .map(|(id, block)| {
                let saved = self
                    .block_sizes
                    .get(id)
                    .copied()
                    .unwrap_or(0)
                    .saturating_sub(policy::block_cost(structs, block));
                if self.cold.contains(id) {
                    saved / 4
                } else {
                    saved
                }
            })
            .sum()
    }
}

pub(super) fn complexity(blocks: &[(mir::BlockId, mir::BasicBlock)]) -> usize {
    blocks
        .iter()
        .map(|(_, block)| {
            usize::from(matches!(block.terminator, mir::Terminator::Branch { .. }))
                + 2 * usize::from(block.unwind.is_some())
        })
        .sum()
}
