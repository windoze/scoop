use super::*;
use crate::{DeclaredVisibilityV1, NominalInheritanceModalityV1};

mod conditions;
mod decode;
pub use conditions::{DecodedNominalInstantiationConditionsV1, NominalInstantiationConditionsV1};
mod inventory;
mod references;
pub use decode::{DecodedNominalDeclarationDetailsV1, NominalDeclarationDetailsResolutionError};
pub use inventory::NominalDeclarationInventoryError;
pub use references::*;

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
    dispatch_selections: CanonicalNominalDispatchSelectionsV1,
    primary_value_constructor: Option<PersistentConstructorId>,
    instantiation_conditions: NominalInstantiationConditionsV1,
}

impl NominalDeclarationDetailsV1 {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        modality: NominalInheritanceModalityV1,
        visibility: DeclaredVisibilityV1,
        constructors: CanonicalPersistentIdsV1<PersistentConstructorId>,
        members: CanonicalNestedMemberRefsV1,
        children: CanonicalNestedNominalRefsV1,
        dispatch_order: NominalDispatchOrderV1,
        dispatch_selections: CanonicalNominalDispatchSelectionsV1,
        primary_value_constructor: Option<PersistentConstructorId>,
        instantiation_conditions: NominalInstantiationConditionsV1,
    ) -> Self {
        Self {
            modality,
            visibility,
            constructors,
            members,
            children,
            dispatch_order,
            dispatch_selections,
            primary_value_constructor,
            instantiation_conditions,
        }
    }

    pub const fn instantiation_conditions(&self) -> &NominalInstantiationConditionsV1 {
        &self.instantiation_conditions
    }

    pub const fn primary_value_constructor(&self) -> Option<PersistentConstructorId> {
        self.primary_value_constructor
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

    pub const fn dispatch_selections(&self) -> &CanonicalNominalDispatchSelectionsV1 {
        &self.dispatch_selections
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
        if let Some(primary) = self.primary_value_constructor {
            if kind != PublicNominalKindV1::Struct {
                return Err(NominalInterfaceRecordBuildError::PrimaryValueConstructorKind(kind));
            }
            if !self.constructors.values().contains(&primary) {
                return Err(NominalInterfaceRecordBuildError::UndeclaredConstructor(
                    primary,
                ));
            }
        } else if kind == PublicNominalKindV1::Struct && !self.constructors.is_empty() {
            return Err(NominalInterfaceRecordBuildError::MissingPrimaryValueConstructor);
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
        encoder.map(9)?;
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
        self.dispatch_order.encode(encoder)?;
        encoder.field(7)?;
        self.dispatch_selections.encode(encoder)?;
        encoder.field(8)?;
        encoder.array(u64::from(self.primary_value_constructor.is_some()))?;
        if let Some(primary) = self.primary_value_constructor {
            primary.encode(encoder)?;
        }
        encoder.field(9)?;
        self.instantiation_conditions.encode(encoder)
    }
}

impl NominalInterfaceRecordV1 {
    pub const fn declaration_details(&self) -> &NominalDeclarationDetailsV1 {
        &self.details
    }
}
