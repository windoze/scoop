use super::{CanonicalReexportRoutesV1, ReexportRouteHopV1};

impl CanonicalReexportRoutesV1 {
    pub(crate) fn contains_exact_suffix(&self, suffix: &[ReexportRouteHopV1]) -> bool {
        let Some(first) = suffix.first() else {
            return false;
        };
        self.routes
            .iter()
            .any(|route| route.immediate_provider() == first.exporter() && route.hops() == suffix)
    }
}
