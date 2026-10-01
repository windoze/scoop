use super::*;
use scoop_identity::{ExactCallableSignature, SignatureTypeKey};

pub(super) fn exact(
    declaration: Declaration,
    metadata: hir::SharedTypeMetadataV1<'_>,
    source: &hir::CallableDeclarationRecordV1,
    signature: &ExactCallableSignature,
) -> Result<(), Error> {
    Error::require(
        declaration,
        Component::Execution,
        signature.effect() == source.effects().execution(),
    )?;
    let receiver = match source.owner().nominal_owner() {
        Some(hir::SourceNominalId::Concrete(owner)) => {
            let exact = metadata.signature_exact_type(&SignatureTypeKey::Nominal(owner))?;

            Error::require(
                declaration,
                Component::Receiver,
                source.receiver().is_none(),
            )?;
            Some(exact)
        }
        Some(hir::SourceNominalId::GenericTemplate(_)) => {
            return Err(Error::Mismatch {
                declaration,
                component: Component::Receiver,
            });
        }
        None => source
            .receiver()
            .map(|ty| metadata.signature_exact_type(ty))
            .transpose()?,
    };
    Error::require(
        declaration,
        Component::Receiver,
        signature.receiver().into_option() == receiver,
    )?;
    let parameters = source.parameters().parameters();
    Error::require(
        declaration,
        Component::ParameterCount,
        signature.parameters().len() == parameters.len(),
    )?;
    for (index, (actual, source)) in signature.parameters().iter().zip(parameters).enumerate() {
        let expected = metadata.signature_exact_type(source.value_type())?;
        Error::require(
            declaration,
            Component::Parameter { index },
            *actual == expected,
        )?;
    }
    let expected = metadata.signature_exact_type(source.result())?;
    Error::require(
        declaration,
        Component::Result,
        signature.result() == expected,
    )
}

pub(super) fn binding(
    declaration: Declaration,
    metadata: hir::SharedTypeMetadataV1<'_>,
    source: &hir::CallableDeclarationRecordV1,
    binding: &mir::ParamFreeMirCallableBindingV1,
) -> Result<(), Error> {
    let semantic = binding.semantic_signature();
    Error::require(
        declaration,
        Component::Implementation,
        binding.implementation()
            == scoop_identity::CallableDefinitionOwner::Strong(declaration.implementation()),
    )?;
    Error::require(
        declaration,
        Component::LoweredSignature,
        semantic.exact().effect() == scoop_identity::Effect::Suspend
            || binding.lowered_signature() == semantic,
    )?;
    exact(declaration, metadata, source, semantic.exact())?;
    let gc = match source.effects().gc_effect() {
        scoop_identity::GcEffect::Managed => mir::GcEffect::Managed,
        scoop_identity::GcEffect::NoGc => mir::GcEffect::NoGc,
    };
    Error::require(declaration, Component::GcEffect, semantic.gc_effect() == gc)?;
    if source.modality() == hir::CallableModalityV1::Abstract {
        let mir::MirCallableLoweringRoleV1::PureVirtualTrap { slot } = binding.lowering_role()
        else {
            return Err(Error::Mismatch {
                declaration,
                component: Component::LoweringRole,
            });
        };

        Error::require(
            declaration,
            Component::TrapSlot,
            source.slot_relations().values().binary_search(slot).is_ok(),
        )
    } else {
        let role = match declaration {
            Declaration::Function(_) => mir::MirCallableLoweringRoleV1::Ordinary,
            Declaration::PropertyAccessor(_) => mir::MirCallableLoweringRoleV1::Accessor,
        };
        Error::require(
            declaration,
            Component::LoweringRole,
            *binding.lowering_role() == role,
        )
    }
}
