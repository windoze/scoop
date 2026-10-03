use super::*;
use scoop_identity::NonEmptyVec;

impl Loaded {
    pub fn register_signature(&mut self, signature: &SignatureTypeKey) -> PersistentExactTypeId {
        let key = match signature {
            SignatureTypeKey::Nominal(owner) => ExactTypeKey::Nominal(*owner),
            SignatureTypeKey::Tuple(elements) => ExactTypeKey::Tuple(
                NonEmptyVec::new(
                    elements
                        .as_slice()
                        .iter()
                        .map(|element| self.register_signature(element))
                        .collect(),
                )
                .unwrap(),
            ),
            SignatureTypeKey::Function {
                effect,
                parameters,
                result,
            } => ExactTypeKey::Function {
                effect: *effect,
                parameters: parameters
                    .iter()
                    .map(|parameter| self.register_signature(parameter))
                    .collect(),
                result: self.register_signature(result),
            },
            SignatureTypeKey::RawPointer(pointee) => {
                ExactTypeKey::RawPointer(self.register_signature(pointee))
            }
            SignatureTypeKey::NativeFunctionPointer {
                calling_convention,
                parameters,
                result,
            } => ExactTypeKey::NativeFunctionPointer {
                calling_convention: *calling_convention,
                parameters: parameters
                    .iter()
                    .map(|parameter| self.register_signature(parameter))
                    .collect(),
                result: self.register_signature(result),
            },
            SignatureTypeKey::Binder { .. } | SignatureTypeKey::NominalApplication { .. } => {
                panic!("call fixtures register complete parameter-free signatures")
            }
        };
        let record = CborIdentityRecord::<PersistentExactTypeId, _>::from_key(key).unwrap();
        let id = record.id();
        if self
            .identities
            .canonical_key::<_, ExactTypeKey>(id)
            .is_err()
        {
            let mut pending = PendingIdentityValidation::new();
            pending
                .register_external_graph_authorities(&self.identities)
                .unwrap();
            pending
                .register_external_canonical_authority(record)
                .unwrap();
            self.identities = pending.finish().unwrap();
        }
        id
    }
}
