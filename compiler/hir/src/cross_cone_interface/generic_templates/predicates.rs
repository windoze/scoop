use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::{
    BinderUseListValidationError, CanonicalBinderUseListV1, DecodedCanonicalBinderUseListV1,
    SignatureTypeReferenceResolver,
};

/// Conditions inferred by the frontend from a callable's actual implementation.
/// Nominal bounds remain on the existing declaration interface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenericTemplatePredicatesV1 {
    no_gc: CanonicalBinderUseListV1,
    gc_free_pointees: CanonicalBinderUseListV1,
}

impl GenericTemplatePredicatesV1 {
    pub const fn new(
        no_gc: CanonicalBinderUseListV1,
        gc_free_pointees: CanonicalBinderUseListV1,
    ) -> Self {
        Self {
            no_gc,
            gc_free_pointees,
        }
    }

    pub const fn no_gc(&self) -> &CanonicalBinderUseListV1 {
        &self.no_gc
    }

    pub const fn gc_free_pointees(&self) -> &CanonicalBinderUseListV1 {
        &self.gc_free_pointees
    }
}

impl WireEncode for GenericTemplatePredicatesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.no_gc.encode(encoder)?;
        encoder.field(2)?;
        self.gc_free_pointees.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedGenericTemplatePredicatesV1 {
    no_gc: DecodedCanonicalBinderUseListV1,
    gc_free_pointees: DecodedCanonicalBinderUseListV1,
}

impl DecodedGenericTemplatePredicatesV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<GenericTemplatePredicatesV1, BinderUseListValidationError<E>>
    where
        R: SignatureTypeReferenceResolver<E>,
    {
        Ok(GenericTemplatePredicatesV1::new(
            self.no_gc.resolve(resolver)?,
            self.gc_free_pointees.resolve(resolver)?,
        ))
    }
}

impl WireEncode for DecodedGenericTemplatePredicatesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.no_gc.encode(encoder)?;
        encoder.field(2)?;
        self.gc_free_pointees.encode(encoder)
    }
}

impl WireDecode for DecodedGenericTemplatePredicatesV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            no_gc: decoder.field(1, DecodedCanonicalBinderUseListV1::decode)?,
            gc_free_pointees: decoder.field(2, DecodedCanonicalBinderUseListV1::decode)?,
        })
    }
}
