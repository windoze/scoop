use super::{
    NominalSourceCallablePayloadV1, ProtectedCallableSemanticAuthority,
    ProtectedCallableSemanticError as Error,
};
use crate::TypeParameterBoundsV1;
use scoop_identity::SignatureTypeKey;
use scoop_wire::{BudgetMeter, WirePath};

pub(in crate::cross_cone_type_semantics::protected_interfaces) fn validate_types<
    A: ProtectedCallableSemanticAuthority<E>,
    E,
>(
    payload: &NominalSourceCallablePayloadV1,
    owner_arity: u32,
    authority: &mut A,
    meter: &mut BudgetMeter,
) -> Result<(), Error<E>> {
    let outer = (owner_arity != 0).then_some(owner_arity);
    let scope = payload.type_parameters().signature_scope(outer);
    let path = WirePath::root();
    meter
        .charge_work(payload.type_parameters().len_u32() as u64, &path)
        .map_err(Error::Resource)?;
    for binder in payload.type_parameters().binders() {
        if let TypeParameterBoundsV1::Nominal(bounds) = binder.bounds() {
            for bound in bounds
                .class()
                .into_iter()
                .chain(bounds.interfaces().values())
            {
                scope
                    .validate_signature_semantics_metered(bound, authority, meter, &path)
                    .map_err(Error::Signature)?;
            }
        }
    }
    payload
        .type_parameters()
        .validate_bound_semantics(outer, authority)
        .map_err(Error::Binders)?;
    for value in payload
        .parameters()
        .parameters()
        .iter()
        .map(|parameter| parameter.value_type())
        .chain(std::iter::once(payload.result()))
    {
        scope
            .validate_signature_semantics_metered(value, authority, meter, &path)
            .map_err(Error::Signature)?;
    }
    Ok(())
}

pub(super) fn parameters_match(
    payload: &NominalSourceCallablePayloadV1,
    expected: &[SignatureTypeKey],
) -> bool {
    payload.parameters().parameters().len() == expected.len()
        && payload
            .parameters()
            .parameters()
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual.value_type() == expected)
}
