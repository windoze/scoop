//! Target layout replay from MIR records, without physical module candidates.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use scoop_identity::{
    CborIdentityRecord, ExactTypeKey, LayoutKey, PersistentExactTypeId, PersistentLayoutId,
    RepresentationRole, ValidatedIdentityGraph,
};
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::WirePath;

use super::SharedLirLayoutValidationError as Error;

mod constituents;
mod enumeration;
mod fields;
mod instance;
mod resources;
mod value;

type Result<T> = std::result::Result<T, Error>;

/// Reconstructs only the exported layout constituents. The caller must supply
/// HIR-joined MIR records and layouts from its actual reachable dependencies.
/// This result grants no selected use, descriptor, ABI or artifact capability.
pub fn replay_shared_mir_layouts(
    target: lir::LirTargetProfile,
    types: &mir::CanonicalParamFreeMirTypeExportsV1,
    foundation: &lir::ConeLirFoundation,
    identities: &ValidatedIdentityGraph,
    dependencies: &[&lir::CanonicalExactLayoutExportsV1],
) -> Result<lir::CanonicalExactLayoutExportsV1> {
    let mut replay = Replay {
        target,
        types,
        foundation,
        identities,

        dependencies: BTreeMap::new(),
        completed: BTreeMap::new(),
        values: BTreeMap::new(),
        active: BTreeSet::new(),
    };
    replay.index_dependencies(dependencies)?;
    for source in types.records() {
        // Backing classes supply the source object's shape, not another
        // independently materialized value or object definition.
        if matches!(
            source.representation(),
            mir::MirTypeRepresentationV1::ObjectBacking { .. }
        ) {
            continue;
        }
        for role in [
            RepresentationRole::ManagedValue,
            RepresentationRole::ManagedObject,
        ] {
            replay.layout(source.exact(), role)?;
        }
        if matches!(
            source.representation(),
            mir::MirTypeRepresentationV1::Struct {
                c_layout: mir::MirTypeCLayoutPolicyV1::CLayout(_),
                ..
            }
        ) {
            replay.layout(source.exact(), RepresentationRole::CValue)?;
        }
    }
    let mut records = replay.reserve(replay.completed.len())?;
    records.extend(replay.completed.into_values());
    Ok(lir::CanonicalExactLayoutExportsV1::try_new(
        target, foundation, records,
    )?)
}

struct Replay<'a> {
    target: lir::LirTargetProfile,
    types: &'a mir::CanonicalParamFreeMirTypeExportsV1,
    foundation: &'a lir::ConeLirFoundation,
    identities: &'a ValidatedIdentityGraph,

    dependencies: BTreeMap<PersistentLayoutId, &'a lir::ExactLayoutExportV1>,
    completed: BTreeMap<PersistentLayoutId, lir::ExactLayoutExportV1>,
    values: BTreeMap<PersistentExactTypeId, lir::ValueLayoutConstituentV1>,
    active: BTreeSet<PersistentLayoutId>,
}

impl<'a> Replay<'a> {
    fn index_dependencies(
        &mut self,
        dependencies: &[&'a lir::CanonicalExactLayoutExportsV1],
    ) -> Result<()> {
        let mut providers = BTreeSet::new();
        for dependency in dependencies {
            let provider = dependency.provider();
            if provider == self.foundation.producer() || providers.contains(&provider) {
                return Err(Error::DependencyProvider(provider));
            }
            if dependency.target() != self.target {
                return Err(Error::DependencyTarget(provider));
            }

            providers.insert(provider);
            for record in dependency.records() {
                let id = record.identity().layout();

                if self.dependencies.contains_key(&id) {
                    return Err(Error::AmbiguousDependency(id));
                }

                self.dependencies.insert(id, record);
            }
        }
        Ok(())
    }

    fn layout(
        &mut self,
        exact: PersistentExactTypeId,
        role: RepresentationRole,
    ) -> Result<lir::ExactLayoutExportV1> {
        let key = LayoutKey::new(exact, self.target.wire_id(), role);
        let id = PersistentLayoutId::from_key(&key)?;

        if let Some(record) = self.completed.get(&id) {
            return Ok(record.clone());
        }

        let Some(source) = self.types.get(exact) else {
            return self
                .dependencies
                .get(&id)
                .map(|record| (*record).clone())
                .ok_or(Error::MissingDependency(id));
        };

        if self.active.contains(&id) {
            return Err(Error::Cycle(id));
        }

        self.active.insert(id);
        let identity = lir::ExactLayoutIdentityV1::from_foundation(
            self.target,
            self.identities.canonical_record(exact)?,
            role,
            self.foundation,
        )?;
        let record: lir::ExactLayoutExportV1 = match role {
            RepresentationRole::ManagedValue | RepresentationRole::CValue => {
                self.value(identity, source)?.into()
            }
            RepresentationRole::ManagedObject => self.instance(identity, source)?.into(),
            _ => return Err(Error::Role(id)),
        };

        self.completed.insert(id, record.clone());
        self.active.remove(&id);
        Ok(record)
    }

    fn value_dependency(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<Arc<lir::ExactValueLayoutV1>> {
        self.layout(exact, RepresentationRole::ManagedValue)?
            .value_handle()
            .ok_or(Error::DependencyKind(exact))
    }
}
