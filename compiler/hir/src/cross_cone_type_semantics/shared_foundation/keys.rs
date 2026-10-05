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
            .identity_graph(exact)
            .canonical_key::<PersistentExactTypeId, ExactTypeKey>(exact)?)
    }

    /// Dependency inheritance may mention applications absent from the root's
    /// own uses. Read the original checked key from the same semantic world.
    pub(super) fn identity_graph<I: scoop_identity::PersistentId>(
        self,
        id: I,
    ) -> &'a ValidatedIdentityGraph {
        std::iter::once(self.current.identities)
            .chain(
                self.dependencies
                    .iter()
                    .map(|source| source.metadata.identities),
            )
            .find(|graph| graph.contains_resolved_identity(id))
            .unwrap_or(self.current.identities)
    }

    pub(crate) fn nominal_key(
        self,
        owner: PersistentTypeId,
    ) -> Result<Arc<SourceDeclarationKey>, Error> {
        Ok(self
            .identity_graph(owner)
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
                .identity_graph(owner)
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
                self.application_key(*origin, self.exacts(arguments.as_slice(), bindings)?)?
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

    fn application_key(
        self,
        origin: scoop_identity::PersistentGenericTypeId,
        arguments: Vec<PersistentExactTypeId>,
    ) -> Result<ExactTypeKey, Error> {
        let owner = crate::SourceNominalId::GenericTemplate(origin);
        let declaration = std::iter::once(self.current.public)
            .chain(
                self.dependencies
                    .iter()
                    .map(|source| source.metadata.public),
            )
            .find_map(|public| public.nominal_interfaces().declaration(owner));
        if declaration.is_some_and(|record| {
            matches!(record.source_shape(),
            crate::NominalSourceShapeV1::Intrinsic(representation)
                if representation.family() == crate::IntrinsicTypeKind::FunPtr)
        }) {
            let [function] = arguments.as_slice() else {
                return Err(Error::Key("FunPtr requires one function type".into()));
            };
            let function = self.key(*function)?;
            let ExactTypeKey::Function {
                effect: scoop_identity::Effect::Ordinary,
                parameters,
                result,
            } = function.as_ref()
            else {
                return Err(Error::Key(
                    "FunPtr requires an ordinary function type".into(),
                ));
            };
            return Ok(ExactTypeKey::NativeFunctionPointer {
                calling_convention: scoop_identity::CallingConvention::C,
                parameters: parameters.clone(),
                result: *result,
            });
        }
        Ok(ExactTypeKey::NominalApplication {
            origin,
            arguments: NonEmptyVec::new(arguments)
                .map_err(|error| Error::Key(error.to_string()))?,
        })
    }

    fn exact_key(self, key: ExactTypeKey) -> Result<PersistentExactTypeId, Error> {
        let exact = PersistentExactTypeId::from_key(&key).map_err(|e| Error::Key(e.to_string()))?;
        if self.key(exact)?.as_ref() != &key {
            return Err(Error::Key("exact type key mismatch".into()));
        }
        Ok(exact)
    }
}
