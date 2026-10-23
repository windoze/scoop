//! Price an inline together with the local class projection that it enables.

use std::collections::HashSet;

use super::{mir, policy};
use crate::optimization::allocation::{
    InlineReceivers, defined_local, is_class_field, local_alias, statement_alias,
};

pub(super) fn cost(
    module: &mir::Module,
    caller: &mir::Function,
    call: &mir::Call,
    callee: &mir::Function,
    blocks: &[(mir::BlockId, mir::BasicBlock)],
    candidates: &InlineReceivers,
) -> Option<policy::Cost> {
    let parameter = callee.params.first()?;
    if !matches!(parameter.ty, mir::Type::Class(_)) {
        return None;
    }
    let receiver = candidates.receiver(call.args.first()?)?;
    let fields = Fields::parameter(module, receiver.class, callee, blocks)?;
    let callee_cost = fields
        .cost(blocks.iter().map(|(id, block)| (*id, block)), None)
        .with_parameters(&module.structs, callee);
    let before = policy::blocks_cost(&module.structs, caller.body.blocks.iter());
    let after = Fields {
        module,
        class: receiver.class,
        aliases: receiver.aliases,
    }
    .cost(caller.body.blocks.iter(), Some(receiver.allocation));
    let binding = policy::assignment_cost(&module.structs, &parameter.ty);
    // The receiver binding is a removable alias. Other parameter copies and
    // field initializers remain, including aggregate copies and side effects.
    Some(policy::Cost {
        execution: (callee_cost.execution + after.execution)
            .saturating_sub(before.execution + binding),
        size: (callee_cost.size + after.size).saturating_sub(before.size + binding),
    })
}

struct Fields<'a> {
    module: &'a mir::Module,
    class: mir::ClassId,
    aliases: HashSet<mir::LocalId>,
}

impl<'a> Fields<'a> {
    fn parameter(
        module: &'a mir::Module,
        class: mir::ClassId,
        callee: &mir::Function,
        blocks: &[(mir::BlockId, mir::BasicBlock)],
    ) -> Option<Self> {
        let receiver = callee.params.first()?.local;
        let mut fields = Self {
            module,
            class,
            aliases: HashSet::from([receiver]),
        };
        loop {
            let mut changed = false;
            for (_, block) in blocks {
                for statement in &block.statements {
                    if let Some(source) = statement_alias(statement)
                        && fields.aliases.contains(&source)
                        && let Some(destination) = defined_local(statement)
                    {
                        changed |= fields.aliases.insert(destination);
                    }
                }
            }
            if !changed {
                break;
            }
        }
        if callee
            .params
            .iter()
            .skip(1)
            .any(|p| fields.aliases.contains(&p.local))
        {
            return None;
        }
        let (mut reads, mut replaced, mut valid) = (0, 0, true);
        for (_, block) in blocks {
            if block.unwind.is_some()
                || matches!(
                    block.terminator,
                    mir::Terminator::Resume
                        | mir::Terminator::Rethrow { .. }
                        | mir::Terminator::Throw {
                            unwind: Some(_),
                            ..
                        }
                )
            {
                return None;
            }
            for statement in &block.statements {
                let alias =
                    statement_alias(statement).is_some_and(|local| fields.aliases.contains(&local));
                if defined_local(statement).is_some_and(|local| fields.aliases.contains(&local))
                    && !alias
                {
                    return None;
                }
                replaced += usize::from(alias);
                if let mir::StatementKind::FieldSet {
                    object,
                    index,
                    value,
                } = &statement.kind
                {
                    replaced += usize::from(fields.field(object, *index, &value.ty));
                }
                if matches!(statement.kind, mir::StatementKind::Eh(_)) {
                    return None;
                }
            }
            mir::visit_block_exprs(block, &mut |expr| match &expr.kind {
                mir::ExprKind::Local(local) => reads += usize::from(fields.aliases.contains(local)),
                mir::ExprKind::AddressOf { local, .. } => valid &= !fields.aliases.contains(local),
                mir::ExprKind::FieldAccess { receiver, index } => {
                    replaced += usize::from(fields.field(receiver, *index, &expr.ty));
                }
                _ => {}
            });
        }
        (valid && reads == replaced).then_some(fields)
    }

    fn owns(&self, object: &mir::Expr) -> bool {
        local_alias(object).is_some_and(|local| self.aliases.contains(&local))
    }

    fn field(&self, object: &mir::Expr, index: u32, ty: &mir::Type) -> bool {
        self.owns(object) && is_class_field(self.module, self.class, object, index, ty)
    }

    fn cost<'b>(
        &self,
        blocks: impl Iterator<Item = (mir::BlockId, &'b mir::BasicBlock)>,
        allocation: Option<(mir::BlockId, usize)>,
    ) -> policy::Cost {
        policy::adjusted_blocks_cost(&self.module.structs, blocks, |id, block, size| {
            let (mut removed, mut added) = (0, 0);
            for (index, statement) in block.statements.iter().enumerate() {
                if allocation == Some((id, index))
                    || statement_alias(statement).is_some_and(|local| self.aliases.contains(&local))
                {
                    removed += policy::statement_cost(&self.module.structs, statement);
                    mir::visit_statement_exprs(statement, &mut |expr| {
                        removed += policy::expression_cost(expr);
                    });
                } else if let mir::StatementKind::FieldSet {
                    object,
                    index,
                    value,
                } = &statement.kind
                    && self.field(object, *index, &value.ty)
                {
                    removed += policy::statement_cost(&self.module.structs, statement)
                        + policy::tree_cost(object);
                    added += policy::assignment_cost(&self.module.structs, &value.ty);
                }
            }
            mir::visit_block_exprs(block, &mut |expr| {
                if let mir::ExprKind::FieldAccess { receiver, index } = &expr.kind
                    && self.field(receiver, *index, &expr.ty)
                {
                    removed += policy::expression_cost(expr) + policy::tree_cost(receiver);
                }
            });
            size - removed + added
        })
    }
}
