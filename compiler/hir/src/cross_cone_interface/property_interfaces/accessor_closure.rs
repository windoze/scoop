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
use std::collections::BTreeMap;

mod errors;
mod validation;
pub use errors::PropertyAccessorClosureValidationError;
use validation::validate_source_accessor;

#[derive(Clone, Copy)]
struct AccessorExpectation<'property> {
    property: &'property PropertyDeclarationRecordV1,
    role: AccessorRole,
    public: Option<PublicLookupAccessV1>,
    implementation: crate::PropertyAccessorImplementationV1,
}

impl CanonicalPropertyInterfacesV1 {
    pub fn validate_accessor_closure(
        &self,
        callables: &CanonicalCallableInterfacesV1,
    ) -> Result<(), PropertyAccessorClosureValidationError> {
        use PropertyAccessorClosureValidationError as Error;

        let mut expected = BTreeMap::new();
        for property in self.all_declarations() {
            let public = self.get(property.declaration());
            let access = public.map(|p| match p.access() {
                PropertyPublicAccessV1::DirectOnly => PublicLookupAccessV1::DirectOnly,
                PropertyPublicAccessV1::PublicSlot => PublicLookupAccessV1::PublicSlot,
            });
            let accessors = property.accessors();
            for (source, role, public) in
                std::iter::once((accessors.getter_source(), AccessorRole::Getter, access)).chain(
                    accessors.setter_source().map(|setter| {
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
                let accessor = source.accessor();

                if let Some(previous) = expected.insert(
                    accessor,
                    AccessorExpectation {
                        property,
                        role,
                        public,
                        implementation: source.implementation(),
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
        Ok(())
    }
}

#[cfg(test)]
mod tests;
