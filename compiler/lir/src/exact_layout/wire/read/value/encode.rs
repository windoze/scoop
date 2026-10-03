use super::*;

impl WireEncode for RawValue {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        match self {
            Self::Scalar(kind) => {
                sum(encoder, 1, 1)?;
                encoder.field(1)?;
                crate::exact_layout::wire::value::scalar(encoder, *kind)
            }
            Self::QualifiedPointer(kind) => {
                sum(encoder, 2, 1)?;
                encoder.field(1)?;
                pointer(encoder, *kind)
            }
            Self::Struct {
                policy,
                interior_mutable,
                fields,
            } => {
                sum(encoder, 3, 3)?;
                field(encoder, 1, policy)?;
                unsigned(encoder, 2, u64::from(*interior_mutable))?;
                encoder.field(3)?;
                array(encoder, fields)
            }
            Self::Tuple(elements) => {
                sum(encoder, 4, 1)?;
                encoder.field(1)?;
                array(encoder, elements)
            }
            Self::TaggedEnum {
                tag,
                pure,
                variants,
            } => {
                sum(encoder, 5, 3)?;
                field(encoder, 1, tag)?;
                field(encoder, 2, pure)?;
                encoder.field(3)?;
                array(encoder, variants)
            }
            Self::NicheEnum {
                pointer: kind,
                variants,
                payload,
            } => {
                sum(encoder, 6, 3)?;
                encoder.field(1)?;
                pointer(encoder, *kind)?;
                encoder.field(2)?;
                array(encoder, variants)?;
                field(encoder, 3, payload)
            }
            Self::Unit => {
                sum(encoder, 7, 1)?;
                encoder.field(1)?;
                sum(encoder, 1, 0)
            }
        }
    }
}

impl WireEncode for RawPolicy {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        match self {
            Self::Ordinary => sum(encoder, 1, 0),
            Self::CLayout {
                aligned,
                packed,
                contract,
            } => {
                sum(encoder, 2, 3)?;
                encoder.field(1)?;
                crate::exact_layout::wire::value::alignment(encoder, *aligned)?;
                encoder.field(2)?;
                crate::exact_layout::wire::value::alignment(encoder, *packed)?;
                field(encoder, 3, contract)
            }
        }
    }
}
