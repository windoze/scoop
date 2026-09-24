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
use scoop_wire::{BudgetMeter, WirePath};

use super::SharedLirLayoutValidationError as Error;

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
    foundation: &lir::OdrFreeLirFoundation,
    identities: &ValidatedIdentityGraph,
    dependencies: &[&lir::CanonicalExactLayoutExportsV1],
    meter: &mut BudgetMeter,
) -> Result<lir::CanonicalExactLayoutExportsV1> {
    let mut replay = Replay {
        target,
        types,
        foundation,
        identities,
        meter,
        dependencies: BTreeMap::new(),
        completed: BTreeMap::new(),
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
            replay.layout(source.exact(), role, 1)?;
        }
        if matches!(
            source.representation(),
            mir::MirTypeRepresentationV1::Struct {
                c_layout: mir::MirTypeCLayoutPolicyV1::CLayout(_),
                ..
            }
        ) {
            replay.layout(source.exact(), RepresentationRole::CValue, 1)?;
        }
    }
    let mut records = replay.reserve(replay.completed.len())?;
    records.extend(replay.completed.into_values());
    Ok(lir::CanonicalExactLayoutExportsV1::try_new(
        target, foundation, records, meter,
    )?)
}

struct Replay<'a, 'm> {
    target: lir::LirTargetProfile,
    types: &'a mir::CanonicalParamFreeMirTypeExportsV1,
    foundation: &'a lir::OdrFreeLirFoundation,
    identities: &'a ValidatedIdentityGraph,
    meter: &'m mut BudgetMeter,
    dependencies: BTreeMap<PersistentLayoutId, &'a lir::ExactLayoutExportV1>,
    completed: BTreeMap<PersistentLayoutId, lir::ExactLayoutExportV1>,
    active: BTreeSet<PersistentLayoutId>,
}

impl<'a> Replay<'a, '_> {
    fn index_dependencies(
        &mut self,
        dependencies: &[&'a lir::CanonicalExactLayoutExportsV1],
    ) -> Result<()> {
        let mut providers = BTreeSet::new();
        for dependency in dependencies {
            self.lookup(providers.len())?;
            let provider = dependency.provider();
            if provider == self.foundation.producer() || providers.contains(&provider) {
                return Err(Error::DependencyProvider(provider));
            }
            if dependency.target() != self.target {
                return Err(Error::DependencyTarget(provider));
            }
            self.index_entry::<scoop_identity::ConeIdentity>()?;
            providers.insert(provider);
            for record in dependency.records() {
                let id = record.identity().layout();
                self.lookup(self.dependencies.len())?;
                if self.dependencies.contains_key(&id) {
                    return Err(Error::AmbiguousDependency(id));
                }
                self.index_entry::<(PersistentLayoutId, &lir::ExactLayoutExportV1)>()?;
                self.dependencies.insert(id, record);
            }
        }
        Ok(())
    }

    fn layout(
        &mut self,
        exact: PersistentExactTypeId,
        role: RepresentationRole,
        depth: u64,
    ) -> Result<lir::ExactLayoutExportV1> {
        let path = WirePath::root();
        self.meter.check_semantic_depth(depth, &path)?;
        self.meter.charge_work(1, &path)?;
        self.meter.charge_edges(1, &path)?;
        let key = LayoutKey::new(exact, self.target.wire_id(), role);
        let id = PersistentLayoutId::from_key(&key)?;
        self.lookup(self.completed.len())?;
        if let Some(record) = self.completed.get(&id) {
            return Ok(record.clone());
        }
        self.lookup(self.types.records().len())?;
        let Some(source) = self.types.get(exact) else {
            self.lookup(self.dependencies.len())?;
            return self
                .dependencies
                .get(&id)
                .map(|record| (*record).clone())
                .ok_or(Error::MissingDependency(id));
        };
        self.lookup(self.active.len())?;
        if self.active.contains(&id) {
            return Err(Error::Cycle(id));
        }
        self.meter.charge_nodes(1, &path)?;
        self.index_entry::<PersistentLayoutId>()?;
        self.active.insert(id);
        let identity = lir::ExactLayoutIdentityV1::from_foundation(
            self.target,
            self.identities.canonical_record(exact)?,
            role,
            self.foundation,
            self.meter,
        )?;
        let next = depth.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        let record: lir::ExactLayoutExportV1 = match role {
            RepresentationRole::ManagedValue | RepresentationRole::CValue => {
                self.value(identity, source, next)?.into()
            }
            RepresentationRole::ManagedObject => self.instance(identity, source, next)?.into(),
            _ => return Err(Error::Role(id)),
        };
        self.index_entry::<(PersistentLayoutId, lir::ExactLayoutExportV1)>()?;
        self.completed.insert(id, record.clone());
        self.active.remove(&id);
        Ok(record)
    }

    fn value_dependency(
        &mut self,
        exact: PersistentExactTypeId,
        depth: u64,
    ) -> Result<Arc<lir::ExactValueLayoutV1>> {
        self.layout(exact, RepresentationRole::ManagedValue, depth)?
            .value_handle()
            .ok_or(Error::DependencyKind(exact))
    }
}
