use super::*;
use crate::cfg::CallSiteKind;

pub(super) fn positions(
    plan: mir::ProfilePlanId,
    function: &mut mir::Function,
    order: &[mir::BlockId],
    sites: &mut [CallSite],
) -> Vec<Position> {
    let mut positions = Vec::new();
    let entry = marker(plan, &mut positions, PositionKind::Entry);
    for &id in order {
        let original = std::mem::take(&mut function.body.blocks[id].statements);
        let mut output = Vec::new();
        let site_positions = sites
            .iter()
            .enumerate()
            .filter(|(_, site)| site.block == id)
            .map(|(index, site)| (index, site.statement))
            .collect::<Vec<_>>();
        let block_marker = marker(plan, &mut positions, PositionKind::Block);
        let landing_pad = original.first().is_some_and(|statement| {
            matches!(
                statement.kind,
                mir::StatementKind::Eh(mir::EhStatement::LandingPad { .. })
            )
        });
        if !landing_pad {
            output.push(block_marker.clone());
        }
        for boundary in 0..=original.len() {
            for &(index, _) in site_positions
                .iter()
                .filter(|(_, position)| *position == boundary)
            {
                if matches!(sites[index].kind, CallSiteKind::Suspend(_)) {
                    output.push(marker(plan, &mut positions, PositionKind::Call));
                    sites[index].statement = output.len();
                }
            }
            if let Some(statement) = original.get(boundary) {
                if matches!(statement.kind, mir::StatementKind::Call(_))
                    && !original.get(boundary.wrapping_sub(1)).is_some_and(|previous| matches!(&previous.kind, mir::StatementKind::ProfileCounter(marker) if marker.kind == PositionKind::Call))
                {
                    output.push(marker(plan, &mut positions, PositionKind::Call));
                }
                for &(index, _) in site_positions
                    .iter()
                    .filter(|(_, position)| *position == boundary)
                {
                    if matches!(sites[index].kind, CallSiteKind::Call) {
                        sites[index].statement = output.len();
                    }
                }
                output.push(statement.clone());
                if boundary == 0 && landing_pad {
                    output.push(block_marker.clone());
                }
            }
        }
        function.body.blocks[id].statements = output;
        let terminator = function.body.blocks[id].terminator.clone();
        if let mir::Terminator::Branch {
            cond,
            then_block,
            else_block,
        } = terminator
        {
            let then_block = edge(
                function,
                marker(plan, &mut positions, PositionKind::ThenEdge),
                then_block,
            );
            let else_block = edge(
                function,
                marker(plan, &mut positions, PositionKind::ElseEdge),
                else_block,
            );
            function.body.blocks[id].terminator = mir::Terminator::Branch {
                cond,
                then_block,
                else_block,
            };
        }
    }
    let original_entry = function.body.entry;
    function.body.entry = edge(function, entry, original_entry);
    positions
}

fn marker(
    plan: mir::ProfilePlanId,
    positions: &mut Vec<Position>,
    kind: PositionKind,
) -> mir::Statement {
    let position =
        u32::try_from(positions.len()).expect("logical positions fit the counter index width");
    positions.push(Position { kind });
    mir::Statement {
        debug_origin: Default::default(),
        span: mir::SourceSpan::new(0, 0).expect("generated marker has an empty span"),
        kind: mir::StatementKind::ProfileCounter(mir::ProfileMarker {
            site: mir::ProfileSite { plan, position },
            kind,
            inlined_at: Vec::new(),
        }),
    }
}

fn edge(
    function: &mut mir::Function,
    marker: mir::Statement,
    target: mir::BlockId,
) -> mir::BlockId {
    function.body.blocks.alloc(mir::BasicBlock {
        name: "profile".into(),
        terminator_origin: Default::default(),
        statements: vec![marker],
        terminator: mir::Terminator::Goto(target),
        unwind: None,
    })
}
