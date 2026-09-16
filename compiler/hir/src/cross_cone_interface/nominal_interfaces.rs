use scoop_identity::{
    CallableTemplateOrigin, PersistentConstructorId, PersistentExportBindingId,
    PersistentIdResolver,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    CanonicalBinderListV1, CanonicalPersistentIdSetValidationError, CanonicalPersistentIdsV1,
    CanonicalPublicMemberRefsV1, CanonicalSignatureTypesV1, DecodedCanonicalBinderListV1,
    DecodedCanonicalPersistentIdsV1, DecodedCanonicalPublicMemberRefsV1,
    DecodedCanonicalSignatureTypesV1, DecodedNominalSourceShapeV1, DecodedSourceNominalId,
    NominalSourceShapeResolutionError, NominalSourceShapeResolver, NominalSourceShapeV1,
    PublicMemberRefResolver, PublicMemberRefSetValidationError, PublicMemberRefV1,
    PublicNominalKindV1, SignatureTypeSetValidationError, SourceNominalId, SourceNominalIdResolver,
};
use crate::BinderListValidationError;

mod errors;

pub use errors::{NominalInterfaceRecordBuildError, NominalInterfaceRecordResolutionError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalInterfaceRecordV1 {
    declaration: SourceNominalId,
    kind: PublicNominalKindV1,
    type_parameters: CanonicalBinderListV1,
    exact_supertypes: CanonicalSignatureTypesV1,
    constructors: CanonicalPersistentIdsV1<PersistentConstructorId>,
    members: CanonicalPublicMemberRefsV1,
    nested_bindings: CanonicalPersistentIdsV1<PersistentExportBindingId>,
    source_shape: NominalSourceShapeV1,
}

impl NominalInterfaceRecordV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        declaration: SourceNominalId,
        kind: PublicNominalKindV1,
        type_parameters: CanonicalBinderListV1,
        exact_supertypes: CanonicalSignatureTypesV1,
        constructors: CanonicalPersistentIdsV1<PersistentConstructorId>,
        members: CanonicalPublicMemberRefsV1,
        nested_bindings: CanonicalPersistentIdsV1<PersistentExportBindingId>,
        source_shape: NominalSourceShapeV1,
    ) -> Result<Self, NominalInterfaceRecordBuildError> {
        if source_shape.kind() != kind {
            return Err(NominalInterfaceRecordBuildError::SourceShapeKind {
                expected: kind,
                actual: source_shape.kind(),
            });
        }
        validate_member_partition(&members)?;
        Ok(Self {
            declaration,
            kind,
            type_parameters,
            exact_supertypes,
            constructors,
            members,
            nested_bindings,
            source_shape,
        })
    }

    pub const fn declaration(&self) -> SourceNominalId {
        self.declaration
    }

    pub const fn kind(&self) -> PublicNominalKindV1 {
        self.kind
    }

    pub const fn type_parameters(&self) -> &CanonicalBinderListV1 {
        &self.type_parameters
    }

    pub const fn exact_supertypes(&self) -> &CanonicalSignatureTypesV1 {
        &self.exact_supertypes
    }

    pub const fn constructors(&self) -> &CanonicalPersistentIdsV1<PersistentConstructorId> {
        &self.constructors
    }

    pub const fn members(&self) -> &CanonicalPublicMemberRefsV1 {
        &self.members
    }

    pub const fn nested_bindings(&self) -> &CanonicalPersistentIdsV1<PersistentExportBindingId> {
        &self.nested_bindings
    }

    pub const fn source_shape(&self) -> &NominalSourceShapeV1 {
        &self.source_shape
    }
}

impl WireEncode for NominalInterfaceRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
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
        self.source_shape.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNominalInterfaceRecordV1 {
    declaration: DecodedSourceNominalId,
    kind: PublicNominalKindV1,
    type_parameters: DecodedCanonicalBinderListV1,
    exact_supertypes: DecodedCanonicalSignatureTypesV1,
    constructors: DecodedCanonicalPersistentIdsV1<PersistentConstructorId>,
    members: DecodedCanonicalPublicMemberRefsV1,
    nested_bindings: DecodedCanonicalPersistentIdsV1<PersistentExportBindingId>,
    source_shape: DecodedNominalSourceShapeV1,
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
        NominalInterfaceRecordV1::try_new(
            declaration,
            self.kind,
            type_parameters,
            exact_supertypes,
            constructors,
            members,
            nested_bindings,
            source_shape,
        )
        .map_err(NominalInterfaceRecordResolutionError::Record)
    }
}

impl WireEncode for DecodedNominalInterfaceRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
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
        self.source_shape.encode(encoder)
    }
}

impl WireDecode for DecodedNominalInterfaceRecordV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(8)?;
        Ok(Self {
            declaration: decoder.field(1, DecodedSourceNominalId::decode)?,
            kind: decoder.field(2, PublicNominalKindV1::decode)?,
            type_parameters: decoder.field(3, DecodedCanonicalBinderListV1::decode)?,
            exact_supertypes: decoder.field(4, DecodedCanonicalSignatureTypesV1::decode)?,
            constructors: decoder.field(5, DecodedCanonicalPersistentIdsV1::decode)?,
            members: decoder.field(6, DecodedCanonicalPublicMemberRefsV1::decode)?,
            nested_bindings: decoder.field(7, DecodedCanonicalPersistentIdsV1::decode)?,
            source_shape: decoder.field(8, DecodedNominalSourceShapeV1::decode)?,
        })
    }
}

pub trait NominalInterfaceRecordResolver<E>:
    SourceNominalIdResolver<E>
    + PublicMemberRefResolver<E>
    + NominalSourceShapeResolver<E>
    + PersistentIdResolver<PersistentExportBindingId, Error = E>
{
}

impl<R, E> NominalInterfaceRecordResolver<E> for R where
    R: SourceNominalIdResolver<E>
        + PublicMemberRefResolver<E>
        + NominalSourceShapeResolver<E>
        + PersistentIdResolver<PersistentExportBindingId, Error = E>
{
}

fn validate_member_partition(
    members: &CanonicalPublicMemberRefsV1,
) -> Result<(), NominalInterfaceRecordBuildError> {
    for member in members.members() {
        match member {
            PublicMemberRefV1::Callable(CallableTemplateOrigin::Constructor(constructor)) => {
                return Err(NominalInterfaceRecordBuildError::ConstructorMember(
                    *constructor,
                ));
            }
            PublicMemberRefV1::Callable(CallableTemplateOrigin::VariantConstructor(variant)) => {
                return Err(NominalInterfaceRecordBuildError::VariantConstructorMember(
                    *variant,
                ));
            }
            PublicMemberRefV1::Callable(
                CallableTemplateOrigin::Function(_)
                | CallableTemplateOrigin::GenericFunction(_)
                | CallableTemplateOrigin::Accessor(_),
            )
            | PublicMemberRefV1::Property(_) => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
