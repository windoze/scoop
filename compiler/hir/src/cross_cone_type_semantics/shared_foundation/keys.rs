use std::{collections::BTreeSet, sync::Arc};

use scoop_identity::{ExactTypeKey, NonEmptyVec, SignatureTypeKey, SourceDeclarationKey};
use scoop_wire::WirePath;

use super::*;
use crate::{CheckedExactTypeFactV1, NominalInterfaceRecordV1, SourceNominalId};

impl<'a> MetadataTypes<'a, '_> {
    pub(super) fn validate_dependencies(self, meter: &mut BudgetMeter) -> Result<(), Error> {
        let mut providers = BTreeSet::new();
        for dependency in self.dependencies {
            meter.charge_collection_slots(1, &WirePath::root())?;
            meter.charge_work(
                1 + u64::from(providers.len().max(1).ilog2()),
                &WirePath::root(),
            )?;
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

    pub(crate) fn key(
        self,
        exact: PersistentExactTypeId,
        meter: &mut BudgetMeter,
    ) -> Result<Arc<ExactTypeKey>, Error> {
        self.lookup_work(meter)?;
        Ok(self
            .current
            .identities
            .canonical_key::<PersistentExactTypeId, ExactTypeKey>(exact)?)
    }

    pub(crate) fn nominal_key(
        self,
        owner: PersistentTypeId,
        meter: &mut BudgetMeter,
    ) -> Result<Arc<SourceDeclarationKey>, Error> {
        self.lookup_work(meter)?;
        Ok(self
            .current
            .identities
            .canonical_key::<PersistentTypeId, SourceDeclarationKey>(owner)?)
    }

    pub(crate) fn nominal(
        self,
        owner: PersistentTypeId,
        meter: &mut BudgetMeter,
    ) -> Result<&'a NominalInterfaceRecordV1, Error> {
        let origin = self.nominal_key(owner, meter)?.origin();
        let public = if origin == self.current.provider {
            self.current.public
        } else {
            self.dependency(origin, meter)?.metadata.public
        };
        meter.charge_work(
            1 + u64::from(
                public
                    .nominal_interfaces()
                    .declaration_count()
                    .max(1)
                    .ilog2(),
            ),
            &WirePath::root(),
        )?;
        public
            .nominal_interfaces()
            .declaration(SourceNominalId::Concrete(owner))
            .ok_or(Error::MissingNominal(owner))
    }

    pub(super) fn dependency(
        self,
        provider: ConeIdentity,
        meter: &mut BudgetMeter,
    ) -> Result<CheckedSharedTypeFoundationV1<'a>, Error> {
        meter.charge_work(self.dependencies.len() as u64 + 1, &WirePath::root())?;
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
        meter: &mut BudgetMeter,
    ) -> Result<CheckedExactTypeFactV1<'a>, Error> {
        let dependency = self.dependency(provider, meter)?;
        meter.charge_work(
            1 + u64::from(dependency.facts.records().len().max(1).ilog2()),
            &WirePath::root(),
        )?;
        dependency
            .facts
            .get_checked(exact)
            .ok_or(Error::MissingFact(exact))
    }

    pub(crate) fn nominal_exact(
        self,
        owner: PersistentTypeId,
        meter: &mut BudgetMeter,
    ) -> Result<PersistentExactTypeId, Error> {
        self.exact_key(ExactTypeKey::Nominal(owner), meter)
    }

    pub(crate) fn exact(
        self,
        signature: &SignatureTypeKey,
        depth: u64,
        meter: &mut BudgetMeter,
    ) -> Result<PersistentExactTypeId, Error> {
        let path = WirePath::root();
        meter.check_semantic_depth(depth, &path)?;
        meter.charge_work(1, &path)?;
        let key = match signature {
            SignatureTypeKey::Nominal(owner) => ExactTypeKey::Nominal(*owner),
            SignatureTypeKey::NominalApplication { .. } | SignatureTypeKey::Binder { .. } => {
                return Err(Error::NonConcreteSignature);
            }
            SignatureTypeKey::Tuple(elements) => ExactTypeKey::Tuple(
                NonEmptyVec::new(self.exacts(elements.as_slice(), depth, meter)?)
                    .map_err(|e| Error::Key(e.to_string()))?,
            ),
            SignatureTypeKey::Function {
                effect,
                parameters,
                result,
            } => ExactTypeKey::Function {
                effect: *effect,
                parameters: self.exacts(parameters, depth, meter)?,
                result: self.exact(result, depth + 1, meter)?,
            },
            SignatureTypeKey::RawPointer(pointee) => {
                ExactTypeKey::RawPointer(self.exact(pointee, depth + 1, meter)?)
            }
            SignatureTypeKey::NativeFunctionPointer {
                calling_convention,
                parameters,
                result,
            } => ExactTypeKey::NativeFunctionPointer {
                calling_convention: *calling_convention,
                parameters: self.exacts(parameters, depth, meter)?,
                result: self.exact(result, depth + 1, meter)?,
            },
        };
        self.exact_key(key, meter)
    }

    fn exacts(
        self,
        signatures: &[SignatureTypeKey],
        depth: u64,
        meter: &mut BudgetMeter,
    ) -> Result<Vec<PersistentExactTypeId>, Error> {
        let mut exacts = Vec::new();
        meter.try_reserve_collection_slots(&mut exacts, signatures.len(), &WirePath::root())?;
        for signature in signatures {
            exacts.push(self.exact(signature, depth + 1, meter)?);
        }
        Ok(exacts)
    }

    fn exact_key(
        self,
        key: ExactTypeKey,
        meter: &mut BudgetMeter,
    ) -> Result<PersistentExactTypeId, Error> {
        meter.charge_sha256(
            PersistentExactTypeId::hash_stream_length(&key)
                .map_err(|e| Error::Key(e.to_string()))?,
            &WirePath::root(),
        )?;
        let exact = PersistentExactTypeId::from_key(&key).map_err(|e| Error::Key(e.to_string()))?;
        if self.key(exact, meter)?.as_ref() != &key {
            return Err(Error::Key("exact type key mismatch".into()));
        }
        Ok(exact)
    }

    fn lookup_work(self, meter: &mut BudgetMeter) -> Result<(), Error> {
        Ok(meter.charge_work(
            1 + u64::from(self.current.identities.identity_count().max(1).ilog2()),
            &WirePath::root(),
        )?)
    }
}
