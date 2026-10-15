use std::sync::Arc;

use scoop_identity::{ExactTypeKey, NonEmptyVec, SignatureTypeKey};
use scoop_wire::WirePath;

use super::*;

impl<'a> SharedTypeMetadataV1<'a> {
    pub(crate) fn key(self, exact: PersistentExactTypeId) -> Result<Arc<ExactTypeKey>, Error> {
        Ok(self
            .identities
            .canonical_key::<PersistentExactTypeId, ExactTypeKey>(exact)?)
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
                receiver,
                effect,
                parameters,
                result,
            } => ExactTypeKey::Function {
                receiver: receiver
                    .as_ref()
                    .map(|ty| self.exact_with_bindings(ty, bindings))
                    .transpose()?,
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
        let declaration = self.public.nominal_interfaces().declaration(owner);
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
                receiver: None,
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
