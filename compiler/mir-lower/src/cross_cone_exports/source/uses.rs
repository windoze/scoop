use super::*;
use MirTypeBridgeSourceProjectionError as Error;

mod shapes;

pub(super) fn project(
    input: MirTypeBridgeExportInputV1<'_>,
) -> Result<Vec<mir::MirTypeBridgeDependencyV1>, Error> {
    let module = input.mir.module();

    let mut uses = Vec::new();
    let local = &input.hir.output().local;
    for ty in local
        .materialized_type_closure()
        .map_err(Error::MaterializedTypes)?
    {
        let exact = &local.exact_type_identities[ty];
        let scoop_identity::ExactTypeKey::Nominal(source) = exact.key() else {
            continue;
        };
        let declaration = input
            .identities
            .canonical_key::<_, scoop_identity::SourceDeclarationKey>(*source)
            .map_err(Error::Identity)?;
        let provider = declaration.origin();
        if provider != module.cone {
            push(
                &mut uses,
                mir::MirTypeBridgeDependencyV1::new(
                    provider,
                    mir::MirTypeBridgeTargetV1::Type(exact.id()),
                ),
            )?;
        }
    }

    uses.sort_unstable();
    uses.dedup();
    let shared = input
        .public
        .external_references()
        .materialized_type_dependencies(module.cone, input.identities)
        .map_err(|source| Error::SharedTypeOccurrences(Box::new(source)))?;

    if !uses
        .iter()
        .map(|usage| (usage.provider(), usage.target()))
        .eq(shared
            .into_iter()
            .map(|(provider, exact)| (provider, mir::MirTypeBridgeTargetV1::Type(exact))))
    {
        return Err(Error::TypeOccurrenceInventory);
    }
    for shape in shapes::project(input)? {
        push(&mut uses, shape)?;
    }
    for root in input.mir.materialization().external_callable_roots() {
        if root.role() == mir::CallableRole::InitializationCycle
            || input.ordinary.selected().iter().any(|selected| {
                selected.provider() == root.provider()
                    && selected.declaration() == root.declaration()
                    && selected.implementation() == root.implementation()
                    && selected.signature() == root.signature()
            })
        {
            continue;
        }
        push(
            &mut uses,
            mir::MirTypeBridgeDependencyV1::new(
                root.provider(),
                mir::MirTypeBridgeTargetV1::Callable(root.implementation()),
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
    let path = WirePath::root();

    scoop_wire::allocation::try_reserve(uses, 1, &path)?;
    uses.push(relation);
    Ok(())
}
