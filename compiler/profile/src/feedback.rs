use std::collections::{BTreeMap, BTreeSet};

use crate::{
    BodyId, BodyKey, BodyLayout, IndexedRecord, Location, PositionKind, ProfileMap, SCHEMA,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingCount {
    MissingBody,
    BodyChanged,
    NotEmitted,
    MissingRecord,
    ZeroEntry,
    InconsistentFlow,
    Overflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Count {
    Observed { value: u64, partial: bool },
    Missing(MissingCount),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Frequency {
    pub calls: u64,
    pub entries: u64,
    pub partial: bool,
}

#[derive(Debug, Default)]
pub struct Feedback {
    bodies: BTreeMap<BodyKey, BodyFacts>,
    body_ids: BTreeSet<BodyId>,
}

#[derive(Debug)]
struct BodyFacts {
    counts: Vec<Count>,
    frequencies: BTreeMap<u32, Result<Frequency, MissingCount>>,
}

impl Feedback {
    /// Decode the Scoop map boundary once. Duplicate ODR maps do not count an
    /// indexed record twice; conflicting layouts are malformed input.
    pub fn from_records(
        target: &str,
        maps: &[ProfileMap],
        records: &[IndexedRecord],
    ) -> Result<Self, String> {
        let mut layouts = BTreeMap::<BodyKey, &BodyLayout>::new();
        let mut physical = BTreeMap::new();
        for map in maps {
            if map.schema != SCHEMA || map.target != target {
                return Err("profile map schema or target does not match".into());
            }
            for body in &map.bodies {
                if body.positions.is_empty()
                    || body.positions[0].kind != PositionKind::Entry
                    || body.positions[1..]
                        .iter()
                        .any(|position| position.kind == PositionKind::Entry)
                {
                    return Err("profile body must have exactly one entry at position zero".into());
                }
                if let Some(previous) = layouts.insert(body.key, body)
                    && previous.positions != body.positions
                {
                    return Err("conflicting logical profile layouts".into());
                }
            }
            for record in &map.records {
                if record.counters.is_empty() {
                    return Err("empty physical profile record".into());
                }
                if let Some(previous) = physical.insert((&record.name, record.hash), record)
                    && previous.counters != record.counters
                {
                    return Err("conflicting physical profile layouts".into());
                }
            }
        }
        let location_kind = |location: Location| {
            layouts
                .get(&location.body)
                .and_then(|body| body.positions.get(location.position as usize))
                .map(|position| position.kind)
                .ok_or_else(|| "profile counter references an unknown logical position".to_string())
        };
        let mut indexed = BTreeMap::new();
        for record in records {
            if indexed
                .insert((record.name.as_slice(), record.hash), &record.counts)
                .is_some()
            {
                return Err("duplicate indexed profile record".into());
            }
        }
        let mut result = Self {
            body_ids: layouts.keys().map(|key| key.body).collect(),
            bodies: layouts
                .iter()
                .map(|(&key, layout)| {
                    (
                        key,
                        BodyFacts {
                            counts: vec![
                                Count::Missing(MissingCount::NotEmitted);
                                layout.positions.len()
                            ],
                            frequencies: BTreeMap::new(),
                        },
                    )
                })
                .collect(),
        };
        let mut scoped_bodies = BTreeSet::new();
        for ((name, hash), record) in physical {
            let counts = indexed.get(&(name.as_bytes(), hash)).copied();
            if counts.is_some_and(|counts| counts.len() != record.counters.len()) {
                return Err("indexed counter count does not match the physical layout".into());
            }
            // Pair entries and calls inside the same actual inline copy in
            // this physical record before adding any counts across records.
            let mut scopes = BTreeMap::<_, BTreeMap<u32, Count>>::new();
            for (index, counter) in record.counters.iter().enumerate() {
                location_kind(counter.location)?;
                for site in &counter.inlined_at {
                    if let crate::InlineOrigin::Call(site) = site
                        && location_kind(*site)? != PositionKind::Call
                    {
                        return Err("profile inline origin is not a call position".into());
                    }
                }
                let count = counts.map_or(Count::Missing(MissingCount::MissingRecord), |counts| {
                    Count::Observed {
                        value: counts[index],
                        partial: false,
                    }
                });
                let scope = scopes
                    .entry((counter.location.body, &counter.inlined_at))
                    .or_default();
                if scope.insert(counter.location.position, count).is_some() {
                    return Err("duplicate logical counter in one physical scope".into());
                }
            }
            for ((body, _), scope) in scopes {
                let first_scope = scoped_bodies.insert(body);
                let entry = scope
                    .get(&0)
                    .copied()
                    .unwrap_or(Count::Missing(MissingCount::NotEmitted));
                for (position, definition) in layouts[&body].positions.iter().enumerate() {
                    let position = u32::try_from(position)
                        .map_err(|_| "too many logical profile positions")?;
                    let count = scope
                        .get(&position)
                        .copied()
                        .unwrap_or(Count::Missing(MissingCount::NotEmitted));
                    let total =
                        &mut result.bodies.get_mut(&body).unwrap().counts[position as usize];
                    *total = if first_scope {
                        count
                    } else {
                        add_count(*total, count)
                    };
                    if !matches!(definition.kind, PositionKind::Call | PositionKind::Block) {
                        continue;
                    }
                    let frequency = match (count, entry) {
                        (
                            Count::Observed { value: calls, .. },
                            Count::Observed { value: entries, .. },
                        ) => {
                            if entries == 0 && calls != 0 {
                                Err(MissingCount::InconsistentFlow)
                            } else {
                                Ok(Frequency {
                                    calls,
                                    entries,
                                    partial: false,
                                })
                            }
                        }
                        (Count::Missing(reason), _) | (_, Count::Missing(reason)) => Err(reason),
                    };
                    let frequencies = &mut result.bodies.get_mut(&body).unwrap().frequencies;
                    match frequencies.entry(position) {
                        std::collections::btree_map::Entry::Vacant(slot) => {
                            slot.insert(frequency);
                        }
                        std::collections::btree_map::Entry::Occupied(mut slot) => {
                            *slot.get_mut() = add_frequency(*slot.get(), frequency);
                        }
                    }
                }
            }
        }
        Ok(result)
    }

    pub fn count(&self, location: Location) -> Count {
        match self.bodies.get(&location.body) {
            Some(body) => body
                .counts
                .get(location.position as usize)
                .copied()
                .unwrap_or(Count::Missing(MissingCount::NotEmitted)),
            None => Count::Missing(self.missing_body(location.body.body)),
        }
    }

    pub fn frequency(&self, location: Location) -> Result<Frequency, MissingCount> {
        let Some(body) = self.bodies.get(&location.body) else {
            return Err(self.missing_body(location.body.body));
        };
        let frequency = body
            .frequencies
            .get(&location.position)
            .copied()
            .unwrap_or(Err(MissingCount::NotEmitted))?;
        if frequency.entries == 0 {
            return Err(MissingCount::ZeroEntry);
        }
        Ok(frequency)
    }

    fn missing_body(&self, body: BodyId) -> MissingCount {
        if self.body_ids.contains(&body) {
            MissingCount::BodyChanged
        } else {
            MissingCount::MissingBody
        }
    }
}

fn add_frequency(
    left: Result<Frequency, MissingCount>,
    right: Result<Frequency, MissingCount>,
) -> Result<Frequency, MissingCount> {
    let (left, right) = match (left, right) {
        (Err(reason @ (MissingCount::Overflow | MissingCount::InconsistentFlow)), _)
        | (_, Err(reason @ (MissingCount::Overflow | MissingCount::InconsistentFlow))) => {
            return Err(reason);
        }
        (Ok(mut frequency), Err(_)) | (Err(_), Ok(mut frequency)) => {
            frequency.partial = true;
            return Ok(frequency);
        }
        (Err(reason), Err(_)) => return Err(reason),
        (Ok(left), Ok(right)) => (left, right),
    };
    Ok(Frequency {
        calls: left
            .calls
            .checked_add(right.calls)
            .ok_or(MissingCount::Overflow)?,
        entries: left
            .entries
            .checked_add(right.entries)
            .ok_or(MissingCount::Overflow)?,
        partial: left.partial || right.partial,
    })
}

fn add_count(left: Count, right: Count) -> Count {
    match (left, right) {
        (Count::Missing(MissingCount::Overflow), _)
        | (_, Count::Missing(MissingCount::Overflow)) => Count::Missing(MissingCount::Overflow),
        (
            Count::Observed {
                value: left,
                partial: a,
            },
            Count::Observed {
                value: right,
                partial: b,
            },
        ) => left
            .checked_add(right)
            .map(|value| Count::Observed {
                value,
                partial: a || b,
            })
            .unwrap_or(Count::Missing(MissingCount::Overflow)),
        (Count::Observed { value, .. }, Count::Missing(_))
        | (Count::Missing(_), Count::Observed { value, .. }) => Count::Observed {
            value,
            partial: true,
        },
        (Count::Missing(reason), _) => Count::Missing(reason),
    }
}
