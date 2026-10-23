//! Conditional relations retain their original owner's binder and declaration identity.

use scoop_identity::{DecodedSignatureTypeKey, SignatureTypeKey};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::{
    CanonicalBinderListV1, ConditionalInterfaceId, DecodedCanonicalBinderListV1,
    NominalInterfaceRecordResolver,
};

mod application;
mod conformance;
mod key;
mod wire;
pub use application::{
    ConditionalInterfaceApplicationV1, DecodedConditionalInterfaceApplicationV1,
};
pub use conformance::{
    ConditionalInterfaceConformanceV1, DecodedConditionalInterfaceConformanceV1,
};
pub use wire::{ConditionalInterfaceRecordResolutionError, DecodedConditionalInterfaceRecordV1};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConditionalInterfaceRecordV1 {
    id: ConditionalInterfaceId,
    interface: SignatureTypeKey,
    parameters: CanonicalBinderListV1,
}

impl ConditionalInterfaceRecordV1 {
    pub fn new(
        id: ConditionalInterfaceId,
        interface: SignatureTypeKey,
        parameters: CanonicalBinderListV1,
    ) -> Self {
        Self {
            id,
            interface,
            parameters,
        }
    }

    pub const fn id(&self) -> ConditionalInterfaceId {
        self.id
    }

    pub fn interface(&self) -> &SignatureTypeKey {
        &self.interface
    }

    pub fn parameters(&self) -> &CanonicalBinderListV1 {
        &self.parameters
    }
}

impl crate::NominalInterfaceRecordV1 {
    pub fn member_owner_parameters(
        &self,
        availability: Option<ConditionalInterfaceId>,
    ) -> Option<&CanonicalBinderListV1> {
        match availability {
            None => Some(self.type_parameters()),
            Some(id) => self
                .declaration_details()
                .conditional_interfaces()
                .iter()
                .find(|implementation| implementation.id() == id)
                .map(ConditionalInterfaceRecordV1::parameters),
        }
    }
}
