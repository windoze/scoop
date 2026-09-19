use super::*;
use crate::TypeParameterBoundsV1;
use scoop_identity::SignatureTypeKey;

mod shape;

pub(super) fn validate<A: NestedNominalSemanticAuthority<E>, E>(
    owner: SourceNominalId,
    source: &ProtectedNestedSourceInterfaceV1,
    authority: &mut A,
    meter: &mut BudgetMeter,
) -> Result<(), NestedSourceSemanticError<E>> {
    use NestedSourceSemanticError as Error;
    compare(
        source.type_parameters(),
        authority
            .nominal_source_binders(owner)
            .map_err(Error::Foundation)?,
        meter,
    )?;
    compare(
        source.supertypes(),
        authority
            .nominal_source_supertypes(owner)
            .map_err(Error::Foundation)?,
        meter,
    )?;
    compare(
        source.constructors(),
        authority
            .nominal_source_constructors(owner)
            .map_err(Error::Foundation)?,
        meter,
    )?;
    compare(
        source.members(),
        authority
            .nominal_source_members(owner)
            .map_err(Error::Foundation)?,
        meter,
    )?;
    compare(
        source.children(),
        authority
            .nominal_source_children(owner)
            .map_err(Error::Foundation)?,
        meter,
    )?;
    compare(
        source.source_shape(),
        authority
            .nominal_source_shape(owner)
            .map_err(Error::Foundation)?,
        meter,
    )?;
    let scope = source.type_parameters().signature_scope(None);
    let path = WirePath::root();
    for binder in source.type_parameters().binders() {
        meter.charge_nodes(1, &path).map_err(Error::Resource)?;
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
    source
        .type_parameters()
        .validate_bound_semantics(None, authority)
        .map_err(Error::Binders)?;
    let mut class_seen = false;
    for supertype in source.supertypes().values() {
        scope
            .validate_signature_semantics_metered(supertype, authority, meter, &path)
            .map_err(Error::Signature)?;
        let kind = match supertype {
            SignatureTypeKey::Nominal(id) => authority
                .concrete_nominal_shape(*id)
                .map_err(Error::Foundation)?
                .kind(),
            SignatureTypeKey::NominalApplication { origin, .. } => authority
                .generic_nominal_shape(*origin)
                .map_err(Error::Foundation)?
                .kind(),
            _ => return Err(Error::Supertypes),
        };
        match kind {
            PublicNominalKindV1::Interface => {}
            PublicNominalKindV1::Class
                if !class_seen
                    && matches!(
                        source.kind(),
                        PublicNominalKindV1::Class | PublicNominalKindV1::Object
                    ) =>
            {
                class_seen = true
            }
            _ => return Err(Error::Supertypes),
        }
    }
    shape::validate(owner, source, authority, meter)
}
