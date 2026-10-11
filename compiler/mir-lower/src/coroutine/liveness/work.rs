use super::*;
use crate::cfg::{CallSite, CallSiteKind, CleanupContext, SuspendCall};

pub(super) struct WorkingCfg<'a> {
    pub(super) blocks: Vec<Vec<WorkingStatement<'a>>>,
    pub(super) calls: HashMap<(mir::BlockId, usize), (usize, &'a CleanupContext)>,
}

pub(super) enum WorkingStatement<'a> {
    Mir {
        position: usize,
        statement: &'a mir::Statement,
    },
    Suspend {
        position: usize,
        order: usize,
        call: &'a SuspendCall,
        pending: &'a CleanupContext,
    },
}

pub(super) fn collect<'a>(body: &'a mir::Body, sites: &'a [CallSite]) -> WorkingCfg<'a> {
    let mut pending = vec![Vec::new(); body.blocks.len()];
    let mut calls = HashMap::new();
    for (order, site) in sites.iter().enumerate() {
        match &site.kind {
            CallSiteKind::Call => {
                calls.insert((site.block, site.statement), (order, &site.pending));
            }
            CallSiteKind::Suspend(call) => {
                pending[raw(site.block)].push((site.statement, order, call, &site.pending));
            }
        }
    }
    let blocks = body
        .blocks
        .iter()
        .zip(pending)
        .map(|((_, block), pending)| {
            let mut pending = pending.into_iter().peekable();
            let mut statements = Vec::with_capacity(block.statements.len() + pending.len());
            for (position, statement) in block.statements.iter().enumerate() {
                while let Some((_, order, call, context)) =
                    pending.next_if(|item| item.0 == position)
                {
                    statements.push(WorkingStatement::Suspend {
                        position,
                        order,
                        call,
                        pending: context,
                    });
                }
                statements.push(WorkingStatement::Mir {
                    position,
                    statement,
                });
            }
            statements.extend(pending.map(|(position, order, call, pending)| {
                WorkingStatement::Suspend {
                    position,
                    order,
                    call,
                    pending,
                }
            }));
            statements
        })
        .collect();
    WorkingCfg { blocks, calls }
}
