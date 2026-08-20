use super::*;

type Edge = (
    PersistentInitializationUnitId,
    ConeIdentity,
    PersistentInitializationUnitId,
);

impl CanonicalMirExternalInitializationUsesV1 {
    /// Compare the complete registration edge inventory with actual MIR uses.
    /// Distinct causes in one local unit may share a dependency registration.
    pub fn validate_registration_edges(
        &self,
        actual: impl IntoIterator<Item = Edge>,
    ) -> Result<(), MirObjectBridgeError> {
        let path = WirePath::root();
        let mut edges = Vec::new();
        for edge in actual {
            scoop_wire::allocation::try_reserve(&mut edges, 1, &path)?;
            edges.push(edge);
        }

        edges.sort_unstable();
        if edges.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(MirObjectBridgeError::DuplicateInitializationDependency);
        }

        let mut actual = edges.into_iter();
        let mut previous = None;
        // Canonical uses order their unit/provider/dependency before the cause.
        for usage in self.records() {
            let edge = (
                usage.local_unit(),
                usage.provider(),
                usage.dependency_unit(),
            );
            if previous == Some(edge) {
                continue;
            }
            if actual.next() != Some(edge) {
                return Err(MirObjectBridgeError::InitializationDependencyInventory);
            }
            previous = Some(edge);
        }
        if actual.next().is_some() {
            return Err(MirObjectBridgeError::InitializationDependencyInventory);
        }
        Ok(())
    }
}
