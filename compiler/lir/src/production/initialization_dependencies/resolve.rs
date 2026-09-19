use scoop_identity::DecodedPersistentId;
use scoop_wire::{BudgetMeter, Encoder, WireEncode, WireError, WirePath};

use super::*;

/// Available checked definitions, not a substitute for an artifact closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongInitializationDefinitionCatalogV2 {
    producer: ConeIdentity,
    definitions: Vec<StrongInitializationUnitDefinitionRefV2>,
}

impl StrongInitializationDefinitionCatalogV2 {
    pub fn new(
        producer: ConeIdentity,
        definitions: &[StrongInitializationUnitDefinitionRefV2],
        meter: &mut BudgetMeter,
    ) -> Result<Self, InitializationDependencyResolutionError> {
        let path = WirePath::root();
        let mut canonical = Vec::new();
        meter.try_reserve_collection_slots(&mut canonical, definitions.len(), &path)?;
        meter.charge_nodes(definitions.len() as u64, &path)?;
        meter.charge_work((definitions.len() as u64).saturating_mul(64), &path)?;
        canonical.extend_from_slice(definitions);
        canonical.sort_unstable_by_key(StrongInitializationUnitDefinitionRefV2::unit);
        if let Some(pair) = canonical
            .windows(2)
            .find(|pair| pair[0].unit() == pair[1].unit())
        {
            return Err(
                InitializationDependencyResolutionError::DuplicateDefinition(pair[0].unit()),
            );
        }
        Ok(Self {
            producer,
            definitions: canonical,
        })
    }

    pub fn resolve(
        &self,
        local_unit: PersistentInitializationUnitId,
        dependencies: &[DecodedPersistentId<PersistentInitializationUnitId>],
        meter: &mut BudgetMeter,
    ) -> Result<ResolvedInitializationDependenciesV2, InitializationDependencyResolutionError> {
        let path = WirePath::root();
        let mut references = Vec::new();
        meter.try_reserve_collection_slots(&mut references, dependencies.len(), &path)?;
        meter.charge_work((dependencies.len() as u64).saturating_mul(64), &path)?;
        for (index, dependency) in dependencies.iter().enumerate() {
            if index != 0 && dependencies[index - 1].as_array() >= dependency.as_array() {
                return Err(InitializationDependencyResolutionError::NonCanonicalOrder { index });
            }
            if dependency.as_array() == local_unit.as_array() {
                return Err(InitializationDependencyResolutionError::SelfDependency(
                    local_unit,
                ));
            }
            let position = self
                .definitions
                .binary_search_by(|definition| {
                    definition.unit().as_array().cmp(dependency.as_array())
                })
                .map_err(|_| InitializationDependencyResolutionError::UnknownUnit(*dependency))?;
            let definition = self.definitions[position];
            let body = if definition.provider() == self.producer {
                DependencyBody::Local(definition)
            } else {
                DependencyBody::External(definition)
            };
            references.push(StrongInitializationDependencyRefV2(body));
        }
        Ok(ResolvedInitializationDependenciesV2 {
            local_unit,
            references,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedInitializationDependenciesV2 {
    local_unit: PersistentInitializationUnitId,
    references: Vec<StrongInitializationDependencyRefV2>,
}

impl ResolvedInitializationDependenciesV2 {
    pub const fn local_unit(&self) -> PersistentInitializationUnitId {
        self.local_unit
    }
    pub fn references(&self) -> &[StrongInitializationDependencyRefV2] {
        &self.references
    }
}

impl WireEncode for ResolvedInitializationDependenciesV2 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.references.len() as u64)?;
        for reference in &self.references {
            reference.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum InitializationDependencyResolutionError {
    Resource(WireError),
    DuplicateDefinition(PersistentInitializationUnitId),
    NonCanonicalOrder { index: usize },
    SelfDependency(PersistentInitializationUnitId),
    UnknownUnit(DecodedPersistentId<PersistentInitializationUnitId>),
}

impl From<WireError> for InitializationDependencyResolutionError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for InitializationDependencyResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid initialization dependency definitions: {self:?}")
    }
}
impl std::error::Error for InitializationDependencyResolutionError {}
