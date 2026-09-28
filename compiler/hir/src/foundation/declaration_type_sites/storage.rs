use super::*;
use scoop_identity::{
    DefinitionOwnerAtom, ExactTypeKey, FieldIdentityView, GeneratedNominalKey,
    InitializationUnitKey, NominalDeclarationOwner, PersistentEnumVariantFieldId,
    PersistentExactTypeId, PersistentFieldId, PersistentInitializationUnitId,
    SourceDeclarationKind,
};

impl Input<'_> {
    pub(super) fn field(
        &mut self,
        exact_owner: PersistentExactTypeId,
        field: PersistentFieldId,
    ) -> Result<(), Error> {
        let at = HirDependencyTypePositionV1::FieldStorage(exact_owner, field);
        let (foundation, key) = self.source_record(field, |foundation| &foundation.fields)?;
        match key.view() {
            FieldIdentityView::SourceDeclared { owner, .. }
            | FieldIdentityView::SourcePropertyBacking { owner, .. }
            | FieldIdentityView::SourcePropertyDelegate { owner, .. } => {
                self.storage_owner(exact_owner, owner, at)?;
            }
            FieldIdentityView::Generated { owner, key } => {
                let generated = self.key(&foundation.generated_types, owner)?;
                let GeneratedNominalKey::ObjectBackingClass { object } = generated else {
                    return Err(Error::StoragePosition(at));
                };
                if self.key(&self.foundation.exact_types, exact_owner)?
                    != &ExactTypeKey::Nominal(*object)
                {
                    return Err(Error::StoragePosition(at));
                }
                self.nominal(NominalDeclarationOwner::Concrete(*object))?;
                let property = key
                    .object_backing_property()
                    .ok_or(Error::StoragePosition(at))?;
                self.property(PropertyOwner::Property(property))?;
            }
        }
        Self::source_origin(foundation, DefinitionOriginSubject::Field(field))
    }

    pub(super) fn variant_field(
        &mut self,
        exact_owner: PersistentExactTypeId,
        field: PersistentEnumVariantFieldId,
    ) -> Result<(), Error> {
        let at = HirDependencyTypePositionV1::EnumVariantFieldStorage(exact_owner, field);
        let (foundation, key) =
            self.source_record(field, |foundation| &foundation.enum_variant_fields)?;
        let variant = self.key(&foundation.enum_variants, key.variant())?;
        let owner = variant.source_owner().ok_or(Error::StoragePosition(at))?;
        self.storage_owner(exact_owner, owner, at)?;
        Self::source_origin(foundation, DefinitionOriginSubject::EnumVariantField(field))
    }

    fn storage_owner(
        &mut self,
        exact: PersistentExactTypeId,
        expected: NominalDeclarationOwner,
        at: HirDependencyTypePositionV1,
    ) -> Result<(), Error> {
        match (self.key(&self.foundation.exact_types, exact)?, expected) {
            (ExactTypeKey::Nominal(actual), NominalDeclarationOwner::Concrete(expected))
                if *actual == expected =>
            {
                self.nominal(NominalDeclarationOwner::Concrete(expected))?;
            }
            (
                ExactTypeKey::NominalApplication { origin, .. },
                NominalDeclarationOwner::GenericTemplate(expected),
            ) if *origin == expected => {
                self.source_record(expected, |foundation| &foundation.generic_types)?;
            }
            _ => return Err(Error::StoragePosition(at)),
        }
        Ok(())
    }

    pub(super) fn constructor_initializer(
        &mut self,
        root: CallableMaterialization,
    ) -> Result<(), Error> {
        self.root(root, None)?;
        let at = HirDependencyTypePositionV1::ConstructorInitializerResult(root);
        let CallableTemplateOwner::Constructor(id) = root.template() else {
            return Err(Error::StoragePosition(at));
        };
        let (foundation, key) = self.source_record(id, |foundation| &foundation.constructors)?;
        let owner = match key.owners().owners().last() {
            Some(DefinitionOwnerAtom::Type(owner)) => self.key(&foundation.types, *owner)?,
            Some(DefinitionOwnerAtom::GenericType(owner)) => {
                self.key(&foundation.generic_types, *owner)?
            }
            _ => return Err(Error::StoragePosition(at)),
        };
        if !matches!(
            owner.declaration_kind(),
            SourceDeclarationKind::Class | SourceDeclarationKind::Object
        ) {
            return Err(Error::StoragePosition(at));
        }
        let subject = DefinitionOriginSubject::Constructor(id);
        let origin = foundation
            .definition_origin(subject)
            .ok_or(Error::MissingOrigin(subject))?;
        foundation
            .validate_definition_origin_location(origin.origin().source().cone(), origin.origin())
            .map_err(|source| Error::Origin(Box::new(source)))
    }

    pub(super) fn initialization(
        &mut self,
        unit: PersistentInitializationUnitId,
    ) -> Result<(), Error> {
        let at = HirDependencyTypePositionV1::InitializationCycleMessage(unit);
        let key = self.key(&self.foundation.initialization_units, unit)?;
        match key {
            InitializationUnitKey::TopLevelProperty(property) => {
                self.property(PropertyOwner::Property(*property))?
            }
            InitializationUnitKey::ExtensionProperty(property) => {
                self.property(PropertyOwner::ExtensionProperty(*property))?
            }
            InitializationUnitKey::Object(owner) | InitializationUnitKey::Companion(owner) => {
                let key = self.nominal(NominalDeclarationOwner::Concrete(*owner))?;
                if key.declaration_kind() != SourceDeclarationKind::Object {
                    return Err(Error::StoragePosition(at));
                }
            }
            InitializationUnitKey::GenericDelegatedExtensionApplication { .. } => {
                return self.generic_delegate(unit);
            }
        }
        self.origin(DefinitionOriginSubject::InitializationUnit(unit))
    }

    pub(super) fn generic_delegate(
        &mut self,
        unit: PersistentInitializationUnitId,
    ) -> Result<(), Error> {
        let at = HirDependencyTypePositionV1::GenericDelegateStorage(unit);
        let InitializationUnitKey::GenericDelegatedExtensionApplication { property, .. } =
            self.key(&self.foundation.initialization_units, unit)?
        else {
            return Err(Error::StoragePosition(at));
        };
        let (foundation, _) =
            self.source_record(*property, |foundation| &foundation.extension_properties)?;
        let declaration = foundation
            .initialization_units
            .iter()
            .find(|record| record.key() == &InitializationUnitKey::ExtensionProperty(*property))
            .ok_or(Error::StoragePosition(at))?;
        Self::source_origin(
            foundation,
            DefinitionOriginSubject::InitializationUnit(declaration.id()),
        )
    }

    fn nominal(&mut self, owner: NominalDeclarationOwner) -> Result<&SourceDeclarationKey, Error> {
        let key = match owner {
            NominalDeclarationOwner::Concrete(owner) => self.key(&self.foundation.types, owner)?,
            NominalDeclarationOwner::GenericTemplate(owner) => {
                self.key(&self.foundation.generic_types, owner)?
            }
        };
        self.source(key)?;
        Ok(key)
    }
}
