//! Mechanical replay of physical layout exports from a sealed lowering pair.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use scoop_identity::{
    CborIdentityRecord, ExactTypeKey, LayoutKey, PersistentExactTypeId, PersistentLayoutId,
    RepresentationRole, ValidatedIdentityGraph,
};
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WirePath};

mod error;
mod fields;
mod instance;
mod physical;
mod source;
mod value;

pub use error::ExactLayoutLoweringError;
type Result<T> = std::result::Result<T, ExactLayoutLoweringError>;

/// Joins canonical MIR representation records to every emitted layout and
/// descriptor instance. The containing section separately closes HIR source,
/// intrinsic bindings, and selected-provider authority for dependency tables.
pub fn lower_exact_layout_exports(
    input: &mir::SingleConeStrongMirInput,
    output: &lir::SingleConeStrongLirOutput,
    types: &mir::CanonicalParamFreeMirTypeExportsV1,
    identities: &ValidatedIdentityGraph,
    dependencies: &[&lir::CanonicalExactLayoutExportsV1],
    meter: &mut BudgetMeter,
) -> Result<lir::CanonicalExactLayoutExportsV1> {
    if input.module().cone != output.foundation().producer() {
        return Err(ExactLayoutLoweringError::Provider);
    }
    let mut projection = Projection {
        module: input.module(),
        output,
        types,
        identities,
        dependencies,
        meter,
        roots: BTreeMap::new(),
        completed: BTreeMap::new(),
        active: BTreeSet::new(),
    };
    for (_, layout) in output.module().meta.layouts.iter() {
        projection.add_root(layout.identity.layout_record())?;
    }
    for (_, descriptor) in output.module().meta.type_descriptors.iter() {
        projection.add_root(descriptor.instance_layout.layout_record())?;
    }
    let mut roots = projection.reserve(projection.roots.len())?;
    roots.extend(projection.roots.values().cloned());
    for root in roots {
        projection.layout(root.exact_type(), root.representation(), 1)?;
    }
    let mut records = projection.reserve(projection.completed.len())?;
    records.extend(projection.completed.into_values());
    Ok(lir::CanonicalExactLayoutExportsV1::try_new(
        output.module().meta.target_profile,
        output.foundation(),
        records,
        meter,
    )?)
}

struct Projection<'a, 'meter> {
    module: &'a mir::Module,
    output: &'a lir::SingleConeStrongLirOutput,
    types: &'a mir::CanonicalParamFreeMirTypeExportsV1,
    identities: &'a ValidatedIdentityGraph,
    dependencies: &'a [&'a lir::CanonicalExactLayoutExportsV1],
    meter: &'meter mut BudgetMeter,
    roots: BTreeMap<PersistentLayoutId, LayoutKey>,
    completed: BTreeMap<PersistentLayoutId, lir::ExactLayoutExportV1>,
    active: BTreeSet<PersistentLayoutId>,
}

impl Projection<'_, '_> {
    fn add_root(&mut self, root: &CborIdentityRecord<PersistentLayoutId, LayoutKey>) -> Result<()> {
        self.lookup(self.roots.len())?;
        if root.key().target_profile() != &self.output.module().meta.target_profile.wire_id() {
            return Err(ExactLayoutLoweringError::Target);
        }
        if !self.roots.contains_key(&root.id()) {
            self.meter.charge_collection_slots(1, &WirePath::root())?;
            self.roots.insert(root.id(), root.key().clone());
        }
        Ok(())
    }

    fn layout(
        &mut self,
        exact: PersistentExactTypeId,
        role: RepresentationRole,
        depth: u64,
    ) -> Result<lir::ExactLayoutExportV1> {
        self.meter.check_semantic_depth(depth, &WirePath::root())?;
        self.meter.charge_work(1, &WirePath::root())?;
        let key = LayoutKey::new(
            exact,
            self.output.module().meta.target_profile.wire_id(),
            role,
        );
        let id = PersistentLayoutId::from_key(&key)?;
        self.lookup(self.completed.len())?;
        if let Some(value) = self.completed.get(&id) {
            return Ok(value.clone());
        }
        self.lookup(self.roots.len())?;
        if !self.roots.contains_key(&id) {
            return self.dependency(id);
        }
        self.lookup(self.active.len())?;
        if self.active.contains(&id) {
            return Err(ExactLayoutLoweringError::Cycle(id));
        }
        self.meter.charge_collection_slots(1, &WirePath::root())?;
        self.active.insert(id);
        let shape = self.shape(exact)?;
        let ty = self.physical_type(exact)?;
        self.validate_source(shape, &ty)?;
        let identity = lir::ExactLayoutIdentityV1::from_foundation(
            self.output.module().meta.target_profile,
            self.identities.canonical_record(exact)?,
            role,
            self.output.foundation(),
            self.meter,
        )?;
        let record = match role {
            RepresentationRole::ManagedValue | RepresentationRole::CValue => {
                self.value(identity, shape, depth + 1)?.into()
            }
            RepresentationRole::ManagedObject => self.instance(identity, shape, depth + 1)?.into(),
            _ => return Err(ExactLayoutLoweringError::Role(id)),
        };
        self.validate_physical(&record, shape, &ty)?;
        self.meter.charge_collection_slots(1, &WirePath::root())?;
        self.completed.insert(id, record.clone());
        self.active.remove(&id);
        Ok(record)
    }

    fn dependency(&mut self, id: PersistentLayoutId) -> Result<lir::ExactLayoutExportV1> {
        let mut found = None;
        for table in self.dependencies {
            self.lookup(table.records().len())?;
            if let Some(record) = table.get(id) {
                if table.target() != self.output.module().meta.target_profile {
                    return Err(ExactLayoutLoweringError::Target);
                }
                if found.is_some() || table.provider() == self.output.foundation().producer() {
                    return Err(ExactLayoutLoweringError::AmbiguousDependency(id));
                }
                found = Some(record.clone());
            }
        }
        found.ok_or(ExactLayoutLoweringError::MissingDependency(id))
    }

    fn value_dependency(
        &mut self,
        exact: PersistentExactTypeId,
        depth: u64,
    ) -> Result<Arc<lir::ExactValueLayoutV1>> {
        self.layout(exact, RepresentationRole::ManagedValue, depth)?
            .value_handle()
            .ok_or(ExactLayoutLoweringError::DependencyKind(exact))
    }
    fn reserve<T>(&mut self, count: usize) -> Result<Vec<T>> {
        let mut values = Vec::new();
        self.meter
            .try_reserve_collection_slots(&mut values, count, &WirePath::root())?;
        Ok(values)
    }
    fn lookup(&mut self, count: usize) -> Result<()> {
        self.meter
            .charge_work(u64::from(count.max(1).ilog2()) + 1, &WirePath::root())?;
        Ok(())
    }
}
