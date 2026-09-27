use super::*;
use scoop_identity::{
    DefinitionOwnerAtom, FieldIdentityView, GeneratedNominalKey, InitializationUnitKey,
    NominalDeclarationOwner, PersistentEnumVariantFieldId, PersistentFieldId,
    PersistentInitializationUnitId, PersistentTypeId, SourceDeclarationKind,
};

impl Input<'_> {
    pub(super) fn field(&mut self, field: PersistentFieldId) -> Result<(), Error> {
        let at = HirDependencyTypePositionV1::FieldStorage(field);
        let key = self.key(&self.foundation.fields, field)?;
        match key.view() {
            FieldIdentityView::SourceDeclared { owner, .. }
            | FieldIdentityView::SourcePropertyBacking { owner, .. }
            | FieldIdentityView::SourcePropertyDelegate { owner, .. } => {
                self.nominal(owner, at)?;
            }
            FieldIdentityView::Generated { owner, key } => {
                let generated = self.key(&self.foundation.generated_types, owner)?;
                let GeneratedNominalKey::ObjectBackingClass { object } = generated else {
                    return Err(Error::StoragePosition(at));
                };
                self.nominal(NominalDeclarationOwner::Concrete(*object), at)?;
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
        self.nominal(owner, at)?;
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
        let key = self.key(&self.foundation.constructors, id)?;
        let Some(DefinitionOwnerAtom::Type(owner)) = key.owners().owners().last() else {
            return Err(Error::StoragePosition(at));
        };
        let owner = self.nominal(NominalDeclarationOwner::Concrete(*owner), at)?;
        if !matches!(
            owner.declaration_kind(),
            SourceDeclarationKind::Class | SourceDeclarationKind::Object
        ) {
            return Err(Error::StoragePosition(at));
        }
        self.origin(DefinitionOriginSubject::Constructor(id))
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
                let key = self.nominal(NominalDeclarationOwner::Concrete(*owner), at)?;
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

    fn nominal(
        &mut self,
        owner: NominalDeclarationOwner,
        at: HirDependencyTypePositionV1,
    ) -> Result<&SourceDeclarationKey, Error> {
        let NominalDeclarationOwner::Concrete(owner) = owner else {
            return Err(Error::StoragePosition(at));
        };
        let key = self.key::<PersistentTypeId, _>(&self.foundation.types, owner)?;
        self.source(key)?;
        Ok(key)
    }
}
