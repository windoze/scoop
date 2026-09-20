use super::*;
use scoop_identity::{DuplicateSignatureKey, SignatureTypeKey};

pub(super) fn validate(
    nominals: &BoundNominalSourceContractsV1<'_, '_>,
    nominal: &NominalSourceContractV1,
    key: &SourceDeclarationKey,
    record: &NominalSupportConstructorInterfaceV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let declaration = record.declaration();
    let payload = record.payload();
    let path = WirePath::root();
    if nominal.owner() != payload.owner()
        || !matches!(
            nominal.kind(),
            PublicNominalKindV1::Class | PublicNominalKindV1::Struct
        )
    {
        return Err(invalid(
            declaration,
            "constructor differs from its nominal inventory owner",
        ));
    }
    super::access::validate(nominals.foundation, key, record, meter)?;
    let effects = payload.effects();
    if effects.execution() != scoop_identity::Effect::Ordinary
        || effects.implementation() != CallableImplementationV1::Scoop
        || effects.operator_role() != CallableOperatorRoleV1::None
        || effects.infix() != CallableInfixV1::Ordinary
        || (nominal.kind() == PublicNominalKindV1::Class
            && effects.gc_effect() != scoop_identity::GcEffect::Managed)
    {
        return Err(invalid(declaration, "invalid source constructor effects"));
    }
    let DuplicateSignatureKey::Constructor { parameters } = key.duplicate_signature() else {
        return Err(invalid(
            declaration,
            "source key has another declaration kind",
        ));
    };
    let actual = payload.parameters().parameters();
    meter.check_table_entries(actual.len() as u64, &path)?;
    meter.charge_work(actual.len() as u64, &path)?;
    if parameters.len() != actual.len() {
        return Err(invalid(
            declaration,
            "constructor parameter arity differs from source key",
        ));
    }
    let scope = nominal.type_parameters().signature_scope(None);
    let mut shapes = super::signatures::Shapes(nominals);
    for (expected, actual) in parameters.iter().zip(actual) {
        if !NominalRepresentationSupportV1::signature_types_match_metered(
            expected,
            actual.value_type(),
            3,
            meter,
            &path,
        )? {
            return Err(invalid(
                declaration,
                "constructor parameter type differs from source key",
            ));
        }
        super::signatures::validate(declaration, &scope, actual.value_type(), &mut shapes, meter)?;
    }
    super::signatures::validate(declaration, &scope, payload.result(), &mut shapes, meter)?;
    let arity = nominal.type_parameters().len_u32();
    meter.charge_work(u64::from(arity) + 1, &path)?;
    let matches = match (nominal.owner(), payload.result()) {
        (SourceNominalId::Concrete(owner), SignatureTypeKey::Nominal(actual)) => owner == *actual,
        (SourceNominalId::GenericTemplate(owner), SignatureTypeKey::NominalApplication { origin, arguments }) => {
            owner == *origin && arguments.as_slice().len() == arity as usize
                && arguments.as_slice().iter().enumerate().all(|(index, argument)| {
                    matches!(argument, SignatureTypeKey::Binder { depth: 0, index: actual } if *actual as usize == index)
                })
        }
        _ => false,
    };
    if !matches {
        return Err(invalid(
            declaration,
            "constructor result differs from its owner self type",
        ));
    }
    Ok(())
}
