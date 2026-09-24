use super::*;
use crate::{
    CanonicalNestedMemberRefsV1, CanonicalNestedNominalRefsV1, DeclaredVisibilityV1,
    NestedSourceMemberRefV1, NominalInheritanceModalityV1,
};

mod decode;
mod inventory;
pub use decode::{DecodedNominalDeclarationDetailsV1, NominalDeclarationDetailsResolutionError};
pub use inventory::NominalDeclarationInventoryError;

/// Declaration relationships shared by public lookup and representation queries.
/// Kind, binders, supertypes and storage remain in the enclosing nominal record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalDeclarationDetailsV1 {
    modality: NominalInheritanceModalityV1,
    visibility: DeclaredVisibilityV1,
    constructors: CanonicalPersistentIdsV1<PersistentConstructorId>,
    members: CanonicalNestedMemberRefsV1,
    children: CanonicalNestedNominalRefsV1,
    dispatch_order: NominalDispatchOrderV1,
}

impl NominalDeclarationDetailsV1 {
    pub const fn new(
        modality: NominalInheritanceModalityV1,
        visibility: DeclaredVisibilityV1,
        constructors: CanonicalPersistentIdsV1<PersistentConstructorId>,
        members: CanonicalNestedMemberRefsV1,
        children: CanonicalNestedNominalRefsV1,
        dispatch_order: NominalDispatchOrderV1,
    ) -> Self {
        Self {
            modality,
            visibility,
            constructors,
            members,
            children,
            dispatch_order,
        }
    }

    pub const fn modality(&self) -> NominalInheritanceModalityV1 {
        self.modality
    }
    pub const fn declared_visibility(&self) -> DeclaredVisibilityV1 {
        self.visibility
    }
    pub const fn constructors(&self) -> &CanonicalPersistentIdsV1<PersistentConstructorId> {
        &self.constructors
    }
    pub const fn members(&self) -> &CanonicalNestedMemberRefsV1 {
        &self.members
    }
    pub const fn children(&self) -> &CanonicalNestedNominalRefsV1 {
        &self.children
    }
    pub const fn dispatch_order(&self) -> &NominalDispatchOrderV1 {
        &self.dispatch_order
    }

    pub(super) fn validate(
        &self,
        kind: PublicNominalKindV1,
        constructors: &CanonicalPersistentIdsV1<PersistentConstructorId>,
        members: &CanonicalPublicMemberRefsV1,
    ) -> Result<(), NominalInterfaceRecordBuildError> {
        use NominalInheritanceModalityV1 as Modality;
        let valid = match kind {
            PublicNominalKindV1::Class => self.modality != Modality::Interface,
            PublicNominalKindV1::Interface => self.modality == Modality::Interface,
            PublicNominalKindV1::Struct
            | PublicNominalKindV1::Enum
            | PublicNominalKindV1::Object => self.modality == Modality::Final,
        };
        if !valid {
            return Err(NominalInterfaceRecordBuildError::Modality {
                kind,
                modality: self.modality,
            });
        }
        if !self.constructors.is_empty()
            && !matches!(
                kind,
                PublicNominalKindV1::Class | PublicNominalKindV1::Struct
            )
        {
            return Err(NominalInterfaceRecordBuildError::ConstructorsNotAllowed(
                kind,
            ));
        }
        for constructor in constructors.values() {
            if self
                .constructors
                .values()
                .binary_search(constructor)
                .is_err()
            {
                return Err(NominalInterfaceRecordBuildError::UndeclaredConstructor(
                    *constructor,
                ));
            }
        }
        for member in members.members() {
            let declared = match *member {
                PublicMemberRefV1::Callable(CallableTemplateOrigin::Function(id)) => {
                    NestedSourceMemberRefV1::Function(id)
                }
                PublicMemberRefV1::Callable(CallableTemplateOrigin::GenericFunction(id)) => {
                    NestedSourceMemberRefV1::GenericFunction(id)
                }
                PublicMemberRefV1::Property(scoop_identity::PropertyOwner::Property(id)) => {
                    NestedSourceMemberRefV1::Property(id)
                }
                // The semantic pass joins accessors to their logical property.
                PublicMemberRefV1::Callable(CallableTemplateOrigin::Accessor(_)) => continue,
                _ => return Err(NominalInterfaceRecordBuildError::UndeclaredMember(*member)),
            };
            if !self.members.values().contains(&declared) {
                return Err(NominalInterfaceRecordBuildError::UndeclaredMember(*member));
            }
        }
        Ok(())
    }
}

impl WireEncode for NominalDeclarationDetailsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.modality.encode(encoder)?;
        encoder.field(2)?;
        self.visibility.encode(encoder)?;
        encoder.field(3)?;
        self.constructors.encode(encoder)?;
        encoder.field(4)?;
        self.members.encode(encoder)?;
        encoder.field(5)?;
        self.children.encode(encoder)?;
        encoder.field(6)?;
        self.dispatch_order.encode(encoder)
    }
}

impl NominalInterfaceRecordV1 {
    pub const fn declaration_details(&self) -> &NominalDeclarationDetailsV1 {
        &self.details
    }

    pub fn source_contract(
        &self,
    ) -> Result<crate::NominalSourceContractV1, crate::SourceInventoryError> {
        crate::NominalSourceContractV1::try_new(
            self.declaration,
            self.details.modality,
            self.type_parameters.clone(),
            self.exact_supertypes.clone(),
            self.details.constructors.clone(),
            self.details.members.clone(),
            self.details.children.clone(),
            self.source_shape.clone(),
        )
    }

    pub(crate) fn from_source_contract(
        source: crate::NominalSourceContractV1,
        visibility: DeclaredVisibilityV1,
        dispatch_order: NominalDispatchOrderV1,
    ) -> Result<Self, NominalInterfaceRecordBuildError> {
        let details = NominalDeclarationDetailsV1::new(
            source.modality(),
            visibility,
            source.constructors().clone(),
            source.members().clone(),
            source.children().clone(),
            dispatch_order,
        );
        Self::try_new(
            source.owner(),
            source.kind(),
            source.type_parameters().clone(),
            source.supertypes().clone(),
            CanonicalPersistentIdsV1::empty(),
            CanonicalPublicMemberRefsV1::default(),
            CanonicalPersistentIdsV1::empty(),
            source.source_shape().clone(),
            details,
        )
    }
}
