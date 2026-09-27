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
            HirDependencyTypePositionV1::FieldStorage(field) => input.field(field),
            HirDependencyTypePositionV1::EnumVariantFieldStorage(field) => {
                input.variant_field(field)
            }
            HirDependencyTypePositionV1::ConstructorInitializerResult(root) => {
                input.constructor_initializer(root)
            }
            HirDependencyTypePositionV1::InitializationCycleMessage(unit) => {
                input.initialization(unit)
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
        let foundation = self.foundation;
        match root.template() {
            CallableTemplateOwner::Function(id) => {
                let key = self.key(&foundation.functions, id)?;
                self.source(key)?;
                signature::source(key, root, position)
            }
            CallableTemplateOwner::Constructor(id) => {
                let key = self.key(&foundation.constructors, id)?;
                self.source(key)?;
                signature::source(key, root, position)
            }
            CallableTemplateOwner::Accessor(id) => {
                let key = self.key(&foundation.property_accessors, id)?;
                let property = self.property_key(key.owner())?;
                self.source(property)?;
                signature::accessor(property, key.role(), root, position)
            }
            CallableTemplateOwner::Generated(id) => {
                let key = self.key(&foundation.generated_callables, id)?;
                signature::generated(key, root, position)?;
                // Generated bodies can have no source span. Their typed owners
                // and roles were resolved by the same foundation, and later
                // MIR joins still check the actual generated signature.
                let subject = DefinitionOriginSubject::GeneratedCallable(id);

                if foundation.definition_origin(subject).is_some() {
                    self.origin(subject)?;
                }
                Ok(())
            }
            CallableTemplateOwner::VariantConstructor(id) => {
                self.key(&foundation.enum_variants, id)?;
                self.origin(DefinitionOriginSubject::EnumVariant(id))
            }
            CallableTemplateOwner::GenericFunction(_) => Err(Error::Materialization(root)),
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

    fn origin(&mut self, subject: DefinitionOriginSubject) -> Result<(), Error> {
        let foundation: &CanonicalHirFoundation = self.foundation;

        let origin = foundation
            .definition_origin(subject)
            .ok_or(Error::MissingOrigin(subject))?;
        self.foundation
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
