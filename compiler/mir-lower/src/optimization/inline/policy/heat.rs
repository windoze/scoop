//! Frequency belongs to a logical location, not its target.

use super::{cold_blocks, graph, mir};
use crate::optimization::allocation::ControlFlow;
use mir::optimization_report::{InlineHeat, InlineHeatSource, ProfileHeat};
use scoop_profile::MissingCount;

pub(in crate::optimization) fn heats(body: &mir::Body) -> Vec<InlineHeat> {
    let cyclic = graph::cyclic_nodes(&graph::block_edges(body));
    let nesting = if cyclic.iter().any(|&cycle| cycle) {
        ControlFlow::new(body).loop_nesting()
    } else {
        vec![Default::default(); body.blocks.len()]
    };
    let cold = cold_blocks(&body.blocks.iter().collect::<Vec<_>>());
    body.blocks
        .iter()
        .map(|(id, _)| {
            let loop_nesting = nesting[graph::index(id)];
            let depth = loop_nesting
                .depth
                .max(usize::from(cyclic[graph::index(id)]));
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
                loop_header: loop_nesting.header,
                numerator,
                denominator,
                profile: None,
            }
        })
        .collect()
}

pub(in crate::optimization) fn with_profile(
    mut heat: InlineHeat,
    marker: Option<&mir::ProfileMarker>,
    profiling: &mir::MirProfiling,
) -> InlineHeat {
    if !profiling.feedback {
        return heat;
    }
    let frequency = marker
        .and_then(|marker| profiling.frequencies.get(&marker.site).copied())
        .unwrap_or(Err(MissingCount::NotEmitted));
    heat.profile = Some(ProfileHeat {
        location: marker.map(|marker| profiling.location(marker.site)),
        frequency,
    });
    if let Ok(frequency) = frequency {
        let (calls, entries) = (frequency.calls as u128, frequency.entries as u128);
        let (numerator, denominator) = if calls * 8 < entries {
            (1, 8)
        } else if calls > entries * 32 {
            (32, 1)
        } else {
            (frequency.calls, frequency.entries)
        };
        heat.source = InlineHeatSource::Profile;
        heat.numerator = numerator;
        heat.denominator = denominator;
    }
    heat
}

pub(in crate::optimization) fn compare(left: InlineHeat, right: InlineHeat) -> std::cmp::Ordering {
    (left.numerator as u128 * right.denominator as u128)
        .cmp(&(right.numerator as u128 * left.denominator as u128))
}
