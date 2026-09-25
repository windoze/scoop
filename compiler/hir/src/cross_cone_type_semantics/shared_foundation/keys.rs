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
        let origin = self.nominal_key(owner)?.origin();
        let public = if origin == self.current.provider {
            self.current.public
        } else {
            self.dependency(origin)?.metadata.public
        };

        public
            .nominal_interfaces()
            .declaration(SourceNominalId::Concrete(owner))
            .ok_or(Error::MissingNominal(owner))
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
        let key = match signature {
            SignatureTypeKey::Nominal(owner) => ExactTypeKey::Nominal(*owner),
            SignatureTypeKey::NominalApplication { .. } | SignatureTypeKey::Binder { .. } => {
                return Err(Error::NonConcreteSignature);
            }
            SignatureTypeKey::Tuple(elements) => ExactTypeKey::Tuple(
                NonEmptyVec::new(self.exacts(elements.as_slice())?)
                    .map_err(|e| Error::Key(e.to_string()))?,
            ),
            SignatureTypeKey::Function {
                effect,
                parameters,
                result,
            } => ExactTypeKey::Function {
                effect: *effect,
                parameters: self.exacts(parameters)?,
                result: self.exact(result)?,
            },
            SignatureTypeKey::RawPointer(pointee) => ExactTypeKey::RawPointer(self.exact(pointee)?),
            SignatureTypeKey::NativeFunctionPointer {
                calling_convention,
                parameters,
                result,
            } => ExactTypeKey::NativeFunctionPointer {
                calling_convention: *calling_convention,
                parameters: self.exacts(parameters)?,
                result: self.exact(result)?,
            },
        };
        self.exact_key(key)
    }

    fn exacts(self, signatures: &[SignatureTypeKey]) -> Result<Vec<PersistentExactTypeId>, Error> {
        let mut exacts = Vec::new();
        scoop_wire::allocation::try_reserve(&mut exacts, signatures.len(), &WirePath::root())?;
        for signature in signatures {
            exacts.push(self.exact(signature)?);
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
