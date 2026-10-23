use super::*;
use crate::{DeclaredVisibilityV1, NominalInheritanceModalityV1};

mod class_primary;
pub use class_primary::*;
mod conditions;
mod decode;
pub use conditions::{DecodedNominalInstantiationConditionsV1, NominalInstantiationConditionsV1};
mod inventory;
mod references;
mod release;
mod sealed;
pub use decode::{DecodedNominalDeclarationDetailsV1, NominalDeclarationDetailsResolutionError};
pub use inventory::NominalDeclarationInventoryError;
pub use references::*;
pub use release::NominalReleasePolicyV1;

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
    release_policy: NominalReleasePolicyV1,
    class_primary_constructor: Option<ClassPrimaryConstructorV1>,
    sealed_subtypes: Option<crate::CanonicalSourceNominalIdsV1>,
    conditional_interfaces: Vec<ConditionalInterfaceRecordV1>,
    conditional_conformances: Vec<ConditionalInterfaceConformanceV1>,
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
        release_policy: NominalReleasePolicyV1,
        class_primary_constructor: Option<ClassPrimaryConstructorV1>,
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
            release_policy,
            class_primary_constructor,
            sealed_subtypes: None,
            conditional_interfaces: Vec::new(),
            conditional_conformances: Vec::new(),
        }
    }

    pub fn with_sealed_subtypes(
        mut self,
        subtypes: Option<crate::CanonicalSourceNominalIdsV1>,
    ) -> Self {
        self.sealed_subtypes = subtypes;
        self
    }

    pub fn sealed_subtypes(&self) -> Option<&crate::CanonicalSourceNominalIdsV1> {
        self.sealed_subtypes.as_ref()
    }

    pub fn with_conditional_interfaces(
        mut self,
        implementations: Vec<ConditionalInterfaceRecordV1>,
    ) -> Self {
        self.conditional_interfaces = implementations;
        self
    }

    pub fn conditional_interfaces(&self) -> &[ConditionalInterfaceRecordV1] {
        &self.conditional_interfaces
    }

    pub fn with_conditional_conformances(
        mut self,
        values: Vec<ConditionalInterfaceConformanceV1>,
    ) -> Self {
        self.conditional_conformances = values;
        self
    }

    pub fn conditional_conformances(&self) -> &[ConditionalInterfaceConformanceV1] {
        &self.conditional_conformances
    }

    pub fn inheritance_family(&self) -> crate::InheritanceFamily {
        match &self.sealed_subtypes {
            None => crate::InheritanceFamily::Open,
            Some(subtypes) => crate::InheritanceFamily::Sealed(subtypes.values().to_vec()),
        }
    }

    pub const fn instantiation_conditions(&self) -> &NominalInstantiationConditionsV1 {
        &self.instantiation_conditions
    }

    pub const fn release_policy(&self) -> &NominalReleasePolicyV1 {
        &self.release_policy
    }

    pub fn class_primary_constructor(&self) -> Option<&ClassPrimaryConstructorV1> {
        self.class_primary_constructor.as_ref()
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
        if !valid
            || (self.sealed_subtypes.is_some()
                && !matches!(
                    (kind, self.modality),
                    (PublicNominalKindV1::Class, Modality::Abstract)
                        | (PublicNominalKindV1::Interface, Modality::Interface)
                ))
        {
            return Err(NominalInterfaceRecordBuildError::Modality {
                kind,
                modality: self.modality,
            });
        }
        if !self.constructors.is_empty()
            && !matches!(
                kind,
                PublicNominalKindV1::Class
                    | PublicNominalKindV1::Struct
                    | PublicNominalKindV1::Object
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
        if let Some(primary) = &self.class_primary_constructor {
            if kind != PublicNominalKindV1::Class {
                return Err(NominalInterfaceRecordBuildError::PrimaryClassConstructorKind(kind));
            }
            if !self.constructors.values().contains(&primary.constructor()) {
                return Err(NominalInterfaceRecordBuildError::UndeclaredConstructor(
                    primary.constructor(),
                ));
            }
            for &property in primary.properties().iter().flatten() {
                if !self
                    .members
                    .values()
                    .contains(&NestedSourceMemberRefV1::Property(property))
                {
                    return Err(NominalInterfaceRecordBuildError::UndeclaredMember(
                        PublicMemberRefV1::Property(scoop_identity::PropertyOwner::Property(
                            property,
                        )),
                    ));
                }
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
        encoder.map(
            11 + u64::from(self.sealed_subtypes.is_some())
                + u64::from(!self.conditional_interfaces.is_empty())
                + u64::from(!self.conditional_conformances.is_empty()),
        )?;
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
        self.instantiation_conditions.encode(encoder)?;
        encoder.field(10)?;
        self.release_policy.encode(encoder)?;
        encoder.field(11)?;
        encoder.array(u64::from(self.class_primary_constructor.is_some()))?;
        if let Some(primary) = &self.class_primary_constructor {
            primary.encode(encoder)?;
        }
        if let Some(subtypes) = &self.sealed_subtypes {
            encoder.field(12)?;
            encoder.array(subtypes.values().len() as u64)?;
            for subtype in subtypes.values() {
                subtype.encode(encoder)?;
            }
        }
        if !self.conditional_interfaces.is_empty() {
            encoder.field(13)?;
            encoder.array(self.conditional_interfaces.len() as u64)?;
            for implementation in &self.conditional_interfaces {
                implementation.encode(encoder)?;
            }
        }
        if !self.conditional_conformances.is_empty() {
            encoder.field(14)?;
            encoder.array(self.conditional_conformances.len() as u64)?;
            for conformance in &self.conditional_conformances {
                conformance.encode(encoder)?;
            }
        }
        Ok(())
    }
}

impl NominalInterfaceRecordV1 {
    pub const fn declaration_details(&self) -> &NominalDeclarationDetailsV1 {
        &self.details
    }
}
