use super::*;

macro_rules! encode_identity {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                e.map(2)?;
                e.field(1)?;
                self.source.encode(e)?;
                e.field(2)?;
                self.span.encode(e)
            }
        }
    };
}
encode_identity!(AnnotationTypeUseId);
encode_identity!(DecodedTypeUseId);

macro_rules! encode_application {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                e.map(4)?;
                e.field(1)?;
                self.annotation.encode(e)?;
                e.field(2)?;
                sequence(e, &self.arguments)?;
                e.field(3)?;
                self.source.encode(e)?;
                e.field(4)?;
                self.span.encode(e)
            }
        }
    };
}
encode_application!(TypeAnnotation);
encode_application!(DecodedTypeAnnotation);

macro_rules! encode_node {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                e.map(2)?;
                e.field(1)?;
                sequence(e, &self.path)?;
                e.field(2)?;
                sequence(e, &self.annotations)
            }
        }
    };
}
encode_node!(TypeAnnotationNode);
encode_node!(DecodedTypeAnnotationNode);

macro_rules! encode_use {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                e.map(4)?;
                e.field(1)?;
                self.identity.encode(e)?;
                e.field(2)?;
                self.ty.encode(e)?;
                e.field(3)?;
                e.unsigned(self.binder_count.into())?;
                e.field(4)?;
                sequence(e, &self.nodes)
            }
        }
    };
}
encode_use!(AnnotatedTypeUseV1);
encode_use!(DecodedAnnotatedTypeUseV1);

macro_rules! encode_site {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                e.map(3)?;
                e.field(1)?;
                self.target.encode(e)?;
                e.field(2)?;
                self.slot.encode(e)?;
                e.field(3)?;
                self.type_use.encode(e)
            }
        }
    };
}
encode_site!(AnnotationTypeSiteV1);
encode_site!(DecodedAnnotationTypeSiteV1);
