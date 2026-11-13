//! Logical observations carried through MIR transformations.

use std::collections::{BTreeMap, BTreeSet};

use scoop_profile::{
    BodyLayout, Count, Counter, FeedbackConsumer, Frequency, Instrumentation, Location,
    MissingCount, PositionKind, UseSummary,
};

/// A compile-local reference into the complete profile layout table.
/// This index is never a persistent profile key.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProfilePlanId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProfileSite {
    pub plan: ProfilePlanId,
    pub position: u32,
}

#[derive(Clone, Debug)]
pub struct ProfileMarker {
    pub site: ProfileSite,
    pub kind: PositionKind,
    /// Outermost call first; independent of debug-info availability.
    pub inlined_at: Vec<ProfileInlineOrigin>,
}

#[derive(Clone, Debug)]
pub enum ProfileInlineOrigin {
    Call(ProfileSite),
    Generated(u32),
}

impl ProfileMarker {
    pub fn resolve(&self, profiling: &MirProfiling) -> Counter {
        Counter {
            location: profiling.location(self.site),
            inlined_at: self
                .inlined_at
                .iter()
                .map(|site| match site {
                    ProfileInlineOrigin::Call(site) => {
                        scoop_profile::InlineOrigin::Call(profiling.location(*site))
                    }
                    ProfileInlineOrigin::Generated(copy) => {
                        scoop_profile::InlineOrigin::Generated(*copy)
                    }
                })
                .collect(),
        }
    }

    pub fn inline_at(&mut self, caller: Option<&Self>, copy: u32) {
        let mut path = match caller {
            Some(caller) => {
                let mut path = caller.inlined_at.clone();
                path.push(ProfileInlineOrigin::Call(caller.site));
                path
            }
            None => vec![ProfileInlineOrigin::Generated(copy)],
        };
        path.append(&mut self.inlined_at);
        self.inlined_at = path;
    }
}

#[derive(Debug, Default)]
pub struct MirProfiling {
    pub instrumentation: Instrumentation,
    pub feedback: bool,
    pub plans: Vec<BodyLayout>,
    pub counts: BTreeMap<ProfileSite, Count>,
    pub frequencies: BTreeMap<ProfileSite, Result<Frequency, MissingCount>>,
    pub used: BTreeMap<FeedbackConsumer, BTreeSet<Location>>,
}

impl MirProfiling {
    pub fn use_summary(&self) -> Option<UseSummary> {
        if !self.feedback {
            return None;
        }
        let mut summary = UseSummary::default();
        let mut functions = BTreeSet::new();
        let mut positions = BTreeSet::new();
        for (&site, &count) in &self.counts {
            let location = self.location(site);
            if !positions.insert(location) {
                continue;
            }
            match count {
                Count::Observed { partial, .. } => {
                    functions.insert(location.body);
                    summary.matched_positions += 1;
                    summary.partial_positions += usize::from(partial);
                }
                Count::Missing(MissingCount::BodyChanged) => summary.changed_positions += 1,
                Count::Missing(_) => summary.missing_positions += 1,
            }
        }
        summary.matched_functions = functions.len();
        summary.consumers = self
            .used
            .iter()
            .map(|(&kind, sites)| (kind, sites.len()))
            .collect();
        Some(summary)
    }

    pub fn location(&self, site: ProfileSite) -> Location {
        Location {
            body: self.plans[site.plan.0 as usize].key,
            position: site.position,
        }
    }
}

/// Calls are normalized effect statements. A retained original call has its
/// own marker immediately before it, including after block moves and clones.
pub fn profile_call_before(block: &crate::BasicBlock, statement: usize) -> Option<&ProfileMarker> {
    match &block.statements.get(statement.checked_sub(1)?)?.kind {
        crate::StatementKind::ProfileCounter(marker) if marker.kind == PositionKind::Call => {
            Some(marker)
        }
        _ => None,
    }
}
