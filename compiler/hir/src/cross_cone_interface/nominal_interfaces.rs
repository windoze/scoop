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

mod declaration;
mod dispatch;
mod errors;
mod field_inventory;
mod semantics;
mod table;

pub use declaration::{
    DecodedNominalDeclarationDetailsV1, NominalDeclarationDetailsResolutionError,
    NominalDeclarationDetailsV1, NominalDeclarationInventoryError,
};
pub use dispatch::*;
pub use errors::{NominalInterfaceRecordBuildError, NominalInterfaceRecordResolutionError};
pub use field_inventory::NominalSourceFieldInventoryError;
pub use semantics::{
    ExactSupertypeSemanticError, NominalInterfaceSemanticAuthority,
    NominalInterfaceSemanticValidationError,
};
pub use table::{
    CanonicalNominalInterfacesV1, DecodedCanonicalNominalInterfacesV1,
    NominalInterfaceSetBuildError, NominalInterfaceSetSemanticValidationError,
    NominalInterfaceSetValidationError,
};

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
    details: NominalDeclarationDetailsV1,
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
        details: NominalDeclarationDetailsV1,
    ) -> Result<Self, NominalInterfaceRecordBuildError> {
        if source_shape.kind() != kind {
            return Err(NominalInterfaceRecordBuildError::SourceShapeKind {
                expected: kind,
                actual: source_shape.kind(),
            });
        }
        if let NominalSourceShapeV1::Intrinsic(representation) = source_shape {
            representation
                .validate_binders(&type_parameters)
                .map_err(NominalInterfaceRecordBuildError::IntrinsicBinders)?;
        }
        if !constructors.is_empty()
            && !matches!(
                kind,
                PublicNominalKindV1::Class | PublicNominalKindV1::Struct
            )
        {
            return Err(NominalInterfaceRecordBuildError::ConstructorsNotAllowed(
                kind,
            ));
        }
        validate_member_partition(&members)?;
        details.validate(kind, &constructors, &members)?;
        details
            .dispatch_order()
            .validate_kind(kind)
            .map_err(NominalInterfaceRecordBuildError::DispatchOrder)?;
        Ok(Self {
            declaration,
            kind,
            type_parameters,
            exact_supertypes,
            constructors,
            members,
            nested_bindings,
            source_shape,
            details,
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

mod decode;
pub use decode::{DecodedNominalInterfaceRecordV1, NominalInterfaceRecordResolver};

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
