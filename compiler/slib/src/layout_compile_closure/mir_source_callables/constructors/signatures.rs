use super::*;
use SharedMirConstructorComponent as Component;
use scoop_identity::{CoreBuiltinNominal, SignatureTypeKey};

pub(super) fn validate(
    declaration: PersistentConstructorId,
    metadata: hir::SharedTypeMetadataV1<'_>,
    source: &hir::CallableDeclarationRecordV1,
    binding: &mir::ParamFreeMirCallableBindingV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    for length in [
        scoop_wire::encoded_length(source),
        scoop_wire::encoded_length(binding),
    ] {
        meter.charge_work(length.map_err(Error::Encoding)?, &WirePath::root())?;
    }
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
    let exact = metadata.signature_exact_type(&SignatureTypeKey::Nominal(owner), meter)?;
    lookup(
        metadata.public.nominal_interfaces().declaration_count(),
        meter,
    )?;
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
        let expected = metadata.signature_exact_type(source.value_type(), meter)?;
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
            metadata.signature_exact_type(
                &SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
                meter,
            )?,
            mir::GcEffect::Managed,
        ),
        hir::NominalSourceShapeV1::Struct(shape) => {
            // A primary has the unique constructor key for the complete field
            // sequence. Secondary constructors cannot reuse that signature.
            meter.charge_work(
                scoop_wire::encoded_length(nominal.source_shape()).map_err(Error::Encoding)?,
                &WirePath::root(),
            )?;
            let primary = parameters
                .iter()
                .map(|parameter| parameter.value_type())
                .eq(shape.fields().iter().map(|field| field.value_type()));
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
