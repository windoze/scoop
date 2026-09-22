//! Source enum variants, payload fields and their shared wire constituents.

use super::*;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EnumSourceVariantStyleV1 {
    Unit,
    Positional,
    Named,
    Constructor,
}

impl WireEncode for EnumSourceVariantStyleV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Unit => 1,
            Self::Positional => 2,
            Self::Named => 3,
            Self::Constructor => 4,
        })
    }
}

impl WireDecode for EnumSourceVariantStyleV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Unit),
            2 => Ok(Self::Positional),
            3 => Ok(Self::Named),
            4 => Ok(Self::Constructor),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumSourceFieldV1 {
    pub(super) field: PersistentEnumVariantFieldId,
    pub(super) value_type: SignatureTypeKey,
}

impl EnumSourceFieldV1 {
    pub const fn new(field: PersistentEnumVariantFieldId, value_type: SignatureTypeKey) -> Self {
        Self { field, value_type }
    }

    pub const fn field(&self) -> PersistentEnumVariantFieldId {
        self.field
    }

    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }
}

impl WireEncode for EnumSourceFieldV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.field.encode(encoder)?;
        encoder.field(2)?;
        self.value_type.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedEnumSourceFieldV1 {
    pub(super) field: DecodedPersistentId<PersistentEnumVariantFieldId>,
    pub(super) value_type: DecodedSignatureTypeKey,
}

impl DecodedEnumSourceFieldV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<EnumSourceFieldV1, EnumSourceFieldResolutionError<E>>
    where
        R: NominalSourceShapeResolver<E>,
    {
        let field = resolver
            .resolve(self.field)
            .map_err(EnumSourceFieldResolutionError::Field)?;
        let value_type = self
            .value_type
            .resolve(resolver)
            .map_err(EnumSourceFieldResolutionError::ValueType)?;
        Ok(EnumSourceFieldV1 { field, value_type })
    }
}

impl WireEncode for DecodedEnumSourceFieldV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.field.encode(encoder)?;
        encoder.field(2)?;
        self.value_type.encode(encoder)
    }
}

impl WireDecode for DecodedEnumSourceFieldV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            field: decoder.field(1, DecodedPersistentId::decode)?,
            value_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumSourceVariantV1 {
    pub(super) variant: PersistentEnumVariantId,
    pub(super) style: EnumSourceVariantStyleV1,
    pub(super) fields: Vec<EnumSourceFieldV1>,
}

impl EnumSourceVariantV1 {
    pub fn try_new(
        variant: PersistentEnumVariantId,
        style: EnumSourceVariantStyleV1,
        fields: Vec<EnumSourceFieldV1>,
    ) -> Result<Self, EnumSourceVariantBuildError> {
        validate_variant_fields(style, &fields)?;
        Ok(Self {
            variant,
            style,
            fields,
        })
    }

    pub const fn variant(&self) -> PersistentEnumVariantId {
        self.variant
    }

    pub const fn style(&self) -> EnumSourceVariantStyleV1 {
        self.style
    }

    pub fn fields(&self) -> &[EnumSourceFieldV1] {
        &self.fields
    }
}

impl WireEncode for EnumSourceVariantV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.variant.encode(encoder)?;
        encoder.field(2)?;
        self.style.encode(encoder)?;
        encoder.field(3)?;
        encode_sequence(encoder, &self.fields)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedEnumSourceVariantV1 {
    pub(super) variant: DecodedPersistentId<PersistentEnumVariantId>,
    pub(super) style: EnumSourceVariantStyleV1,
    pub(super) fields: Vec<DecodedEnumSourceFieldV1>,
}

impl DecodedEnumSourceVariantV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<EnumSourceVariantV1, EnumSourceVariantResolutionError<E>>
    where
        R: NominalSourceShapeResolver<E>,
    {
        let variant = resolver
            .resolve(self.variant)
            .map_err(EnumSourceVariantResolutionError::Variant)?;
        u32::try_from(self.fields.len())
            .map_err(|_| EnumSourceVariantResolutionError::TooManyFields)?;
        if self.style == EnumSourceVariantStyleV1::Unit && !self.fields.is_empty() {
            return Err(EnumSourceVariantResolutionError::UnitFields);
        }
        let mut fields = Vec::with_capacity(self.fields.len());
        let mut ids = BTreeSet::new();
        for (index, field) in self.fields.into_iter().enumerate() {
            let field = field
                .resolve(resolver)
                .map_err(|error| EnumSourceVariantResolutionError::Field { index, error })?;
            if !ids.insert(field.field()) {
                return Err(EnumSourceVariantResolutionError::DuplicateField {
                    index,
                    field: field.field(),
                });
            }
            fields.push(field);
        }
        Ok(EnumSourceVariantV1 {
            variant,
            style: self.style,
            fields,
        })
    }
}

impl WireEncode for DecodedEnumSourceVariantV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.variant.encode(encoder)?;
        encoder.field(2)?;
        self.style.encode(encoder)?;
        encoder.field(3)?;
        encode_sequence(encoder, &self.fields)
    }
}

impl WireDecode for DecodedEnumSourceVariantV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            variant: decoder.field(1, DecodedPersistentId::decode)?,
            style: decoder.field(2, EnumSourceVariantStyleV1::decode)?,
            fields: decoder.field(3, |decoder| {
                decoder.decode_array(|decoder, _| DecodedEnumSourceFieldV1::decode(decoder))
            })?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumSourceShapeV1 {
    pub(super) variants: Vec<EnumSourceVariantV1>,
}

impl EnumSourceShapeV1 {
    pub fn try_new(
        variants: Vec<EnumSourceVariantV1>,
    ) -> Result<Self, NominalSourceShapeBuildError> {
        u32::try_from(variants.len())
            .map_err(|_| NominalSourceShapeBuildError::TooManyEnumVariants)?;
        let mut ids = BTreeSet::new();
        for variant in &variants {
            if !ids.insert(variant.variant()) {
                return Err(NominalSourceShapeBuildError::DuplicateEnumVariant(
                    variant.variant(),
                ));
            }
        }
        Ok(Self { variants })
    }

    pub fn variants(&self) -> &[EnumSourceVariantV1] {
        &self.variants
    }
}
