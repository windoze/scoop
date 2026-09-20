use super::*;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode};

mod errors;
mod resolve;
pub use errors::*;
pub use resolve::TypeDeclarationSourceResolver;

impl WireEncode for TypeDeclarationSourceAuthorityV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let value = self.entries();
        e.map(9)?;
        e.field(1)?;
        value.required_protected.encode(e)?;
        e.field(2)?;
        value.nominals.encode(e)?;
        e.field(3)?;
        value.constructors.encode(e)?;
        e.field(4)?;
        value.properties.encode(e)?;
        e.field(5)?;
        value.callables.encode(e)?;
        e.field(6)?;
        value.inheritance.encode(e)?;
        e.field(7)?;
        value.interfaces.encode(e)?;
        e.field(8)?;
        value.selections.encode(e)?;
        e.field(9)?;
        value.dispatch_callables.encode(e)?;
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedTypeDeclarationSourceAuthorityV1 {
    required_protected: DecodedCanonicalProtectedDeclarationRefsV1,
    nominals: DecodedCanonicalNominalSourceContractsV1,
    constructors: DecodedCanonicalNominalSourceConstructorsV1,
    properties: DecodedCanonicalNominalSourcePropertiesV1,
    callables: DecodedCanonicalNominalSourceCallablesV1,
    inheritance: DecodedCanonicalSourceInheritanceInventoriesV1,
    interfaces: DecodedCanonicalInterfaceSourceDispatchesV1,
    selections: DecodedCanonicalInheritanceSourceSlotSelectionsV1,
    dispatch_callables: DecodedCanonicalInheritanceSourceCallablesV1,
}
impl WireDecode for DecodedTypeDeclarationSourceAuthorityV1 {
    fn decode(d: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        d.expect_map(9)?;
        Ok(Self {
            required_protected: d.field(1, DecodedCanonicalProtectedDeclarationRefsV1::decode)?,
            nominals: d.field(2, DecodedCanonicalNominalSourceContractsV1::decode)?,
            constructors: d.field(3, DecodedCanonicalNominalSourceConstructorsV1::decode)?,
            properties: d.field(4, DecodedCanonicalNominalSourcePropertiesV1::decode)?,
            callables: d.field(5, DecodedCanonicalNominalSourceCallablesV1::decode)?,
            inheritance: d.field(6, DecodedCanonicalSourceInheritanceInventoriesV1::decode)?,
            interfaces: d.field(7, DecodedCanonicalInterfaceSourceDispatchesV1::decode)?,
            selections: d.field(8, DecodedCanonicalInheritanceSourceSlotSelectionsV1::decode)?,
            dispatch_callables: d.field(9, DecodedCanonicalInheritanceSourceCallablesV1::decode)?,
        })
    }
}
impl WireEncode for DecodedTypeDeclarationSourceAuthorityV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(9)?;
        e.field(1)?;
        self.required_protected.encode(e)?;
        e.field(2)?;
        self.nominals.encode(e)?;
        e.field(3)?;
        self.constructors.encode(e)?;
        e.field(4)?;
        self.properties.encode(e)?;
        e.field(5)?;
        self.callables.encode(e)?;
        e.field(6)?;
        self.inheritance.encode(e)?;
        e.field(7)?;
        self.interfaces.encode(e)?;
        e.field(8)?;
        self.selections.encode(e)?;
        e.field(9)?;
        self.dispatch_callables.encode(e)?;
        Ok(())
    }
}
