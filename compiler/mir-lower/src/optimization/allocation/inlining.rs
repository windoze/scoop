//! Reuse allocation proofs when one remaining receiver call blocks replacement.

use std::collections::HashSet;

use super::{index, mir, origins::AllocationKind, uses::Plan};

pub(in crate::optimization) struct InlineReceivers(Plan);

pub(in crate::optimization) struct Receiver {
    pub class: mir::ClassId,
    pub allocation: (mir::BlockId, usize),
    pub aliases: HashSet<mir::LocalId>,
}

impl InlineReceivers {
    pub fn new(module: &mir::Module, function: &mir::Function) -> Option<Self> {
        super::supported(function).then(|| Self(Plan::new(module, function)))
    }

    pub fn receiver(&self, operand: &mir::Expr) -> Option<Receiver> {
        let candidate = self.0.sources.locals[index(super::local_alias(operand)?)].unique()?;
        if !self.0.single_use[index(candidate)] {
            return None;
        }
        let allocation = &self.0.sources.allocations[candidate];
        let AllocationKind::Class(class) = allocation.kind else {
            return None;
        };
        Some(Receiver {
            class,
            allocation: (allocation.site.block, allocation.site.statement),
            aliases: self
                .0
                .sources
                .locals
                .iter()
                .enumerate()
                .filter(|(_, origins)| origins.unique() == Some(candidate))
                .map(|(id, _)| mir::LocalId::from_raw(la_arena::RawIdx::from_u32(id as u32)))
                .collect(),
        })
    }
}
