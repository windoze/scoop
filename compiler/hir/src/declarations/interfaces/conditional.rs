//! Declaration identities and checked environments for conditional conformance.

use super::*;
use scoop_identity::{SignatureConditionKey, SignatureTypeKey};
use scoop_wire::{Digest256, Encoder, WireEncode, domain_separated_cbor_hash};

/// A declaration key in the original owner's binder domain. It is stable
/// across imports and independent of source order and local arena indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConditionalInterfaceId(Digest256);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionalInterfaceKey {
    pub owner: SourceNominalId,
    pub interface: SignatureTypeKey,
    pub conditions: Vec<SignatureConditionKey>,
}

impl ConditionalInterfaceKey {
    pub fn id(&self) -> ConditionalInterfaceId {
        ConditionalInterfaceId(
            domain_separated_cbor_hash("scoop-conditional-interface-v1", self)
                .expect("checked interface declaration keys have canonical encodings"),
        )
    }
}

impl WireEncode for ConditionalInterfaceKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(3)?;
        self.owner.encode(encoder)?;
        self.interface.encode(encoder)?;
        encoder.array(self.conditions.len() as u64)?;
        for condition in &self.conditions {
            condition.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionalInterfaceDeclaration {
    pub key: ConditionalInterfaceKey,
    pub interface: TypeId,
    /// The original owner parameters with the block's additional facts.
    /// These parameters do not alter nominal application well-formedness.
    pub parameters: Vec<TypeParamDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionalInterfaceApplication {
    pub declaration: ConditionalInterfaceId,
    pub owner: SourceNominalId,
    pub arguments: Vec<TypeId>,
}

impl ConditionalInterfaceDeclaration {
    pub fn id(&self) -> ConditionalInterfaceId {
        self.key.id()
    }
}

impl Module {
    pub fn conditional_interfaces(
        &self,
        owner: SourceNominalId,
    ) -> &[ConditionalInterfaceDeclaration] {
        if let Some(owner) = self.nominal_identities.declaration(owner) {
            return match owner {
                NominalOwner::Class(id) => &self.classes[id].conditional_interfaces,
                NominalOwner::Struct(id) => &self.structs[id].conditional_interfaces,
                NominalOwner::Enum(id) => &self.enums[id].conditional_interfaces,
                NominalOwner::Object(id) => {
                    &self.classes[self.objects[id].backing_class].conditional_interfaces
                }
                NominalOwner::Interface(_) => &[],
            };
        }
        if let Some(definition) = self.loaded_class_definitions.get(&owner) {
            &definition.definition.conditional_interfaces
        } else if let Some(definition) = self.loaded_struct_definitions.get(&owner) {
            &definition.definition.conditional_interfaces
        } else if let Some(definition) = self.loaded_enum_definitions.get(&owner) {
            &definition.definition.conditional_interfaces
        } else {
            assert!(
                self.loaded_interface_definitions.contains_key(&owner),
                "conditional queries retain their nominal definition"
            );
            &[]
        }
    }
}
