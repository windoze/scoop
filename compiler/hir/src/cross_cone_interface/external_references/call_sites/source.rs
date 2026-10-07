use scoop_identity::{
    CallableTemplateOrigin, ExactTypeKey, PersistentExactTypeId, PersistentTypeId, SignatureTypeKey,
};
use scoop_wire::WireError;

use super::HirDependencyCallSiteV1;
use crate::{
    CallableDeclarationRecordV1, ExternalHirTargetV1, SharedTypeMetadataError,
    SharedTypeMetadataV1, SourceNominalId,
};

mod applications;
mod errors;

pub use errors::HirDependencyCallSignatureError;

impl HirDependencyCallSiteV1 {
    /// Joins this actual call with the provider's already checked declaration.
    /// Source access, execution roles and MIR implementation selection remain
    /// independent checks. The returned source declaration is joined with
    /// this call's complete substitution and every zero-sized argument.
    pub fn validate_source_signature<'a>(
        &self,
        target: ExternalHirTargetV1,
        metadata: SharedTypeMetadataV1<'a>,
        identities: &'a scoop_identity::ValidatedIdentityGraph,
        nominal_source: impl Fn(PersistentTypeId) -> Option<&'a crate::NominalInterfaceRecordV1>,
    ) -> Result<&'a CallableDeclarationRecordV1, HirDependencyCallSignatureError> {
        use HirDependencyCallSignatureError as Error;

        let ExternalHirTargetV1::Callable(
            declaration @ (CallableTemplateOrigin::Function(_)
            | CallableTemplateOrigin::GenericFunction(_)
            | CallableTemplateOrigin::Accessor(_)
            | CallableTemplateOrigin::Constructor(_)
            | CallableTemplateOrigin::VariantConstructor(_)),
        ) = target
        else {
            return Err(Error::Target(target));
        };
        let callables = metadata.public.callable_interfaces();

        let source = callables
            .declaration(declaration)
            .ok_or(Error::Declaration(declaration))?;
        if self.instantiation() == crate::HirDependencyCallInstantiationV1::NativeLeaf
            && (source.effects().implementation() != crate::CallableImplementationV1::SourceExternC
                || !matches!(
                    self.position().root.template(),
                    scoop_identity::CallableTemplateOwner::ReleaseHook(_)
                )
                || self.position().root.context()
                    != scoop_identity::CallableMaterializationContext::NoSubstitution)
        {
            return Err(Error::NativeLeaf(declaration));
        }
        let bindings = applications::bindings(self.instantiation(), source, metadata, identities)?;
        let exact = |ty: &SignatureTypeKey| {
            metadata.signature_exact_type_with_bindings(ty, &bindings, identities)
        };
        let construction = matches!(
            declaration,
            CallableTemplateOrigin::Constructor(_) | CallableTemplateOrigin::VariantConstructor(_)
        );
        let receiver = match source.owner().nominal_owner() {
            Some(SourceNominalId::Concrete(_)) if construction => None,
            Some(SourceNominalId::Concrete(owner)) => {
                Some(exact(&SignatureTypeKey::Nominal(owner))?)
            }
            Some(SourceNominalId::GenericTemplate(_)) if construction => None,
            Some(SourceNominalId::GenericTemplate(owner)) => {
                let depth = usize::from(!source.type_parameters().is_empty());
                let arguments = &bindings[depth];
                let signature = SignatureTypeKey::NominalApplication {
                    origin: owner,
                    arguments: scoop_identity::NonEmptyVec::new(
                        (0..arguments.len())
                            .map(|index| SignatureTypeKey::Binder {
                                depth: depth as u32,
                                index: index as u32,
                            })
                            .collect(),
                    )
                    .map_err(|_| Error::GenericDeclaration(declaration))?,
                };
                Some(exact(&signature)?)
            }
            None => source.receiver().map(exact).transpose()?,
        };
        let parameters = source.parameters().parameters();

        if self.receiver().has_receiver() != receiver.is_some() {
            return Err(Error::ReceiverRole {
                expected: receiver.is_some(),
                actual: self.receiver().has_receiver(),
            });
        }
        let expected = parameters
            .len()
            .saturating_add(usize::from(receiver.is_some()));

        if self.arguments().len() != expected {
            return Err(Error::ArgumentCount {
                expected,
                actual: self.arguments().len(),
            });
        }
        if let Some(expected) = receiver {
            argument(
                0,
                self.arguments()[0],
                expected,
                identities,
                &nominal_source,
            )?;
        }
        let offset = usize::from(receiver.is_some());
        for (index, parameter) in parameters.iter().enumerate() {
            let expected = exact(parameter.value_type())?;
            argument(
                index + offset,
                self.arguments()[index + offset],
                expected,
                identities,
                &nominal_source,
            )?;
        }
        let expected = exact(source.result())?;

        if self.result() != expected {
            return Err(Error::Result {
                expected,
                actual: self.result(),
            });
        }
        Ok(source)
    }
}

fn argument<'a>(
    index: usize,
    actual: PersistentExactTypeId,
    expected: PersistentExactTypeId,
    identities: &scoop_identity::ValidatedIdentityGraph,
    nominal_source: &impl Fn(PersistentTypeId) -> Option<&'a crate::NominalInterfaceRecordV1>,
) -> Result<(), HirDependencyCallSignatureError> {
    if actual != expected {
        let key = identities
            .canonical_key::<_, ExactTypeKey>(actual)
            .map_err(SharedTypeMetadataError::from)?;
        if let ExactTypeKey::Nominal(owner) = key.as_ref()
            && nominal_source(*owner).is_some_and(|nominal| {
                matches!(
                    nominal.source_shape(),
                    crate::NominalSourceShapeV1::Intrinsic(representation)
                        if representation.family() == crate::IntrinsicTypeKind::Nothing
                )
            })
        {
            return Ok(());
        }
        return Err(HirDependencyCallSignatureError::Argument {
            index,
            expected,
            actual,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
