use super::*;
use MirTypeBridgeUseLoweringError as Error;

pub(super) fn project(
    input: MirTypeBridgeExportInputV1<'_>,
) -> Result<Vec<mir::MirTypeBridgeDependencyV1>, Error> {
    let provider = input.mir.module().cone;
    let references = input.public.external_references();
    let mut uses = Vec::new();
    for usage in references
        .materialized_singleton_uses(provider, input.identities)
        .map_err(|error| Error::SharedTypeOccurrences(Box::new(error)))?
    {
        push(
            &mut uses,
            mir::MirTypeBridgeDependencyV1::new(
                usage.provider,
                mir::MirTypeBridgeTargetV1::Object(usage.value),
            ),
        )?;
    }
    for (provider, exact) in references
        .materialized_type_dependencies(provider, input.identities)
        .map_err(|error| Error::SharedTypeOccurrences(Box::new(error)))?
    {
        push(
            &mut uses,
            mir::MirTypeBridgeDependencyV1::new(provider, mir::MirTypeBridgeTargetV1::Type(exact)),
        )?;
    }
    for (provider, owner) in references
        .materialized_shape_dependencies(provider, input.identities)
        .map_err(|error| Error::SharedTypeOccurrences(Box::new(error)))?
    {
        push(
            &mut uses,
            mir::MirTypeBridgeDependencyV1::new(
                provider,
                mir::MirTypeBridgeTargetV1::ShapeSupport(owner),
            ),
        )?;
    }
    for root in input.mir.materialization().external_callable_roots() {
        if input.ordinary.selected().iter().any(|selected| {
            selected.provider() == root.provider()
                && selected.implementation() == root.implementation()
                && selected.signature() == root.signature()
        }) {
            continue;
        }
        push(
            &mut uses,
            mir::MirTypeBridgeDependencyV1::new(
                root.provider(),
                mir::MirTypeBridgeTargetV1::Callable(
                    scoop_identity::CallableDefinitionOwner::Strong(root.implementation()),
                ),
            ),
        )?;
    }
    uses.sort_unstable();
    uses.dedup();
    Ok(uses)
}

fn push(
    uses: &mut Vec<mir::MirTypeBridgeDependencyV1>,
    relation: mir::MirTypeBridgeDependencyV1,
) -> Result<(), Error> {
    scoop_wire::allocation::try_reserve(uses, 1, &WirePath::root())?;
    uses.push(relation);
    Ok(())
}
