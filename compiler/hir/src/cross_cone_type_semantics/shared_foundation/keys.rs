use std::{collections::BTreeSet, sync::Arc};

use scoop_identity::{ExactTypeKey, NonEmptyVec, SignatureTypeKey, SourceDeclarationKey};
use scoop_wire::WirePath;

use super::*;
use crate::{CheckedExactTypeFactV1, NominalInterfaceRecordV1, SourceNominalId};

impl<'a> MetadataTypes<'a, '_> {
    pub(super) fn validate_dependencies(self) -> Result<(), Error> {
        let mut providers = BTreeSet::new();
        for dependency in self.dependencies {
            let provider = dependency.provider();
            if provider == self.current.provider {
                return Err(Error::CurrentProviderDependency(provider));
            }
            if !providers.insert(provider) {
                return Err(Error::DuplicateProvider(provider));
            }
        }
        Ok(())
    }

    pub(crate) fn key(self, exact: PersistentExactTypeId) -> Result<Arc<ExactTypeKey>, Error> {
        Ok(self
            .current
            .identities
            .canonical_key::<PersistentExactTypeId, ExactTypeKey>(exact)?)
    }

    pub(crate) fn nominal_key(
        self,
        owner: PersistentTypeId,
    ) -> Result<Arc<SourceDeclarationKey>, Error> {
        Ok(self
            .current
            .identities
            .canonical_key::<PersistentTypeId, SourceDeclarationKey>(owner)?)
    }

    pub(crate) fn nominal(
        self,
        owner: PersistentTypeId,
    ) -> Result<&'a NominalInterfaceRecordV1, Error> {
        self.nominal_declaration(SourceNominalId::Concrete(owner))
    }

    pub(crate) fn nominal_declaration(
        self,
        owner: SourceNominalId,
    ) -> Result<&'a NominalInterfaceRecordV1, Error> {
        let origin = match owner {
            SourceNominalId::Concrete(owner) => self.nominal_key(owner)?.origin(),
            SourceNominalId::GenericTemplate(owner) => self
                .current
                .identities
                .canonical_key::<_, SourceDeclarationKey>(owner)?
                .origin(),
        };
        let public = if origin == self.current.provider {
            self.current.public
        } else {
            self.dependency(origin)?.metadata.public
        };

        public
            .nominal_interfaces()
            .declaration(owner)
            .ok_or(match owner {
                SourceNominalId::Concrete(owner) => Error::MissingNominal(owner),
                SourceNominalId::GenericTemplate(owner) => Error::MissingGenericNominal(owner),
            })
    }

    pub(super) fn dependency(
        self,
        provider: ConeIdentity,
    ) -> Result<CheckedSharedTypeFoundationV1<'a>, Error> {
        self.dependencies
            .iter()
            .find(|dependency| dependency.provider() == provider)
            .copied()
            .ok_or(Error::MissingProvider(provider))
    }

    pub(crate) fn dependency_fact(
        self,
        provider: ConeIdentity,
        exact: PersistentExactTypeId,
    ) -> Result<CheckedExactTypeFactV1<'a>, Error> {
        let dependency = self.dependency(provider)?;

        dependency
            .facts
            .get_checked(exact)
            .ok_or(Error::MissingFact(exact))
    }

    pub(crate) fn nominal_exact(
        self,
        owner: PersistentTypeId,
    ) -> Result<PersistentExactTypeId, Error> {
        self.exact_key(ExactTypeKey::Nominal(owner))
    }

    pub(crate) fn exact(
        self,
        signature: &SignatureTypeKey,
    ) -> Result<PersistentExactTypeId, Error> {
        self.exact_with_bindings(signature, &[])
    }

    pub(crate) fn exact_with_bindings(
        self,
        signature: &SignatureTypeKey,
        bindings: &[Vec<PersistentExactTypeId>],
    ) -> Result<PersistentExactTypeId, Error> {
        let key = match signature {
            SignatureTypeKey::Nominal(owner) => ExactTypeKey::Nominal(*owner),
            SignatureTypeKey::NominalApplication { origin, arguments } => {
                ExactTypeKey::NominalApplication {
                    origin: *origin,
                    arguments: NonEmptyVec::new(self.exacts(arguments.as_slice(), bindings)?)
                        .map_err(|error| Error::Key(error.to_string()))?,
                }
            }
            SignatureTypeKey::Binder { depth, index } => {
                let exact = bindings
                    .get(*depth as usize)
                    .and_then(|scope| scope.get(*index as usize))
                    .copied()
                    .ok_or(Error::NonConcreteSignature)?;
                self.key(exact)?;
                return Ok(exact);
            }
            SignatureTypeKey::Tuple(elements) => ExactTypeKey::Tuple(
                NonEmptyVec::new(self.exacts(elements.as_slice(), bindings)?)
                    .map_err(|e| Error::Key(e.to_string()))?,
            ),
            SignatureTypeKey::Function {
                effect,
                parameters,
                result,
            } => ExactTypeKey::Function {
                effect: *effect,
                parameters: self.exacts(parameters, bindings)?,
                result: self.exact_with_bindings(result, bindings)?,
            },
            SignatureTypeKey::RawPointer(pointee) => {
                ExactTypeKey::RawPointer(self.exact_with_bindings(pointee, bindings)?)
            }
            SignatureTypeKey::NativeFunctionPointer {
                calling_convention,
                parameters,
                result,
            } => ExactTypeKey::NativeFunctionPointer {
                calling_convention: *calling_convention,
                parameters: self.exacts(parameters, bindings)?,
                result: self.exact_with_bindings(result, bindings)?,
            },
        };
        self.exact_key(key)
    }

    fn exacts(
        self,
        signatures: &[SignatureTypeKey],
        bindings: &[Vec<PersistentExactTypeId>],
    ) -> Result<Vec<PersistentExactTypeId>, Error> {
        let mut exacts = Vec::new();
        scoop_wire::allocation::try_reserve(&mut exacts, signatures.len(), &WirePath::root())?;
        for signature in signatures {
            exacts.push(self.exact_with_bindings(signature, bindings)?);
        }
        Ok(exacts)
    }

    fn exact_key(self, key: ExactTypeKey) -> Result<PersistentExactTypeId, Error> {
        let exact = PersistentExactTypeId::from_key(&key).map_err(|e| Error::Key(e.to_string()))?;
        if self.key(exact)?.as_ref() != &key {
            return Err(Error::Key("exact type key mismatch".into()));
        }
        Ok(exact)
    }
}
