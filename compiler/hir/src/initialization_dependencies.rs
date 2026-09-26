//! Direct property initialization uses from executable HIR and shared metadata.

use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, ConeIdentity, GeneratedCallableKey,
    InitializationCallableRole, PersistentInitializationUnitId, PersistentPropertyAccessorId,
    ValidatedIdentityGraph,
};
use scoop_wire::WirePath;

mod committed;
mod errors;
mod properties;
mod shared;
use HirInitializationUseError as Error;
pub use errors::HirInitializationUseError;
pub(crate) use properties::accessor_initialization_unit;

/// A direct accessor use in one materialized initializer. Repeated occurrences
/// with the same four typed identities have the same initialization edge.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct HirPropertyInitializationUseV1 {
    local_unit: PersistentInitializationUnitId,
    provider: ConeIdentity,
    dependency_unit: PersistentInitializationUnitId,
    accessor: PersistentPropertyAccessorId,
}

impl HirPropertyInitializationUseV1 {
    pub const fn local_unit(self) -> PersistentInitializationUnitId {
        self.local_unit
    }
    pub const fn provider(self) -> ConeIdentity {
        self.provider
    }
    pub const fn dependency_unit(self) -> PersistentInitializationUnitId {
        self.dependency_unit
    }
    pub const fn accessor(self) -> PersistentPropertyAccessorId {
        self.accessor
    }
}

pub(crate) fn initializer_root(
    root: CallableMaterialization,
    identities: &ValidatedIdentityGraph,
    local_units: &[PersistentInitializationUnitId],
) -> Result<Option<PersistentInitializationUnitId>, Error> {
    if let scoop_identity::CallableTemplateOwner::Constructor(constructor) = root.template() {
        let source =
            identities.canonical_key::<_, scoop_identity::SourceDeclarationKey>(constructor)?;
        let Some(scoop_identity::DefinitionOwnerAtom::Type(owner)) =
            source.owners().owners().last()
        else {
            return Ok(None);
        };
        for unit in local_units {
            let key =
                identities.canonical_key::<_, scoop_identity::InitializationUnitKey>(*unit)?;
            if matches!(key.as_ref(), scoop_identity::InitializationUnitKey::Object(actual)
                | scoop_identity::InitializationUnitKey::Companion(actual) if actual == owner)
            {
                if root.context() != CallableMaterializationContext::NoSubstitution {
                    return Err(Error::InitializationRootContext(root));
                }
                return Ok(Some(*unit));
            }
        }
        return Ok(None);
    }
    let Some(generated) = root.generated_template() else {
        return Ok(None);
    };

    let key = identities.canonical_key::<_, GeneratedCallableKey>(generated)?;
    let GeneratedCallableKey::Initialization {
        unit,
        role: InitializationCallableRole::Initializer,
    } = key.as_ref()
    else {
        // Lexical bodies retain their own root, even inside an initializer.
        return Ok(None);
    };
    if root.context() != CallableMaterializationContext::NoSubstitution {
        return Err(Error::InitializationRootContext(root));
    }

    if !local_units.contains(unit) {
        return Err(Error::MissingLocalUnit(*unit));
    }
    Ok(Some(*unit))
}

fn push(
    uses: &mut Vec<HirPropertyInitializationUseV1>,
    record: HirPropertyInitializationUseV1,
    consumer: ConeIdentity,
) -> Result<(), Error> {
    if record.provider == consumer {
        return Err(Error::LocalProvider(consumer));
    }

    scoop_wire::allocation::try_reserve(uses, 1, &WirePath::root())?;
    uses.push(record);
    Ok(())
}

fn canonicalize(
    mut uses: Vec<HirPropertyInitializationUseV1>,
) -> Result<Vec<HirPropertyInitializationUseV1>, Error> {
    uses.sort_unstable();
    uses.dedup();
    Ok(uses)
}

fn local_unit_ids(
    units: impl ExactSizeIterator<Item = PersistentInitializationUnitId>,
) -> Result<Vec<PersistentInitializationUnitId>, Error> {
    let mut ids = Vec::new();

    scoop_wire::allocation::try_reserve(&mut ids, units.len(), &WirePath::root())?;
    ids.extend(units);
    Ok(ids)
}
