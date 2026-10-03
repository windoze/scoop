//! Declaration type sites bind to this artifact's existing typed identities.

use super::CanonicalHirFoundation;
use crate::{HirCallableTypePositionV1, HirDependencyTypePositionV1};
use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableTemplateOrigin,
    CallableTemplateOwner, CborIdentityRecord, ConeIdentity, DefinitionOriginSubject, PersistentId,
    PropertyOwner, SourceDeclarationKey,
};

mod errors;
mod signature;
mod storage;
pub use errors::DeclarationTypeSiteValidationError;
type Error = DeclarationTypeSiteValidationError;

impl CanonicalHirFoundation {
    pub fn validate_declaration_type_position(
        &self,
        current: ConeIdentity,
        position: HirDependencyTypePositionV1,
        dependencies: &[&CanonicalHirFoundation],
    ) -> Result<(), Error> {
        let mut input = Input {
            foundation: self,
            current,
            dependencies,
        };
        match position {
            HirDependencyTypePositionV1::Expression(..) => Err(Error::ExpressionPosition),
            HirDependencyTypePositionV1::CallableSignature(root, part) => {
                input.root(root, Some(part))
            }
            HirDependencyTypePositionV1::LocalValue(local) => {
                let key = input.key(&self.local_values, local)?;
                input.root(key.owner(), None)?;
                input.origin(DefinitionOriginSubject::LocalValue(local))
            }
            HirDependencyTypePositionV1::BackingStorage(property)
            | HirDependencyTypePositionV1::DelegateStorage(property) => input.property(property),
            HirDependencyTypePositionV1::FieldStorage(owner, field) => input.field(owner, field),
            HirDependencyTypePositionV1::EnumVariantFieldStorage(owner, field) => {
                input.variant_field(owner, field)
            }
            HirDependencyTypePositionV1::ConstructorInitializerResult(root) => {
                input.constructor_initializer(root)
            }
            HirDependencyTypePositionV1::InitializationCycleMessage(unit) => {
                input.initialization(unit)
            }
            HirDependencyTypePositionV1::GenericDelegateStorage(unit) => {
                input.generic_delegate(unit)
            }
        }
    }
}

struct Input<'a> {
    foundation: &'a crate::CanonicalHirFoundation,
    current: ConeIdentity,
    dependencies: &'a [&'a CanonicalHirFoundation],
}

impl<'a> Input<'a> {
    fn root(
        &mut self,
        root: CallableMaterialization,
        position: Option<HirCallableTypePositionV1>,
    ) -> Result<(), Error> {
        if let CallableTemplateOwner::Generated(id) = root.template() {
            match root.context() {
                CallableMaterializationContext::Application(application) => self
                    .key(&self.foundation.callable_applications, application)
                    .map(|_| ()),
                CallableMaterializationContext::InitializationApplication(unit) => {
                    self.generic_delegate(unit)
                }
                CallableMaterializationContext::NoSubstitution => Ok(()),
            }?;
            let (foundation, key) =
                self.source_record(id, |foundation| &foundation.generated_callables)?;
            signature::generated(key, root, position)?;
            let subject = DefinitionOriginSubject::GeneratedCallable(id);
            if foundation.definition_origin(subject).is_some() {
                Self::source_origin(foundation, subject)?;
            }
            return Ok(());
        }
        let applied_source = match root.template() {
            CallableTemplateOwner::GenericFunction(id) => {
                Some(CallableTemplateOrigin::GenericFunction(id))
            }
            CallableTemplateOwner::Function(id)
                if matches!(
                    root.context(),
                    CallableMaterializationContext::Application(_)
                ) =>
            {
                Some(CallableTemplateOrigin::Function(id))
            }
            CallableTemplateOwner::Constructor(id)
                if matches!(
                    root.context(),
                    CallableMaterializationContext::Application(_)
                ) =>
            {
                Some(CallableTemplateOrigin::Constructor(id))
            }
            CallableTemplateOwner::Accessor(id)
                if matches!(
                    root.context(),
                    CallableMaterializationContext::Application(_)
                ) =>
            {
                Some(CallableTemplateOrigin::Accessor(id))
            }
            _ => None,
        };
        if let Some(source) = applied_source {
            let CallableMaterializationContext::Application(application) = root.context() else {
                return Err(Error::Materialization(root));
            };
            let application = self.key(&self.foundation.callable_applications, application)?;
            if application.origin() != source {
                return Err(Error::Materialization(root));
            }
            return match source {
                CallableTemplateOrigin::GenericFunction(id) => {
                    let (_, key) =
                        self.source_record(id, |foundation| &foundation.generic_functions)?;
                    signature::source(key, root, position)
                }
                CallableTemplateOrigin::Function(id) => {
                    let (_, key) = self.source_record(id, |foundation| &foundation.functions)?;
                    signature::source(key, root, position)
                }
                CallableTemplateOrigin::Constructor(id) => {
                    let (_, key) = self.source_record(id, |foundation| &foundation.constructors)?;
                    signature::source(key, root, position)
                }
                CallableTemplateOrigin::Accessor(id) => {
                    let (foundation, key) =
                        self.source_record(id, |foundation| &foundation.property_accessors)?;
                    let property = match key.owner() {
                        PropertyOwner::Property(id) => self.key(&foundation.properties, id)?,
                        PropertyOwner::ExtensionProperty(id) => {
                            self.key(&foundation.extension_properties, id)?
                        }
                    };
                    signature::accessor(property, key.role(), root, position)
                }
                CallableTemplateOrigin::VariantConstructor(_) => Err(Error::Materialization(root)),
            };
        }
        if root.context() != CallableMaterializationContext::NoSubstitution {
            return Err(Error::Materialization(root));
        }
        match root.template() {
            CallableTemplateOwner::Function(id) => {
                let (foundation, key) =
                    self.source_record(id, |foundation| &foundation.functions)?;
                self.declaration_origin(foundation, key)?;
                signature::source(key, root, position)
            }
            CallableTemplateOwner::Constructor(id) => {
                let (foundation, key) =
                    self.source_record(id, |foundation| &foundation.constructors)?;
                self.declaration_origin(foundation, key)?;
                signature::source(key, root, position)
            }
            CallableTemplateOwner::Accessor(id) => {
                let (foundation, key) =
                    self.source_record(id, |foundation| &foundation.property_accessors)?;
                let property = match key.owner() {
                    PropertyOwner::Property(id) => self.key(&foundation.properties, id)?,
                    PropertyOwner::ExtensionProperty(id) => {
                        self.key(&foundation.extension_properties, id)?
                    }
                };
                self.declaration_origin(foundation, property)?;
                signature::accessor(property, key.role(), root, position)
            }
            CallableTemplateOwner::VariantConstructor(id) => {
                let (foundation, _) =
                    self.source_record(id, |foundation| &foundation.enum_variants)?;
                Self::source_origin(foundation, DefinitionOriginSubject::EnumVariant(id))
            }
            CallableTemplateOwner::ReleaseHook(exact) => {
                if position.is_some() {
                    return Err(Error::Materialization(root));
                }
                let owner = self
                    .foundation
                    .release_hook_owner(exact)
                    .ok_or(Error::Materialization(root))?;
                let (foundation, key) = match owner {
                    scoop_identity::NominalDeclarationOwner::Concrete(id) => {
                        self.source_record(id, |foundation| &foundation.types)?
                    }
                    scoop_identity::NominalDeclarationOwner::GenericTemplate(id) => {
                        self.source_record(id, |foundation| &foundation.generic_types)?
                    }
                };
                self.declaration_origin(foundation, key)
            }
            CallableTemplateOwner::GenericFunction(_) => Err(Error::Materialization(root)),
            CallableTemplateOwner::Generated(_) => {
                unreachable!("generated roots were resolved with their substitution context")
            }
        }
    }

