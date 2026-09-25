use super::*;
use scoop_identity::{CallableTemplateOrigin, ExactTypeKey};

mod callables;
mod nominal;
mod receiver;
mod singleton;
mod slots;

type Exports<'a> = exports::CheckedTypeSectionExportsV1<'a>;
type Error<E> = TypeSelectionValidationError<E>;

pub(super) fn validate<'a, F: TypeSectionFoundationSemanticAuthority<E>, E>(
    request: SelectedExternalTypeUseV1,
    local: &Exports<'a>,
    provider: &Exports<'a>,
    foundation: &F,

    path: &WirePath,
) -> Result<CheckedTypeSelectionTargetV1<'a>, Error<E>> {
    let exact = match request.usage() {
        SelectedTypeUseV1::Signature { exact }
        | SelectedTypeUseV1::Representation { exact }
        | SelectedTypeUseV1::TypeTest { exact }
        | SelectedTypeUseV1::ShapeSupport { exact } => exact,
        SelectedTypeUseV1::Construct { exact, declaration } => {
            callables::construction(provider, exact, declaration, foundation, path)?;
            exact
        }
        SelectedTypeUseV1::MemberCall {
            receiver,
            declaration,
        } => {
            let owner = callables::member(provider, declaration, foundation, path)?;
            receiver::validate(&local.graph, receiver, owner, path)?;
            owner
        }
        SelectedTypeUseV1::SlotCall { receiver, slot } => {
            slots::validate(local, provider, receiver, slot, path)?
        }
        SelectedTypeUseV1::SingletonValue { exact, value } => {
            singleton::validate(provider, exact, value, foundation)?;
            exact
        }
        SelectedTypeUseV1::Inheritance { derived, edge } => {
            direct_edge(local, provider, derived, edge)?
        }
    };
    let (definition, facts) = nominal::resolve(provider, exact, request, foundation)?;
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
) -> Result<PersistentExactTypeId, Error<E>> {
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

fn exact<E>(owner: SourceNominalId) -> Result<PersistentExactTypeId, Error<E>> {
    let SourceNominalId::Concrete(owner) = owner else {
        return Err(Error::DeclarationOwner);
    };
    let key = ExactTypeKey::Nominal(owner);

    PersistentExactTypeId::from_key(&key).map_err(|_| Error::DeclarationOwner)
}
