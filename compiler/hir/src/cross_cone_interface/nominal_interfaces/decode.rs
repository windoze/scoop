use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNominalInterfaceRecordV1 {
    pub(super) declaration: DecodedSourceNominalId,
    pub(super) kind: PublicNominalKindV1,
    pub(super) type_parameters: DecodedCanonicalBinderListV1,
    pub(super) exact_supertypes: DecodedCanonicalSignatureTypesV1,
    pub(super) constructors: DecodedCanonicalPersistentIdsV1<PersistentConstructorId>,
    pub(super) members: DecodedCanonicalPublicMemberRefsV1,
    pub(super) nested_bindings: DecodedCanonicalPersistentIdsV1<PersistentExportBindingId>,
    pub(super) source_shape: DecodedNominalSourceShapeV1,
    pub(super) details: DecodedNominalDeclarationDetailsV1,
}

impl DecodedNominalInterfaceRecordV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<NominalInterfaceRecordV1, NominalInterfaceRecordResolutionError<E>>
    where
        R: NominalInterfaceRecordResolver<E>,
    {
        let declaration = self
            .declaration
            .resolve(resolver)
            .map_err(NominalInterfaceRecordResolutionError::Declaration)?;
        let type_parameters = self
            .type_parameters
            .resolve(resolver)
            .map_err(NominalInterfaceRecordResolutionError::TypeParameters)?;
        let exact_supertypes = self
            .exact_supertypes
            .resolve(resolver)
            .map_err(NominalInterfaceRecordResolutionError::ExactSupertypes)?;
        let constructors = self
            .constructors
            .resolve(resolver)
            .map_err(NominalInterfaceRecordResolutionError::Constructors)?;
        let members = self
            .members
            .resolve(resolver)
            .map_err(NominalInterfaceRecordResolutionError::Members)?;
        let nested_bindings = self
            .nested_bindings
            .resolve(resolver)
            .map_err(NominalInterfaceRecordResolutionError::NestedBindings)?;
        let source_shape = self
            .source_shape
            .resolve(resolver)
            .map_err(NominalInterfaceRecordResolutionError::SourceShape)?;
        let details = self
            .details
            .resolve(resolver)
            .map_err(NominalInterfaceRecordResolutionError::DeclarationDetails)?;
        NominalInterfaceRecordV1::try_new(
            declaration,
            self.kind,
            type_parameters,
            exact_supertypes,
            constructors,
            members,
            nested_bindings,
            source_shape,
            details,
        )
        .map_err(NominalInterfaceRecordResolutionError::Record)
    }
}

impl WireEncode for DecodedNominalInterfaceRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.kind.encode(encoder)?;
        encoder.field(3)?;
        self.type_parameters.encode(encoder)?;
        encoder.field(4)?;
        self.exact_supertypes.encode(encoder)?;
        encoder.field(5)?;
        self.constructors.encode(encoder)?;
        encoder.field(6)?;
        self.members.encode(encoder)?;
        encoder.field(7)?;
        self.nested_bindings.encode(encoder)?;
        encoder.field(8)?;
        self.source_shape.encode(encoder)?;
        encoder.field(9)?;
        self.details.encode(encoder)
    }
}

impl WireDecode for DecodedNominalInterfaceRecordV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(9)?;
        Ok(Self {
            declaration: decoder.field(1, DecodedSourceNominalId::decode)?,
            kind: decoder.field(2, PublicNominalKindV1::decode)?,
            type_parameters: decoder.field(3, DecodedCanonicalBinderListV1::decode)?,
            exact_supertypes: decoder.field(4, DecodedCanonicalSignatureTypesV1::decode)?,
            constructors: decoder.field(5, DecodedCanonicalPersistentIdsV1::decode)?,
            members: decoder.field(6, DecodedCanonicalPublicMemberRefsV1::decode)?,
            nested_bindings: decoder.field(7, DecodedCanonicalPersistentIdsV1::decode)?,
            source_shape: decoder.field(8, DecodedNominalSourceShapeV1::decode)?,
            details: decoder.field(9, DecodedNominalDeclarationDetailsV1::decode)?,
        })
    }
}

pub trait NominalInterfaceRecordResolver<E>:
    SourceNominalIdResolver<E>
    + PublicMemberRefResolver<E>
    + NominalSourceShapeResolver<E>
    + PersistentIdResolver<PersistentExportBindingId, Error = E>
    + PersistentIdResolver<scoop_identity::PersistentDispatchSlotId, Error = E>
{
}

impl<R, E> NominalInterfaceRecordResolver<E> for R where
    R: SourceNominalIdResolver<E>
        + PublicMemberRefResolver<E>
        + NominalSourceShapeResolver<E>
        + PersistentIdResolver<PersistentExportBindingId, Error = E>
        + PersistentIdResolver<scoop_identity::PersistentDispatchSlotId, Error = E>
{
}
