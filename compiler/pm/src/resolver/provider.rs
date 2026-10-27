use std::{cell::RefCell, cmp::Reverse, collections::BTreeSet};

use pubgrub::{
    Dependencies, DependencyProvider, PackageResolutionStatistics, PubGrubError, Ranges,
};

use super::*;
use crate::PmError;

pub(super) type Domain = Ranges<Choice>;
pub(super) type Pool = BTreeMap<Key, BTreeMap<Choice, Candidate>>;

struct Provider<'a, F> {
    pool: RefCell<Pool>,
    load: RefCell<F>,
    policy: &'a ResolvePolicy,
    root: Key,
    fallbacks: RefCell<BTreeMap<Key, Vec<Requirement>>>,
    loaded_fallbacks: RefCell<BTreeSet<(Key, String)>>,
}

pub(super) fn solve<F>(root: Candidate, policy: &ResolvePolicy, load: F) -> PmResult<Solution>
where
    F: FnMut(&Requirement) -> PmResult<Vec<Candidate>>,
{
    let key = root.key();
    let choice = root.choice();
    let provider = Provider {
        root: key.clone(),
        policy,
        load: RefCell::new(load),
        fallbacks: RefCell::new(BTreeMap::new()),
        loaded_fallbacks: RefCell::new(BTreeSet::new()),
        pool: RefCell::new(BTreeMap::from([(
            key.clone(),
            BTreeMap::from([(choice.clone(), root)]),
        )])),
    };
    let selected = match pubgrub::resolve(&provider, key.clone(), choice) {
        Ok(selected) => selected,
        Err(PubGrubError::NoSolution(tree)) => {
            return Err(report::failure(&key, &provider.pool.borrow(), tree));
        }
        Err(
            PubGrubError::ErrorChoosingVersion { source, .. }
            | PubGrubError::ErrorRetrievingDependencies { source, .. }
            | PubGrubError::ErrorInShouldCancel(source),
        ) => return Err(source),
    };
    let pool = provider.pool.into_inner();
    let packages = selected
        .into_iter()
        .map(|(key, choice)| {
            let candidate = pool[&key][&choice].clone();
            (key, candidate)
        })
        .collect();
    Ok(Solution {
        root: key,
        packages,
    })
}

impl<F> Provider<'_, F>
where
    F: FnMut(&Requirement) -> PmResult<Vec<Candidate>>,
{
    fn candidates(&self, dependency: &Requirement) -> PmResult<Domain> {
        let candidates = (self.load.borrow_mut())(dependency)?;
        let mut pool = self.pool.borrow_mut();
        let available = pool.entry(dependency.key.clone()).or_default();
        let mut range = Domain::empty();
        for candidate in candidates {
            if candidate.key() != dependency.key {
                return Err(PmError::new(format!(
                    "expected {}, source declares {}",
                    dependency.key, candidate.coordinate
                )));
            }
            let choice = candidate.choice();
            if candidate.kind == RequestedConeKind::Library
                && self.policy.permitted(&candidate)
                && dependency
                    .versions
                    .iter()
                    .all(|version| version.matches(&choice.version))
            {
                range = range.union(&Domain::singleton(choice.clone()));
            }
            available.entry(choice).or_insert(candidate);
        }
        Ok(range)
    }
}

impl<F> DependencyProvider for Provider<'_, F>
where
    F: FnMut(&Requirement) -> PmResult<Vec<Candidate>>,
{
    type P = Key;
    type V = Choice;
    type VS = Domain;
    type Priority = (bool, Reverse<usize>, Reverse<Key>);
    type M = String;
    type Err = PmError;

    fn prioritize(
        &self,
        key: &Key,
        range: &Domain,
        _: &PackageResolutionStatistics,
    ) -> Self::Priority {
        let count = self.pool.borrow().get(key).map_or(0, |versions| {
            versions.keys().filter(|v| range.contains(v)).count()
        });
        (
            !self.fallbacks.borrow().contains_key(key),
            Reverse(count),
            Reverse(key.clone()),
        )
    }

    fn choose_version(&self, key: &Key, range: &Domain) -> PmResult<Option<Choice>> {
        let eligible = |candidate: &Candidate| {
            range.contains(&candidate.choice())
                && (key == &self.root
                    || (candidate.kind == RequestedConeKind::Library
                        && self.policy.permitted(candidate)))
        };
        let has_candidate = self
            .pool
            .borrow()
            .get(key)
            .is_some_and(|choices| choices.values().any(eligible));
        if !has_candidate {
            let fallbacks = self
                .fallbacks
                .borrow()
                .get(key)
                .cloned()
                .unwrap_or_default();
            for fallback in fallbacks {
                if self
                    .loaded_fallbacks
                    .borrow_mut()
                    .insert((key.clone(), fallback.source.to_string()))
                {
                    self.candidates(&fallback)?;
                }
            }
        }
        let pool = self.pool.borrow();
        let Some(choices) = pool.get(key) else {
            return Ok(None);
        };
        let mut candidates: Vec<_> = choices
            .values()
            .filter(|candidate| eligible(candidate))
            .collect();
        candidates.sort_by(|a, b| {
            let prefer = |candidate: &Candidate| {
                !self.policy.updating(key) && self.policy.is_locked(candidate)
            };
            prefer(b)
                .cmp(&prefer(a))
                .then_with(|| a.choice().preference(&b.choice()))
        });
        Ok(candidates.first().map(|candidate| candidate.choice()))
    }

    fn get_dependencies(
        &self,
        key: &Key,
        choice: &Choice,
    ) -> PmResult<Dependencies<Key, Domain, String>> {
        let candidate = self.pool.borrow()[key][choice].clone();
        let mut dependencies = pubgrub::Map::default();
        for dependency in &candidate.dependencies {
            if dependency.key == *key {
                return Ok(Dependencies::Unavailable(format!(
                    "{} depends on itself",
                    candidate.coordinate
                )));
            }
            let range = match &dependency.source {
                SourceRequest::ImplicitCore | SourceRequest::ExactArtifact(_) => {
                    let version = match &dependency.source {
                        SourceRequest::ExactArtifact(coordinate) => coordinate.version(),
                        _ => "0.1.0",
                    };
                    self.fallbacks
                        .borrow_mut()
                        .entry(dependency.key.clone())
                        .or_default()
                        .push(dependency.clone());
                    Choice::any_source(version)
                }
                _ => self.candidates(dependency)?,
            };
            dependencies
                .entry(dependency.key.clone())
                .and_modify(|existing: &mut Domain| {
                    *existing = existing.intersection(&range);
                })
                .or_insert(range);
        }
        Ok(Dependencies::Available(dependencies.into_iter().collect()))
    }

    fn should_cancel(&self) -> PmResult<()> {
        scoop_process::check_interrupted().map_err(|error| PmError::new(error.to_string()))
    }
}
