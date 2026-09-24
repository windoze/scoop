use super::*;
use scoop_identity::{CallableTemplateOrigin, ExactTypeKey};

mod callables;
mod nominal;
mod receiver;
mod resources;
mod singleton;
mod slots;

type Exports<'a> = exports::CheckedTypeSectionExportsV1<'a>;
type Error<E> = TypeSelectionValidationError<E>;

pub(super) fn validate<'a, F: TypeSectionFoundationSemanticAuthority<E>, E>(
    request: SelectedExternalTypeUseV1,
    local: &Exports<'a>,
    provider: &Exports<'a>,
    foundation: &F,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<CheckedTypeSelectionTargetV1<'a>, Error<E>> {
    meter.charge_nodes(1, path)?;
    meter.charge_work(1, path)?;
    let exact = match request.usage() {
        SelectedTypeUseV1::Signature { exact }
        | SelectedTypeUseV1::Representation { exact }
        | SelectedTypeUseV1::TypeTest { exact }
        | SelectedTypeUseV1::ShapeSupport { exact } => exact,
        SelectedTypeUseV1::Construct { exact, declaration } => {
            callables::construction(provider, exact, declaration, foundation, meter, path)?;
            exact
        }
        SelectedTypeUseV1::MemberCall {
            receiver,
            declaration,
        } => {
            let owner = callables::member(provider, declaration, foundation, meter, path)?;
            receiver::validate(&local.graph, receiver, owner, meter, path)?;
            owner
        }
        SelectedTypeUseV1::SlotCall { receiver, slot } => {
            slots::validate(local, provider, receiver, slot, meter, path)?
        }
        SelectedTypeUseV1::SingletonValue { exact, value } => {
            singleton::validate(provider, exact, value, foundation, meter, path)?;
            exact
        }
        SelectedTypeUseV1::Inheritance { derived, edge } => {
            direct_edge(local, provider, derived, edge, meter, path)?
        }
    };
    let (definition, facts) = nominal::resolve(provider, exact, request, foundation, meter, path)?;
    Ok(CheckedTypeSelectionTargetV1 {
        request,
        definition,
        facts,
        public: provider.public,
        declarations: provider.protected,
    })
}

fn direct_edge<E>(
    local: &Exports<'_>,
    provider: &Exports<'_>,
    derived: PersistentExactTypeId,
    edge: SelectedDirectInheritanceEdgeV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<PersistentExactTypeId, Error<E>> {
    meter.charge_work(2, path)?;
    let actual = local
        .inheritance_records
        .get(&derived)
        .ok_or(Error::DirectEdge)?;
    let target = match edge {
        SelectedDirectInheritanceEdgeV1::ClassBase { exact }
            if actual.edges().direct_base() == (DirectClassBaseV1::ClassBase { exact }) =>
        {
            exact
        }
        SelectedDirectInheritanceEdgeV1::Interface { exact } => {
            meter.charge_work(
                (actual.edges().direct_interfaces().len() as u64 + 1).ilog2() as u64 + 1,
                path,
            )?;
            if actual
                .edges()
                .direct_interfaces()
                .binary_search(&exact)
                .is_err()
            {
                return Err(Error::DirectEdge);
            }
            exact
        }
        _ => return Err(Error::DirectEdge),
    };
    if provider.inheritance.table().get(target).is_none() {
        return Err(Error::DirectEdge);
    }
    Ok(target)
}

fn exact<E>(
    owner: SourceNominalId,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<PersistentExactTypeId, Error<E>> {
    let SourceNominalId::Concrete(owner) = owner else {
        return Err(Error::DeclarationOwner);
    };
    let key = ExactTypeKey::Nominal(owner);
    let bytes =
        PersistentExactTypeId::hash_stream_length(&key).map_err(|_| Error::DeclarationOwner)?;
    meter.charge_sha256(bytes, path)?;
    PersistentExactTypeId::from_key(&key).map_err(|_| Error::DeclarationOwner)
}
