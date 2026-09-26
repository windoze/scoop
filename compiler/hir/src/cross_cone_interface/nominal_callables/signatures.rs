use scoop_identity::{ExactTypeKey, NonEmptyVec};

use super::{NominalCallableClassificationError as Error, *};

impl NominalExactLeafClassifierV1 {
    /// Resolves a param-free signature using the actual nominal declaration
    /// scope. Structural identity preserves element order and callable effects.
    pub fn classify(
        &self,
        signature: &SignatureTypeKey,
    ) -> Result<Option<PersistentExactTypeId>, Error> {
        let key = match signature {
            SignatureTypeKey::Nominal(source) => {
                return Ok(self
                    .leaves
                    .binary_search_by_key(source, |(candidate, _)| *candidate)
                    .ok()
                    .map(|index| self.leaves[index].1));
            }
            SignatureTypeKey::NominalApplication { .. } | SignatureTypeKey::Binder { .. } => {
                return Ok(None);
            }
            SignatureTypeKey::Tuple(elements) => {
                let Some(elements) = self.classify_elements(elements.as_slice())? else {
                    return Ok(None);
                };
                ExactTypeKey::Tuple(
                    NonEmptyVec::new(elements).expect("a signature tuple is nonempty"),
                )
            }
            SignatureTypeKey::Function {
                effect,
                parameters,
                result,
            } => {
                let (Some(parameters), Some(result)) =
                    (self.classify_elements(parameters)?, self.classify(result)?)
                else {
                    return Ok(None);
                };
                ExactTypeKey::Function {
                    effect: *effect,
                    parameters,
                    result,
                }
            }
            SignatureTypeKey::RawPointer(pointee) => {
                let Some(pointee) = self.classify(pointee)? else {
                    return Ok(None);
                };
                ExactTypeKey::RawPointer(pointee)
            }
            SignatureTypeKey::NativeFunctionPointer {
                calling_convention,
                parameters,
                result,
            } => {
                let (Some(parameters), Some(result)) =
                    (self.classify_elements(parameters)?, self.classify(result)?)
                else {
                    return Ok(None);
                };
                ExactTypeKey::NativeFunctionPointer {
                    calling_convention: *calling_convention,
                    parameters,
                    result,
                }
            }
        };
        PersistentExactTypeId::from_key(&key)
            .map(Some)
            .map_err(Error::Identity)
    }

    fn classify_elements(
        &self,
        signatures: &[SignatureTypeKey],
    ) -> Result<Option<Vec<PersistentExactTypeId>>, Error> {
        let mut elements = Vec::new();
        elements
            .try_reserve_exact(signatures.len())
            .map_err(|_| Error::Allocation {
                requested_slots: signatures.len(),
            })?;
        for signature in signatures {
            let Some(exact) = self.classify(signature)? else {
                return Ok(None);
            };
            elements.push(exact);
        }
        Ok(Some(elements))
    }
}
