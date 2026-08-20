//! The runtime's data alias points at an existing, typed Cone definition.
use crate::StrongDefinitionOwnerV1;
use scoop_identity::{StrongDefinitionEntity, StrongDefinitionRole};
use scoop_lir::{
    ExactDescriptorExportV1, InstanceRepresentationKindV1, LirTargetProfile, RuntimeAbiSymbolV1,
    RuntimeSymbolContractId, RuntimeSymbolContractV1,
};

#[cfg(test)]
mod tests;
mod wire;
pub use wire::DecodedLirLinkSupportSectionV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDataAliasV1 {
    contract: RuntimeSymbolContractId,
    owner: StrongDefinitionOwnerV1,
}

impl RuntimeDataAliasV1 {
    pub const fn contract(&self) -> RuntimeSymbolContractId {
        self.contract
    }
    pub const fn owner(&self) -> StrongDefinitionOwnerV1 {
        self.owner
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LirLinkSupportSectionV1 {
    aliases: Vec<RuntimeDataAliasV1>,
}

impl LirLinkSupportSectionV1 {
    pub fn from_string_descriptor(
        descriptor: Option<&ExactDescriptorExportV1>,
    ) -> Result<Self, LinkSupportError> {
        let Some(descriptor) = descriptor else {
            return Ok(Self::default());
        };
        if !matches!(
            descriptor.instance_layout().representation().kind(),
            InstanceRepresentationKindV1::InlineBytes
        ) {
            return Err(LinkSupportError(
                "runtime String alias requires its InlineBytes descriptor".into(),
            ));
        }
        let target = descriptor.value_layout().identity().target();
        let contract = string_contract(target)?;
        let owner = StrongDefinitionOwnerV1::new(
            StrongDefinitionEntity::exact_type(descriptor.exact()),
            StrongDefinitionRole::TypeDescriptor,
        )
        .map_err(err)?;
        Ok(Self {
            aliases: vec![RuntimeDataAliasV1 { contract, owner }],
        })
    }

    pub fn runtime_data_aliases(&self) -> &[RuntimeDataAliasV1] {
        &self.aliases
    }
}

fn string_contract(target: LirTargetProfile) -> Result<RuntimeSymbolContractId, LinkSupportError> {
    RuntimeSymbolContractV1::current(target, RuntimeAbiSymbolV1::CoreStringTypeDescriptor)
        .map(|contract| contract.id())
        .map_err(err)
}

#[derive(Debug)]
pub struct LinkSupportError(pub String);
fn err(value: impl std::fmt::Display) -> LinkSupportError {
    LinkSupportError(value.to_string())
}
impl std::fmt::Display for LinkSupportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for LinkSupportError {}
