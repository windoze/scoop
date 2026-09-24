//! Declaration type sites bind to this artifact's existing typed identities.

use super::{CanonicalHirFoundation, OdrFreeHirFoundation};
use crate::{HirCallableTypePositionV1, HirDependencyTypePositionV1};
use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
    CborIdentityRecord, ConeIdentity, DefinitionOriginSubject, PersistentId, PropertyOwner,
    SourceDeclarationKey,
};
use scoop_wire::{BudgetMeter, WirePath};

mod errors;
mod signature;
pub use errors::DeclarationTypeSiteValidationError;
type Error = DeclarationTypeSiteValidationError;

impl OdrFreeHirFoundation {
    pub fn validate_declaration_type_position(
        &self,
        current: ConeIdentity,
        position: HirDependencyTypePositionV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), Error> {
        let mut input = Input {
            foundation: self,
            current,
            meter,
            path,
        };
        match position {
            HirDependencyTypePositionV1::Expression(..) => Err(Error::ExpressionPosition),
            HirDependencyTypePositionV1::CallableSignature(root, part) => {
                input.root(root, Some(part))
            }
            HirDependencyTypePositionV1::LocalValue(local) => {
                let key = input.key(&self.as_canonical().local_values, local)?;
                input.root(key.owner(), None)?;
                input.origin(DefinitionOriginSubject::LocalValue(local))
            }
            HirDependencyTypePositionV1::BackingStorage(property)
            | HirDependencyTypePositionV1::DelegateStorage(property) => input.property(property),
        }
    }
}

struct Input<'a> {
    foundation: &'a OdrFreeHirFoundation,
    current: ConeIdentity,
    meter: &'a mut BudgetMeter,
    path: &'a WirePath,
}

impl<'a> Input<'a> {
    fn root(
        &mut self,
        root: CallableMaterialization,
        position: Option<HirCallableTypePositionV1>,
    ) -> Result<(), Error> {
        if root.context() != CallableMaterializationContext::NoSubstitution {
            return Err(Error::Materialization(root));
        }
        let foundation = self.foundation.as_canonical();
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
                self.meter.charge_work(
                    1 + u64::from(foundation.counts().definition_origins.max(1).ilog2()),
                    self.path,
                )?;
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
        let foundation = self.foundation.as_canonical();
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
        let foundation: &CanonicalHirFoundation = self.foundation.as_canonical();
        self.meter.charge_work(
            1 + u64::from(foundation.counts().definition_origins.max(1).ilog2()),
            self.path,
        )?;
        let origin = foundation
            .definition_origin(subject)
            .ok_or(Error::MissingOrigin(subject))?;
        self.foundation
            .validate_definition_origin_location(
                self.current,
                origin.origin(),
                self.meter,
                self.path,
            )
            .map_err(|source| Error::Origin(Box::new(source)))
    }

    fn key<'b, I: PersistentId, K>(
        &mut self,
        records: &'b [CborIdentityRecord<I, K>],
        id: I,
    ) -> Result<&'b K, Error> {
        self.meter.charge_work(records.len() as u64, self.path)?;
        records
            .iter()
            .find(|record| record.id() == id)
            .map(|record| record.key())
            .ok_or(Error::MissingIdentity {
                kind: std::any::type_name::<I>(),
                id: *id.as_array(),
            })
    }
}
