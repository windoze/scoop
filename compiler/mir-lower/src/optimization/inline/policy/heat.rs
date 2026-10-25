//! Static frequency belongs to a call site, not its target.

use super::{cold_blocks, graph, mir};
use crate::optimization::allocation::ControlFlow;
use mir::optimization_report::{InlineHeat, InlineHeatSource};

pub(in crate::optimization::inline) fn heats(body: &mir::Body) -> Vec<InlineHeat> {
    let cyclic = graph::cyclic_nodes(&graph::block_edges(body));
    let mut depths = if cyclic.iter().any(|&cycle| cycle) {
        ControlFlow::new(body).loop_depths()
    } else {
        vec![0; body.blocks.len()]
    };
    let cold = cold_blocks(&body.blocks.iter().collect::<Vec<_>>());
    for (depth, cyclic) in depths.iter_mut().zip(cyclic) {
        *depth = (*depth).max(usize::from(cyclic));
    }
    body.blocks
        .iter()
        .map(|(id, _)| {
            let depth = depths[graph::index(id)];
            let (source, numerator, denominator) = if cold.contains(&id) {
                (InlineHeatSource::ColdExit, 1, 8)
            } else if depth != 0 {
                (InlineHeatSource::Loop, [1, 4, 16, 32][depth.min(3)], 1)
            } else {
                (InlineHeatSource::Ordinary, 1, 1)
            };
            InlineHeat {
                source,
                loop_depth: depth,
                numerator,
                denominator,
            }
        })
        .collect()
}
