//! Canonical capability and native-contract contributions to the code fingerprint.

use std::fmt;

use scoop_identity::{
    CapabilityId, NativeExternalContract, NativeExternalContractFingerprint,
    NativeExternalSymbolKey, PersistentNativeExternalSymbolId,
};
use scoop_lir::CanonicalNativeExternalRequirementSurfaceV1;
use scoop_wire::{Encoder, WireEncode, encode};

use crate::{
    link_object::CrossConeLinkSemanticImportSetV1, lir_cross_cone_link_closure_capability,
};

/// One contribution produced by a registered Link-purpose capability handler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnownLinkExtensionCodeContributionV1 {
    capability: CapabilityId,
    canonical_projection: Vec<u8>,
}

impl KnownLinkExtensionCodeContributionV1 {
    pub const fn capability(&self) -> &CapabilityId {
        &self.capability
    }

    pub fn canonical_projection(&self) -> &[u8] {
        &self.canonical_projection
    }
}

impl WireEncode for KnownLinkExtensionCodeContributionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.capability.encode(encoder)?;
        encoder.field(2)?;
        encoder.bytes(&self.canonical_projection)
    }
}

/// Canonical field-2 contribution set in `CodeFingerprintInputV1`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalKnownLinkExtensionCodeContributionSetV1 {
    contributions: Vec<KnownLinkExtensionCodeContributionV1>,
}

impl CanonicalKnownLinkExtensionCodeContributionSetV1 {
    pub(super) const fn empty() -> Self {
        Self {
            contributions: Vec::new(),
        }
    }

    pub(super) fn from_cross_cone_semantic_imports(
        semantic_imports: &CrossConeLinkSemanticImportSetV1,
    ) -> Result<Self, scoop_wire::cbor::EncodeError> {
        Ok(Self {
            contributions: vec![KnownLinkExtensionCodeContributionV1 {
                capability: lir_cross_cone_link_closure_capability(),
                canonical_projection: encode(semantic_imports)?,
            }],
        })
    }

    pub fn contributions(&self) -> &[KnownLinkExtensionCodeContributionV1] {
        &self.contributions
    }
}

impl WireEncode for CanonicalKnownLinkExtensionCodeContributionSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.contributions.len() as u64)?;
        for contribution in &self.contributions {
            contribution.encode(encoder)?;
        }
        Ok(())
    }
}

/// One native external contract stripped of source-only provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeExternalContractCodeRecordV1 {
    symbol_id: PersistentNativeExternalSymbolId,
    symbol_key: NativeExternalSymbolKey,
    fingerprint: NativeExternalContractFingerprint,
    contract: NativeExternalContract,
}

impl NativeExternalContractCodeRecordV1 {
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
}

impl WireEncode for NativeExternalContractCodeRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.symbol_id.encode(encoder)?;
        encoder.field(2)?;
        self.symbol_key.encode(encoder)?;
        encoder.field(3)?;
        self.fingerprint.encode(encoder)?;
        encoder.field(4)?;
        self.contract.encode(encoder)
    }
}

/// Canonical native contract contribution to `CodeFingerprint`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalNativeExternalContractCodeSetV1 {
    pub(super) contracts: Vec<NativeExternalContractCodeRecordV1>,
}

impl CanonicalNativeExternalContractCodeSetV1 {
    pub fn from_requirement_surface(
        surface: &CanonicalNativeExternalRequirementSurfaceV1,
    ) -> Result<Self, NativeExternalContractCodeSetBuildError> {
        let mut contracts = surface
            .contracts()
            .iter()
            .map(|requirement| NativeExternalContractCodeRecordV1 {
                symbol_id: requirement.symbol_id(),
                symbol_key: requirement.symbol_key().clone(),
                fingerprint: requirement.fingerprint(),
                contract: requirement.contract().clone(),
            })
            .collect::<Vec<_>>();
        contracts.sort_unstable_by(|left, right| {
            left.symbol_key
                .native_link_symbol()
                .as_bytes()
                .cmp(right.symbol_key.native_link_symbol().as_bytes())
        });
        if let Some(pair) = contracts.windows(2).find(|pair| {
            pair[0].symbol_key.native_link_symbol().as_bytes()
                == pair[1].symbol_key.native_link_symbol().as_bytes()
        }) {
            return Err(NativeExternalContractCodeSetBuildError::DuplicateSymbol(
                pair[0].symbol_key.native_link_symbol().as_bytes().to_vec(),
            ));
        }
        Ok(Self { contracts })
    }

    pub fn contracts(&self) -> &[NativeExternalContractCodeRecordV1] {
        &self.contracts
    }
}

impl WireEncode for CanonicalNativeExternalContractCodeSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.contracts.len() as u64)?;
        for contract in &self.contracts {
            contract.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeExternalContractCodeSetBuildError {
    DuplicateSymbol(Vec<u8>),
}

impl fmt::Display for NativeExternalContractCodeSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid native external contract code set: {self:?}"
        )
    }
}

impl std::error::Error for NativeExternalContractCodeSetBuildError {}
