use super::*;
use scoop_identity::{
    DefinitionOwnerAtom, FieldIdentityView, GeneratedNominalKey, InitializationUnitKey,
    NominalDeclarationOwner, PersistentEnumVariantFieldId, PersistentFieldId,
    PersistentInitializationUnitId, SourceDeclarationKind,
};

impl Input<'_> {
    pub(super) fn field(&mut self, field: PersistentFieldId) -> Result<(), Error> {
        let at = HirDependencyTypePositionV1::FieldStorage(field);
        let key = self.key(&self.foundation.fields, field)?;
        match key.view() {
            FieldIdentityView::SourceDeclared { owner, .. }
            | FieldIdentityView::SourcePropertyBacking { owner, .. }
            | FieldIdentityView::SourcePropertyDelegate { owner, .. } => {
                self.nominal(owner)?;
            }
            FieldIdentityView::Generated { owner, key } => {
                let generated = self.key(&self.foundation.generated_types, owner)?;
                let GeneratedNominalKey::ObjectBackingClass { object } = generated else {
                    return Err(Error::StoragePosition(at));
                };
                self.nominal(NominalDeclarationOwner::Concrete(*object))?;
                let property = key
                    .object_backing_property()
                    .ok_or(Error::StoragePosition(at))?;
                self.property(PropertyOwner::Property(property))?;
            }
        }
        self.origin(DefinitionOriginSubject::Field(field))
    }

    pub(super) fn variant_field(
        &mut self,
        field: PersistentEnumVariantFieldId,
    ) -> Result<(), Error> {
        let at = HirDependencyTypePositionV1::EnumVariantFieldStorage(field);
        let foundation = self.foundation;
        let key = self.key(&foundation.enum_variant_fields, field)?;
        let variant = self.key(&foundation.enum_variants, key.variant())?;
        let owner = variant.source_owner().ok_or(Error::StoragePosition(at))?;
        self.nominal(owner)?;
        self.origin(DefinitionOriginSubject::EnumVariantField(field))
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
                return Err(Error::StoragePosition(at));
            }
        }
        self.origin(DefinitionOriginSubject::InitializationUnit(unit))
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
