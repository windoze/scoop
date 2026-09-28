//! Physical layout exports from complete MIR and LIR.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use scoop_identity::{
    CborIdentityRecord, ExactTypeKey, LayoutKey, PersistentExactTypeId, PersistentLayoutId,
    RepresentationRole, ValidatedIdentityGraph,
};
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::WirePath;

mod constituents;
mod error;
mod fields;
mod instance;
mod physical;
mod scope;
mod source;
mod value;

pub use error::ExactLayoutLoweringError;
type Result<T> = std::result::Result<T, ExactLayoutLoweringError>;

/// Publishes layouts and descriptors for the current materialization roots,
/// querying complete representations from actual dependencies where needed.
pub fn lower_exact_layout_exports(
    input: &mir::ConeMirInput,
    output: &lir::ConeLirOutput,
    types: &mir::CanonicalParamFreeMirTypeExportsV1,
    identities: &ValidatedIdentityGraph,
    dependencies: &[&lir::CanonicalExactLayoutExportsV1],
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

        roots: BTreeMap::new(),
        completed: BTreeMap::new(),
        values: BTreeMap::new(),
        active: BTreeSet::new(),
    };
    projection.collect_export_roots()?;
    let mut roots = projection.reserve(projection.roots.len())?;
    roots.extend(projection.roots.values().cloned());
    for root in roots {
        projection.layout(root.exact_type(), root.representation())?;
    }
    let mut records = projection.reserve(projection.completed.len())?;
    records.extend(projection.completed.into_values());
    Ok(lir::CanonicalExactLayoutExportsV1::try_new(
        output.module().meta.target_profile,
        output.foundation(),
        records,
    )?)
}

struct Projection<'a> {
    module: &'a mir::Module,
    output: &'a lir::ConeLirOutput,
    types: &'a mir::CanonicalParamFreeMirTypeExportsV1,
    identities: &'a ValidatedIdentityGraph,
    dependencies: &'a [&'a lir::CanonicalExactLayoutExportsV1],

    roots: BTreeMap<PersistentLayoutId, LayoutKey>,
    completed: BTreeMap<PersistentLayoutId, lir::ExactLayoutExportV1>,
    values: BTreeMap<PersistentExactTypeId, lir::ValueLayoutConstituentV1>,
    active: BTreeSet<PersistentLayoutId>,
}

impl Projection<'_> {
    fn add_root(&mut self, root: &CborIdentityRecord<PersistentLayoutId, LayoutKey>) -> Result<()> {
        if root.key().target_profile() != &self.output.module().meta.target_profile.wire_id() {
            return Err(ExactLayoutLoweringError::Target);
        }
        self.roots
            .entry(root.id())
            .or_insert_with(|| root.key().clone());
        Ok(())
    }

    fn layout(
        &mut self,
        exact: PersistentExactTypeId,
        role: RepresentationRole,
    ) -> Result<lir::ExactLayoutExportV1> {
        let key = LayoutKey::new(
            exact,
            self.output.module().meta.target_profile.wire_id(),
            role,
        );
        let id = PersistentLayoutId::from_key(&key)?;

        if let Some(value) = self.completed.get(&id) {
            return Ok(value.clone());
        }

        if !self.roots.contains_key(&id) {
            return self.dependency(id);
        }

        if self.active.contains(&id) {
            return Err(ExactLayoutLoweringError::Cycle(id));
        }

        self.active.insert(id);
        let shape = self.shape(exact)?;
        let ty = self.physical_type(exact)?;
        self.validate_source(shape, &ty)?;
        let identity = lir::ExactLayoutIdentityV1::from_foundation(
            self.output.module().meta.target_profile,
            self.identities.canonical_record(exact)?,
            role,
            self.output.foundation(),
        )?;
        let record = match role {
            RepresentationRole::ManagedValue | RepresentationRole::CValue => {
                self.value(identity, shape)?.into()
            }
            RepresentationRole::ManagedObject => self.instance(identity, shape)?.into(),
            _ => return Err(ExactLayoutLoweringError::Role(id)),
        };
        self.validate_physical(&record, shape, &ty)?;

        self.completed.insert(id, record.clone());
        self.active.remove(&id);
        Ok(record)
    }

    fn dependency(&mut self, id: PersistentLayoutId) -> Result<lir::ExactLayoutExportV1> {
        let mut found = None;
        for table in self.dependencies {
            if let Some(record) = table.get(id) {
                if table.target() != self.output.module().meta.target_profile {
                    return Err(ExactLayoutLoweringError::Target);
                }
                if found
                    .as_ref()
                    .is_some_and(|previous: &lir::ExactLayoutExportV1| {
                        !previous.has_same_odr_definition(record)
                    })
                    || table.provider() == self.output.foundation().producer()
                {
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
    ) -> Result<Arc<lir::ExactValueLayoutV1>> {
        self.layout(exact, RepresentationRole::ManagedValue)?
            .value_handle()
            .ok_or(ExactLayoutLoweringError::DependencyKind(exact))
    }
    fn reserve<T>(&mut self, count: usize) -> Result<Vec<T>> {
        let mut values = Vec::new();
        scoop_wire::allocation::try_reserve(&mut values, count, &WirePath::root())?;
        Ok(values)
    }
}
