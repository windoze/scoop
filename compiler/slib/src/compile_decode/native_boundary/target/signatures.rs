//! Exact signature substitution at native and Scoop ABI boundaries.

use super::*;

impl NativeBoundaryNormalizer<'_> {
    pub(super) fn signature_exact(
        &mut self,
        source: &SignatureTypeKey,
        binders: &[Vec<PersistentExactTypeId>],
    ) -> Result<PersistentExactTypeId, NativeBoundaryCompileError> {
        self.signature_exact_at(source, binders)
    }

    fn signature_exact_at(
        &mut self,
        source: &SignatureTypeKey,
        binders: &[Vec<PersistentExactTypeId>],
    ) -> Result<PersistentExactTypeId, NativeBoundaryCompileError> {
        let path = WirePath::root().field(15);

        let key = match source {
            SignatureTypeKey::Binder { depth, index } => {
                let group = binders
                    .get(*depth as usize)
                    .ok_or(NativeBoundaryTargetError::BinderDepthOutOfRange { depth: *depth })?;
                return group
                    .get(*index as usize)
                    .copied()
                    .ok_or(NativeBoundaryTargetError::BinderIndexOutOfRange {
                        depth: *depth,
                        index: *index,
                    })
                    .map_err(Into::into);
            }
            SignatureTypeKey::Nominal(owner) => ExactTypeKey::Nominal(*owner),
            SignatureTypeKey::NominalApplication { origin, arguments } => {
                let mut resolved = allocate_vec(arguments.as_slice().len(), &path)?;
                for argument in arguments.as_slice() {
                    resolved.push(self.signature_exact_at(argument, binders)?);
                }
                self.application_key(*origin, resolved)?
            }
            SignatureTypeKey::Tuple(elements) => {
                let mut resolved = allocate_vec(elements.as_slice().len(), &path)?;
                for element in elements.as_slice() {
                    resolved.push(self.signature_exact_at(element, binders)?);
                }
                ExactTypeKey::Tuple(
                    NonEmptyVec::new(resolved)
                        .map_err(|_| NativeBoundaryTargetError::InvalidSignatureShape)?,
                )
            }
            SignatureTypeKey::Function {
                effect,
                parameters,
                result,
            } => {
                let mut resolved = allocate_vec(parameters.len(), &path)?;
                for parameter in parameters {
                    resolved.push(self.signature_exact_at(parameter, binders)?);
                }
                ExactTypeKey::Function {
                    effect: *effect,
                    parameters: resolved,
                    result: self.signature_exact_at(result, binders)?,
                }
            }
            SignatureTypeKey::RawPointer(pointee) => {
                ExactTypeKey::RawPointer(self.signature_exact_at(pointee, binders)?)
            }
            SignatureTypeKey::NativeFunctionPointer {
                calling_convention,
                parameters,
                result,
            } => {
                let mut resolved = allocate_vec(parameters.len(), &path)?;
                for parameter in parameters {
                    resolved.push(self.signature_exact_at(parameter, binders)?);
                }
                ExactTypeKey::NativeFunctionPointer {
                    calling_convention: *calling_convention,
                    parameters: resolved,
                    result: self.signature_exact_at(result, binders)?,
                }
            }
        };

        let exact =
            PersistentExactTypeId::from_key(&key).map_err(NativeBoundaryTargetError::Hash)?;
        match self.exact_types.get(&exact) {
            Some(actual) if actual.as_ref() == &key => Ok(exact),
            _ => Err(NativeBoundaryTargetError::MissingExactType { exact }.into()),
        }
    }

    fn application_key(
        &self,
        origin: scoop_identity::PersistentGenericTypeId,
        arguments: Vec<PersistentExactTypeId>,
    ) -> Result<ExactTypeKey, NativeBoundaryCompileError> {
        let owner = NativeBoundaryNominalOwner::GenericTemplate(origin);
        if self.definitions.get(&owner).is_some_and(|definition| {
            matches!(definition.shape(), NativeBoundaryNominalShape::Intrinsic(representation)
                if representation.family() == scoop_hir::IntrinsicTypeKind::FunPtr)
        }) {
            let [function] = arguments.as_slice() else {
                return Err(NativeBoundaryTargetError::InvalidSignatureShape.into());
            };
            let function = self
                .exact_types
                .get(function)
                .ok_or(NativeBoundaryTargetError::MissingExactType { exact: *function })?;
            let ExactTypeKey::Function {
                effect: scoop_identity::Effect::Ordinary,
                parameters,
                result,
            } = function.as_ref()
            else {
                return Err(NativeBoundaryTargetError::InvalidSignatureShape.into());
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
                .map_err(|_| NativeBoundaryTargetError::InvalidSignatureShape)?,
        })
    }
}
