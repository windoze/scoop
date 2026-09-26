use super::*;
use SharedMirConstructorComponent as Component;
use scoop_identity::{CoreBuiltinNominal, SignatureTypeKey};

pub(super) fn validate(
    declaration: PersistentConstructorId,
    metadata: hir::SharedTypeMetadataV1<'_>,
    source: &hir::CallableDeclarationRecordV1,
    binding: &mir::ParamFreeMirCallableBindingV1,
) -> Result<(), Error> {
    Error::require(
        declaration,
        Component::Implementation,
        binding.implementation() == StrongCallableDefinitionOwner::Constructor(declaration)
            && binding.origin() == &mir::MirCallableOriginV1::Constructor(declaration),
    )?;
    let Some(hir::SourceNominalId::Concrete(owner)) = source.owner().nominal_owner() else {
        return Err(Error::Mismatch {
            declaration,
            component: Component::Owner,
        });
    };
    let exact = metadata.signature_exact_type(&SignatureTypeKey::Nominal(owner))?;

    let nominal = metadata
        .public
        .nominal_interfaces()
        .declaration(hir::SourceNominalId::Concrete(owner))
        .ok_or(Error::Mismatch {
            declaration,
            component: Component::Owner,
        })?;
    let semantic = binding.semantic_signature().exact();
    Error::require(
        declaration,
        Component::Execution,
        semantic.effect() == source.effects().execution(),
    )?;
    Error::require(
        declaration,
        Component::Receiver,
        source.receiver().is_none() && !semantic.receiver().is_present(),
    )?;
    Error::require(
        declaration,
        Component::Result,
        source.result() == &SignatureTypeKey::Nominal(owner) && semantic.result() == exact,
    )?;
    let parameters = source.parameters().parameters();
    Error::require(
        declaration,
        Component::ParameterCount,
        semantic.parameters().len() == parameters.len(),
    )?;
    for (index, (actual, source)) in semantic.parameters().iter().zip(parameters).enumerate() {
        let expected = metadata.signature_exact_type(source.value_type())?;
        Error::require(
            declaration,
            Component::Parameter { index },
            *actual == expected,
        )?;
    }
    let source_gc = match source.effects().gc_effect() {
        scoop_identity::GcEffect::Managed => mir::GcEffect::Managed,
        scoop_identity::GcEffect::NoGc => mir::GcEffect::NoGc,
    };
    Error::require(
        declaration,
        Component::GcEffect,
        binding.semantic_signature().gc_effect() == source_gc,
    )?;
    let (role, receiver, result, lowered_gc) = match nominal.source_shape() {
        hir::NominalSourceShapeV1::Class(_) => (
            mir::MirCallableLoweringRoleV1::ClassInitializer { owner: exact },
            Some(exact),
            metadata.signature_exact_type(&SignatureTypeKey::Nominal(
                CoreBuiltinNominal::Unit.identity_record().id(),
            ))?,
            mir::GcEffect::Managed,
        ),
        hir::NominalSourceShapeV1::Struct(_) => {
            let primary =
                nominal.declaration_details().primary_value_constructor() == Some(declaration);
            if primary {
                (
                    mir::MirCallableLoweringRoleV1::PrimaryValueConstructor { owner: exact },
                    None,
                    exact,
                    mir::GcEffect::NoGc,
                )
            } else {
                (
                    mir::MirCallableLoweringRoleV1::ValueConstructor { owner: exact },
                    None,
                    exact,
                    source_gc,
                )
            }
        }
        hir::NominalSourceShapeV1::Interface
        | hir::NominalSourceShapeV1::Enum(_)
        | hir::NominalSourceShapeV1::Object(_)
        | hir::NominalSourceShapeV1::Intrinsic(_) => {
            return Err(Error::Mismatch {
                declaration,
                component: Component::Owner,
            });
        }
    };
    Error::require(
        declaration,
        Component::LoweringRole,
        *binding.lowering_role() == role,
    )?;
    let lowered = binding.lowered_signature().exact();
    Error::require(
        declaration,
        Component::LoweredSignature,
        lowered.effect() == semantic.effect()
            && lowered.receiver().into_option() == receiver
            && lowered.parameters() == semantic.parameters()
            && lowered.result() == result,
    )?;
    Error::require(
        declaration,
        Component::LoweredGcEffect,
        binding.lowered_signature().gc_effect() == lowered_gc,
    )
}