    fn property(&mut self, property: PropertyOwner) -> Result<(), Error> {
        let key = self.property_key(property)?;
        self.source(key)
    }

    fn property_key(&mut self, property: PropertyOwner) -> Result<&'a SourceDeclarationKey, Error> {
        let foundation = self.foundation;
        let key = match property {
            PropertyOwner::Property(id) => self.key(&foundation.properties, id)?,
            PropertyOwner::ExtensionProperty(id) => {
                self.key(&foundation.extension_properties, id)?
            }
        };
        Ok(key)
    }

    fn source(&self, key: &SourceDeclarationKey) -> Result<(), Error> {
        let actual = key.origin();
        if actual == self.current {
            Ok(())
        } else {
            Err(Error::Provider {
                expected: self.current,
                actual,
            })
        }
    }

    fn declaration_origin(
        &self,
        foundation: &CanonicalHirFoundation,
        key: &SourceDeclarationKey,
    ) -> Result<(), Error> {
        if std::ptr::eq(foundation, self.foundation) {
            self.source(key)
        } else {
            Ok(())
        }
    }

    fn origin(&mut self, subject: DefinitionOriginSubject) -> Result<(), Error> {
        Self::source_origin(self.foundation, subject)
    }

    fn source_origin(
        foundation: &CanonicalHirFoundation,
        subject: DefinitionOriginSubject,
    ) -> Result<(), Error> {
        let origin = foundation
            .definition_origin(subject)
            .ok_or(Error::MissingOrigin(subject))?;
        foundation
            .validate_definition_origin_location(origin.origin().source().cone(), origin.origin())
            .map_err(|source| Error::Origin(Box::new(source)))
    }

    fn key<'b, I: PersistentId, K>(
        &self,
        records: &'b [CborIdentityRecord<I, K>],
        id: I,
    ) -> Result<&'b K, Error> {
        records
            .iter()
            .find(|record| record.id() == id)
            .map(|record| record.key())
            .ok_or(Error::MissingIdentity {
                kind: std::any::type_name::<I>(),
                id: *id.as_array(),
            })
    }

    fn source_record<I: PersistentId + 'a, K>(
        &self,
        id: I,
        records: fn(&CanonicalHirFoundation) -> &[CborIdentityRecord<I, K>],
    ) -> Result<(&'a CanonicalHirFoundation, &'a K), Error> {
        std::iter::once(self.foundation)
            .chain(self.dependencies.iter().copied())
            .find_map(|foundation| {
                records(foundation)
                    .iter()
                    .find(|record| record.id() == id)
                    .map(|record| (foundation, record.key()))
            })
            .ok_or(Error::MissingIdentity {
                kind: std::any::type_name::<I>(),
                id: *id.as_array(),
            })
    }
}
