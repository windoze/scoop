use scoop_identity::{CallableTemplateOrigin, CoreBuiltinNominal};

use super::{facts::Replay, *};
use crate::{
    DeclaredVisibilityV1, NominalSourceShapeV1, PublicDeclarationOwnerV1, SourceNominalId,
};

pub(super) fn project(
    replay: &mut Replay<'_, '_>,
    materialization: &NominalMaterializationClosure,
) -> Result<(), Error> {
    let types = replay.types;
    for key in types.current.foundation.nominal_application_keys() {
        let exact = scoop_identity::PersistentExactTypeId::from_key(&key)
            .map_err(|error| Error::Key(error.to_string()))?;
        replay.visit(exact)?;
        let scoop_identity::ExactTypeKey::NominalApplication { origin, arguments } = key else {
            unreachable!("nominal application keys")
        };
        let declaration = types.nominal_declaration(SourceNominalId::GenericTemplate(origin))?;
        let bindings = [arguments.as_slice().to_vec()];
        for field in declaration.source_shape().declared_fields() {
            replay.visit(types.exact_with_bindings(field.value_type(), &bindings)?)?;
        }
    }
    for owner in materialization.sources() {
        if types.nominal_key(*owner)?.origin() != types.current.provider {
            return Err(Error::NominalOwner(*owner));
        }
        replay.visit(types.nominal_exact(*owner)?)?;
        let nominal = types.nominal(*owner)?;
        for field in nominal.source_shape().declared_fields() {
            replay.signature(field.value_type())?;
        }
        if let NominalSourceShapeV1::Enum(shape) = nominal.source_shape() {
            for variant in shape.variants() {
                for field in variant.fields() {
                    replay.signature(field.value_type())?;
                }
            }
        }
        for parent in nominal.exact_supertypes().values() {
            replay.signature(parent)?;
        }
    }
    for callable in types
        .current
        .public
        .callable_interfaces()
        .all_declarations()
    {
        if let PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(owner)) =
            callable.owner()
            && materialization.contains(owner)
            && matches!(
                callable.declaration(),
                CallableTemplateOrigin::Constructor(_)
            )
            && matches!(
                callable.declared_visibility(),
                DeclaredVisibilityV1::Public | DeclaredVisibilityV1::Protected
            )
        {
            for parameter in callable.parameters().parameters() {
                replay.signature(parameter.value_type())?;
            }
        }
    }
    for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
        let declaration = builtin.identity_record();
        if declaration.key().origin() == types.current.provider {
            replay.visit(types.nominal_exact(declaration.id())?)?;
        }
    }
    Ok(())
}
