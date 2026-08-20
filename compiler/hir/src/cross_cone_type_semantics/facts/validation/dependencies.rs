use super::{CheckedExactTypeFactsV1, ExactTypeFactsV1};
use scoop_identity::PersistentExactTypeId;

/// A single fact obtained from an already validated table. It cannot be
/// created from a transported fact record alone.
#[derive(Clone, Copy, Debug)]
pub struct CheckedExactTypeFactV1<'a> {
    fact: &'a ExactTypeFactsV1,
}
impl<'a> CheckedExactTypeFactV1<'a> {
    pub const fn record(self) -> &'a ExactTypeFactsV1 {
        self.fact
    }
}
impl<'a> CheckedExactTypeFactsV1<'a> {
    pub fn get_checked(self, exact: PersistentExactTypeId) -> Option<CheckedExactTypeFactV1<'a>> {
        self.facts
            .get(exact)
            .map(|fact| CheckedExactTypeFactV1 { fact })
    }
}

/// The complete section supplies terminal checked provider facts. This view
/// deliberately has no fallback to raw facts or native-boundary witnesses.
pub trait ExactTypeFactsDependencyLookupV1 {
    fn get_dependency_fact(
        &self,
        exact: PersistentExactTypeId,
    ) -> Option<CheckedExactTypeFactV1<'_>>;
}
pub(super) struct NoDependencies;
impl ExactTypeFactsDependencyLookupV1 for NoDependencies {
    fn get_dependency_fact(&self, _: PersistentExactTypeId) -> Option<CheckedExactTypeFactV1<'_>> {
        None
    }
}

#[cfg(test)]
mod tests;
