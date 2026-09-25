//! Canonical target-specific source extern and native-library requirements.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{
    CborIdentityRecord, ConeIdentity, NativeExternalContract, NativeExternalContractFingerprint,
    NativeExternalSymbolKey, NativeLibraryBinding, NativeLinkRequirementId,
    NativeLinkRequirementKey, PersistentNativeExternalSymbolId,
    PersistentSourceNativeExternalContractId, TargetProfileWireId,
};

use crate::{LirTargetProfile, OdrFreeLirFoundation};

mod resources;

pub type CanonicalNativeLibraryRequirementV1 =
    CborIdentityRecord<NativeLinkRequirementId, NativeLinkRequirementKey>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalNativeLibraryBindingV1 {
    DefaultNativeNamespace,
    Requirement(CanonicalNativeLibraryRequirementV1),
}

impl CanonicalNativeLibraryBindingV1 {
    pub const fn binding(&self) -> NativeLibraryBinding {
        match self {
            Self::DefaultNativeNamespace => NativeLibraryBinding::DefaultNativeNamespace,
            Self::Requirement(record) => NativeLibraryBinding::Requirement(record.id()),
        }
    }

    pub const fn requirement(&self) -> Option<&CanonicalNativeLibraryRequirementV1> {
        match self {
            Self::DefaultNativeNamespace => None,
            Self::Requirement(record) => Some(record),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalNativeExternalRequirementV1 {
    symbol_id: PersistentNativeExternalSymbolId,
    symbol_key: NativeExternalSymbolKey,
    fingerprint: NativeExternalContractFingerprint,
    contract: NativeExternalContract,
    sources: Vec<PersistentSourceNativeExternalContractId>,
    library: CanonicalNativeLibraryBindingV1,
}

impl CanonicalNativeExternalRequirementV1 {
    pub const fn symbol_id(&self) -> PersistentNativeExternalSymbolId {
        self.symbol_id
    }

    pub const fn symbol_key(&self) -> &NativeExternalSymbolKey {
        &self.symbol_key
    }

    pub const fn fingerprint(&self) -> NativeExternalContractFingerprint {
        self.fingerprint
    }

    pub const fn contract(&self) -> &NativeExternalContract {
        &self.contract
    }

    pub fn sources(&self) -> &[PersistentSourceNativeExternalContractId] {
        &self.sources
    }

    pub const fn library(&self) -> &CanonicalNativeLibraryBindingV1 {
        &self.library
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalNativeExternalRequirementSurfaceV1 {
    producer: ConeIdentity,
    target: LirTargetProfile,
    contracts: Vec<CanonicalNativeExternalRequirementV1>,
    library_requirements: Vec<CanonicalNativeLibraryRequirementV1>,
}

impl CanonicalNativeExternalRequirementSurfaceV1 {
    /// Replays native requirements using the caller's cumulative artifact
    /// budget, without exposing the foundation's private library records.
    pub fn from_foundation_with_meter(
        target: LirTargetProfile,
        foundation: &OdrFreeLirFoundation,
        meter: &mut scoop_wire::BudgetMeter,
    ) -> Result<Self, CanonicalNativeExternalRequirementBuildError> {
        resources::charge_foundation(foundation, meter)
            .map_err(CanonicalNativeExternalRequirementBuildError::Resource)?;
        Self::from_foundation(target, foundation)
    }

    pub fn from_foundation(
        target: LirTargetProfile,
        foundation: &OdrFreeLirFoundation,
    ) -> Result<Self, CanonicalNativeExternalRequirementBuildError> {
        let target_wire_id = target.wire_id();
        let mut requirements = BTreeMap::new();
        for record in foundation.native_link_requirements() {
            if record.key().target_profile() != &target_wire_id {
                return Err(
                    CanonicalNativeExternalRequirementBuildError::LibraryTargetMismatch {
                        requirement: record.id(),
                        expected: Box::new(target_wire_id),
                        actual: Box::new(record.key().target_profile().clone()),
                    },
                );
            }
            requirements.insert(record.id(), record.clone());
        }

        let mut grouped = BTreeMap::<Vec<u8>, CanonicalNativeExternalRequirementV1>::new();
        let mut used_requirements = BTreeSet::new();
        for record in foundation.native_contracts() {
            if record.symbol_key().target_profile() != &target_wire_id {
                return Err(
                    CanonicalNativeExternalRequirementBuildError::ContractTargetMismatch {
                        source: record.source(),
                        expected: Box::new(target_wire_id),
                        actual: Box::new(record.symbol_key().target_profile().clone()),
                    },
                );
            }
            let library = match record.contract().library() {
                NativeLibraryBinding::DefaultNativeNamespace => {
                    CanonicalNativeLibraryBindingV1::DefaultNativeNamespace
                }
                NativeLibraryBinding::Requirement(id) => {
                    let requirement = requirements.get(&id).ok_or(
                        CanonicalNativeExternalRequirementBuildError::MissingLibraryRequirement {
                            source: record.source(),
                            requirement: id,
                        },
                    )?;
                    used_requirements.insert(id);
                    CanonicalNativeLibraryBindingV1::Requirement(requirement.clone())
                }
            };
            let symbol = record.symbol_key().native_link_symbol().as_bytes().to_vec();
            match grouped.get_mut(&symbol) {
                Some(existing) => {
                    if existing.symbol_id != record.symbol_id()
                        || existing.symbol_key != *record.symbol_key()
                        || existing.fingerprint != record.fingerprint()
                        || existing.contract != *record.contract()
                        || existing.library.binding() != record.contract().library()
                    {
                        return Err(
                            CanonicalNativeExternalRequirementBuildError::ConflictingContract {
                                symbol,
                                first_source: existing.sources[0],
                                second_source: record.source(),
                            },
                        );
                    }
                    existing.sources.push(record.source());
                }
                None => {
                    grouped.insert(
                        symbol,
                        CanonicalNativeExternalRequirementV1 {
                            symbol_id: record.symbol_id(),
                            symbol_key: record.symbol_key().clone(),
                            fingerprint: record.fingerprint(),
                            contract: record.contract().clone(),
                            sources: vec![record.source()],
                            library,
                        },
                    );
                }
            }
        }

        if let Some(requirement) = requirements
            .keys()
            .find(|requirement| !used_requirements.contains(*requirement))
        {
            return Err(
                CanonicalNativeExternalRequirementBuildError::UnusedLibraryRequirement(
                    *requirement,
                ),
            );
        }

        Ok(Self {
            producer: foundation.producer(),
            target,
            contracts: grouped.into_values().collect(),
            library_requirements: requirements.into_values().collect(),
        })
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub const fn target(&self) -> LirTargetProfile {
        self.target
    }

    pub fn contracts(&self) -> &[CanonicalNativeExternalRequirementV1] {
        &self.contracts
    }

    pub fn library_requirements(&self) -> &[CanonicalNativeLibraryRequirementV1] {
        &self.library_requirements
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalNativeExternalRequirementBuildError {
    Resource(scoop_wire::WireError),
    ContractTargetMismatch {
        source: PersistentSourceNativeExternalContractId,
        expected: Box<TargetProfileWireId>,
        actual: Box<TargetProfileWireId>,
    },
    LibraryTargetMismatch {
        requirement: NativeLinkRequirementId,
        expected: Box<TargetProfileWireId>,
        actual: Box<TargetProfileWireId>,
    },
    MissingLibraryRequirement {
        source: PersistentSourceNativeExternalContractId,
        requirement: NativeLinkRequirementId,
    },
    UnusedLibraryRequirement(NativeLinkRequirementId),
    ConflictingContract {
        symbol: Vec<u8>,
        first_source: PersistentSourceNativeExternalContractId,
        second_source: PersistentSourceNativeExternalContractId,
    },
}

impl fmt::Display for CanonicalNativeExternalRequirementBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid native external requirement surface: {self:?}"
        )
    }
}

impl std::error::Error for CanonicalNativeExternalRequirementBuildError {}

#[cfg(test)]
mod tests;
