use super::*;

impl WireEncode for DecodedHirDependencyTypeSiteV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Expression {
                root,
                expression_index,
                origin,
                role,
                exact,
            } => {
                encoder.map(6)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                root.encode(encoder)?;
                encoder.field(2)?;
                encoder.unsigned(u64::from(*expression_index))?;
                encoder.field(3)?;
                origin.encode(encoder)?;
                encoder.field(4)?;
                role.encode(encoder)?;
                encoder.field(5)?;
                exact.encode(encoder)
            }
            Self::CallableSignature {
                root,
                position,
                exact,
            } => {
                encoder.map(4)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                root.encode(encoder)?;
                encoder.field(2)?;
                position.encode(encoder)?;
                encoder.field(3)?;
                exact.encode(encoder)
            }
            Self::LocalValue { local, exact } => {
                super::super::wire::declaration(encoder, 3, local, exact)
            }
            Self::BackingStorage { property, exact } => {
                super::super::wire::declaration(encoder, 4, property, exact)
            }
            Self::DelegateStorage { property, exact } => {
                super::super::wire::declaration(encoder, 5, property, exact)
            }
            Self::FieldStorage {
                owner,
                field,
                exact,
            } => super::super::wire::storage(encoder, 6, field, exact, owner),
            Self::EnumVariantFieldStorage {
                owner,
                field,
                exact,
            } => super::super::wire::storage(encoder, 7, field, exact, owner),
            Self::ConstructorInitializerResult { constructor, exact } => {
                super::super::wire::declaration(encoder, 8, constructor, exact)
            }
            Self::InitializationCycleMessage { unit, exact } => {
                super::super::wire::declaration(encoder, 9, unit, exact)
            }
            Self::GenericDelegateStorage { unit, exact } => {
                super::super::wire::declaration(encoder, 10, unit, exact)
            }
        }
    }
}
