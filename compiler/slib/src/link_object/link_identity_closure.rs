//! Member assignments and symbol tables consumed by program linking.

use super::{
    CanonicalDefinedLinkSymbolOwnerSetV1, CanonicalScoopLirObjectUnitSetV1,
    CanonicalUndefinedSymbolRequirementSetV1, CodeFingerprintDataV1, CodeFingerprintDataV2,
    FinalCodeObjectSetV1, PlannedLinkObjectMemberSetV1,
};
use crate::SlibMemberId;
use scoop_wire::{Encoder, WireEncode};

mod wire;
pub use wire::DecodedLinkIdentityClosureSectionV1;

#[cfg(test)]
pub(crate) fn encoded_link_identity_closure_for_test() -> Vec<u8> {
    wire::tests::encoded_link_identity_closure_for_test()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LinkObjectMaterializationV1 {
    ScoopLir {
        member: SlibMemberId,
        units: CanonicalScoopLirObjectUnitSetV1,
    },
    GeneratedCBridge {
        member: SlibMemberId,
    },
}

impl LinkObjectMaterializationV1 {
    pub const fn member(&self) -> SlibMemberId {
        match self {
            Self::ScoopLir { member, .. } | Self::GeneratedCBridge { member, .. } => *member,
        }
    }

    pub const fn scoop_lir_units(&self) -> Option<&CanonicalScoopLirObjectUnitSetV1> {
        match self {
            Self::ScoopLir { units, .. } => Some(units),
            Self::GeneratedCBridge { .. } => None,
        }
    }
}

impl WireEncode for LinkObjectMaterializationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(match self {
            Self::ScoopLir { .. } => 3,
            Self::GeneratedCBridge { .. } => 2,
        })?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::ScoopLir { .. } => 1,
            Self::GeneratedCBridge { .. } => 2,
        })?;
        encoder.field(1)?;
        self.member().encode(encoder)?;
        if let Self::ScoopLir { units, .. } = self {
            encoder.field(2)?;
            encode_array(encoder, units.units())?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkIdentityClosureSectionV1 {
    materializations: Vec<LinkObjectMaterializationV1>,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    symbol_uses: Vec<SymbolUse>,
}

impl LinkIdentityClosureSectionV1 {
    pub fn materializations(&self) -> &[LinkObjectMaterializationV1] {
        &self.materializations
    }

    pub const fn defined_symbols(&self) -> &CanonicalDefinedLinkSymbolOwnerSetV1 {
        &self.defined_symbols
    }

    pub fn from_verified_code(code: &CodeFingerprintDataV1) -> Self {
        Self::from_code_parts(
            code.production().link_objects(),
            code.defined_symbols(),
            code.undefined_symbols(),
        )
    }

    pub fn from_verified_layout_code(code: &CodeFingerprintDataV2) -> Self {
        Self::from_code_parts(
            code.production().link_objects(),
            code.defined_symbols(),
            code.undefined_symbols(),
        )
    }

    fn from_code_parts<D, C, I>(
        objects: &FinalCodeObjectSetV1<D, C, I>,
        defined_symbols: &CanonicalDefinedLinkSymbolOwnerSetV1,
        undefined_symbols: &CanonicalUndefinedSymbolRequirementSetV1,
    ) -> Self
    where
        D: scoop_lir::StrongDescriptorReference,
        C: Clone,
    {
        let builtins = objects.final_objects().entry().builtins();
        Self {
            materializations: materializations(builtins.member_plan()),
            defined_symbols: defined_symbols.clone(),
            symbol_uses: symbol_uses(undefined_symbols),
        }
    }
}

impl WireEncode for LinkIdentityClosureSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        encode_array(encoder, &self.materializations)?;
        encoder.field(4)?;
        self.defined_symbols.encode(encoder)?;
        encoder.field(5)?;
        encode_array(encoder, &self.symbol_uses)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SymbolUse {
    member: SlibMemberId,
    symbol: Vec<u8>,
}

impl WireEncode for SymbolUse {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.member.encode(encoder)?;
        encoder.field(2)?;
        encoder.bytes(&self.symbol)
    }
}

fn symbol_uses(requirements: &CanonicalUndefinedSymbolRequirementSetV1) -> Vec<SymbolUse> {
    let mut uses = requirements
        .requirements()
        .iter()
        .map(|requirement| {
            let site = requirement.use_site();
            SymbolUse {
                member: site.source_member(),
                symbol: site.symbol().to_vec(),
            }
        })
        .collect::<Vec<_>>();
    uses.sort_unstable();
    uses.dedup();
    uses
}

fn materializations(plan: &PlannedLinkObjectMemberSetV1) -> Vec<LinkObjectMaterializationV1> {
    let mut result = plan
        .scoop_lir_members()
        .iter()
        .map(|member| LinkObjectMaterializationV1::ScoopLir {
            member: member.member_id(),
            units: member.units().clone(),
        })
        .chain(plan.generated_bridge_members().iter().map(|member| {
            LinkObjectMaterializationV1::GeneratedCBridge {
                member: member.member_id(),
            }
        }))
        .collect::<Vec<_>>();
    result.sort_unstable_by_key(LinkObjectMaterializationV1::member);
    result
}

fn encode_array(
    encoder: &mut Encoder,
    values: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
