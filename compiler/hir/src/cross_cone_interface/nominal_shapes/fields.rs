use super::*;

/// Declaration order is meaningful; identities may occur only once.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NominalSourceFieldsV1 {
    fields: Vec<NominalSourceFieldV1>,
}

impl NominalSourceFieldsV1 {
    pub fn try_new(
        fields: Vec<NominalSourceFieldV1>,
    ) -> Result<Self, NominalSourceShapeBuildError> {
        u32::try_from(fields.len())
            .map_err(|_| NominalSourceShapeBuildError::TooManyNominalFields)?;
        let mut ids = BTreeSet::new();
        for field in &fields {
            if !ids.insert(field.field()) {
                return Err(NominalSourceShapeBuildError::DuplicateNominalField(
                    field.field(),
                ));
            }
        }
        Ok(Self { fields })
    }

    pub fn fields(&self) -> &[NominalSourceFieldV1] {
        &self.fields
    }
}

impl WireEncode for NominalSourceFieldsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_sequence(encoder, &self.fields)
    }
}

pub(super) fn resolve<R: NominalSourceShapeResolver<E>, E>(
    decoded: Vec<DecodedNominalSourceFieldV1>,
    resolver: &mut R,
) -> Result<NominalSourceFieldsV1, NominalSourceShapeResolutionError<E>> {
    u32::try_from(decoded.len())
        .map_err(|_| NominalSourceShapeResolutionError::TooManyNominalFields)?;
    let mut fields = Vec::with_capacity(decoded.len());
    let mut ids = BTreeSet::new();
    for (index, field) in decoded.into_iter().enumerate() {
        let field = field
            .resolve(resolver)
            .map_err(|error| NominalSourceShapeResolutionError::NominalField { index, error })?;
        if !ids.insert(field.field()) {
            return Err(NominalSourceShapeResolutionError::DuplicateNominalField {
                index,
                field: field.field(),
            });
        }
        fields.push(field);
    }
    Ok(NominalSourceFieldsV1 { fields })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalSourceFieldV1 {
    field: PersistentFieldId,
    value_type: SignatureTypeKey,
}

impl NominalSourceFieldV1 {
    pub const fn new(field: PersistentFieldId, value_type: SignatureTypeKey) -> Self {
        Self { field, value_type }
    }

    pub const fn field(&self) -> PersistentFieldId {
        self.field
    }

    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }
}

impl WireEncode for NominalSourceFieldV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.field.encode(encoder)?;
        encoder.field(2)?;
        self.value_type.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNominalSourceFieldV1 {
    field: DecodedPersistentId<PersistentFieldId>,
    pub(super) value_type: DecodedSignatureTypeKey,
}

impl DecodedNominalSourceFieldV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<NominalSourceFieldV1, NominalSourceFieldResolutionError<E>>
    where
        R: NominalSourceShapeResolver<E>,
    {
        let field = resolver
            .resolve(self.field)
            .map_err(NominalSourceFieldResolutionError::Field)?;
        let value_type = self
            .value_type
            .resolve(resolver)
            .map_err(NominalSourceFieldResolutionError::ValueType)?;
        Ok(NominalSourceFieldV1 { field, value_type })
    }
}

impl WireEncode for DecodedNominalSourceFieldV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.field.encode(encoder)?;
        encoder.field(2)?;
        self.value_type.encode(encoder)
    }
}

impl WireDecode for DecodedNominalSourceFieldV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            field: decoder.field(1, DecodedPersistentId::decode)?,
            value_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
        })
    }
}
