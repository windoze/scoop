use super::{
    CanonicalPropertyInterfacesV1, PropertyDeclarationRecordV1, PropertyPublicAccessV1,
    PropertyRepresentationV1, PropertySetterPublicAccessV1,
};
use crate::{
    CallableDeclarationRecordV1, CallableImplementationV1, CallableInfixV1, CallableModalityV1,
    CallableOperatorRoleV1, CanonicalCallableInterfacesV1, DeclaredVisibilityV1,
    PropertyDeclarationId, PublicDeclarationOwnerV1, PublicLookupAccessV1,
};
use scoop_identity::{
    AccessorRole, CallableTemplateOrigin, CoreBuiltinNominal, Effect, PersistentPropertyAccessorId,
    SignatureTypeKey,
};
use scoop_wire::{BudgetMeter, DecodeLimits, WirePath};
use std::collections::BTreeMap;

mod errors;
mod validation;
pub use errors::PropertyAccessorClosureValidationError;
use validation::{validate_runtime_modality, validate_source_accessor};

#[derive(Clone, Copy)]
struct AccessorExpectation<'property> {
    property: &'property PropertyDeclarationRecordV1,
    role: AccessorRole,
    public: Option<PublicLookupAccessV1>,
}

impl CanonicalPropertyInterfacesV1 {
    pub fn validate_accessor_closure(
        &self,
        callables: &CanonicalCallableInterfacesV1,
    ) -> Result<(), PropertyAccessorClosureValidationError> {
        self.validate_accessor_closure_with_budget(
            callables,
            &mut BudgetMeter::new(DecodeLimits::default()),
        )
    }

    pub fn validate_accessor_closure_with_budget(
        &self,
        callables: &CanonicalCallableInterfacesV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), PropertyAccessorClosureValidationError> {
        use PropertyAccessorClosureValidationError as Error;
        let path = WirePath::root().field(4);
        let mut expected = BTreeMap::new();
        for property in self.all_declarations() {
            meter
                .charge_work(u64::from(self.records().len().max(1).ilog2()) + 1, &path)
                .map_err(Error::Resource)?;
            let public = self.get(property.declaration());
            let access = public.map(|p| match p.access() {
                PropertyPublicAccessV1::DirectOnly => PublicLookupAccessV1::DirectOnly,
                PropertyPublicAccessV1::PublicSlot => PublicLookupAccessV1::PublicSlot,
            });
            let accessors = property.accessors();
            for (accessor, role, public) in
                std::iter::once((accessors.getter(), AccessorRole::Getter, access)).chain(
                    accessors.setter().map(|setter| {
                        let setter_access = public
                            .filter(|p| {
                                p.capability().setter_access()
                                    == Some(PropertySetterPublicAccessV1::Public)
                            })
                            .and(access);
                        (setter, AccessorRole::Setter, setter_access)
                    }),
                )
            {
                meter
                    .check_table_entries(expected.len() as u64 + 1, &path)
                    .map_err(Error::Resource)?;
                meter
                    .charge_collection_slots(1, &path)
                    .map_err(Error::Resource)?;
                meter
                    .charge_work(u64::from(expected.len().max(1).ilog2()) + 1, &path)
                    .map_err(Error::Resource)?;
                if let Some(previous) = expected.insert(
                    accessor,
                    AccessorExpectation {
                        property,
                        role,
                        public,
                    },
                ) {
                    return Err(Error::DuplicateAccessorClaim {
                        accessor,
                        first: previous.property.declaration(),
                        second: property.declaration(),
                    });
                }
            }
        }
        for (&accessor, expectation) in &expected {
            meter
                .charge_work(
                    2 * (u64::from(callables.declaration_count().max(1).ilog2()) + 1),
                    &path,
                )
                .map_err(Error::Resource)?;
            let id = CallableTemplateOrigin::Accessor(accessor);
            let lookup = callables.get(id);
            match (expectation.public, lookup) {
                (Some(_), None) => {
                    return Err(Error::MissingPublicAccessor {
                        property: expectation.property.declaration(),
                        role: expectation.role,
                        accessor,
                    });
                }
                (None, Some(_)) if expectation.role == AccessorRole::Setter => {
                    return Err(Error::RestrictedSetterExported {
                        property: expectation.property.declaration(),
                        accessor,
                    });
                }
                (None, Some(_)) => {
                    return Err(Error::RestrictedGetterExported {
                        property: expectation.property.declaration(),
                        accessor,
                    });
                }
                (Some(access), Some(callable)) if callable.access() != access => {
                    return Err(Error::Access {
                        accessor,
                        expected: access,
                        actual: callable.access(),
                    });
                }
                (Some(_), Some(_)) | (None, None) => Ok(()),
            }?;
            let callable = callables
                .declaration(id)
                .ok_or(Error::MissingSourceAccessor {
                    property: expectation.property.declaration(),
                    role: expectation.role,
                    accessor,
                })?;
            validate_source_accessor(accessor, *expectation, callable)?;
            if expectation.role == AccessorRole::Setter
                && self.get(expectation.property.declaration()).is_some()
                && expectation.public.is_some()
                    != (callable.declared_visibility() == DeclaredVisibilityV1::Public)
            {
                return Err(Error::SetterLookup {
                    property: expectation.property.declaration(),
                    accessor,
                });
            }
        }
        for callable in callables.all_declarations() {
            meter
                .charge_work(u64::from(expected.len().max(1).ilog2()) + 1, &path)
                .map_err(Error::Resource)?;
            let CallableTemplateOrigin::Accessor(accessor) = callable.declaration() else {
                continue;
            };
            if !expected.contains_key(&accessor) {
                return Err(if callables.get(callable.declaration()).is_some() {
                    Error::OrphanPublicAccessor(accessor)
                } else {
                    Error::OrphanSourceAccessor(accessor)
                });
            }
        }
        for property in self.all_declarations() {
            meter
                .charge_work(
                    2 * (u64::from(callables.declaration_count().max(1).ilog2()) + 1),
                    &path,
                )
                .map_err(Error::Resource)?;
            validate_runtime_modality(property, callables)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
