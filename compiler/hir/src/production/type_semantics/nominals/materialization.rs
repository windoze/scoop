//! Declaration-driven closure of the nominal representation/inheritance roots.

use super::*;
use scoop_wire::{BudgetMeter, WirePath};

mod dispatch;

pub(super) fn retain_closed(
    output: &Output,
    nominals: &mut Vec<ConcreteNominal<'_>>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let export = output.export.module();
    let mut closure = Closure::new(export, output.local.module(), nominals, meter)?;
    for (index, nominal) in nominals.iter().enumerate() {
        representation::visit_required_types(export, nominal.local, |ty| {
            let exact = export
                .type_identities
                .get(ty)
                .and_then(HirTypeIdentity::exact)
                .ok_or(Error::MissingExactIdentity)?;
            closure.require(index, exact.id())
        })?;
    }
    closure.dispatch(export)?;
    closure.propagate()?;
    let mut index = 0;
    nominals.retain(|_| {
        let keep = !closure.blocked[index];
        index += 1;
        keep
    });
    Ok(())
}

struct Closure<'a, 'm> {
    local: &'a LocalConcreteHir,
    roots: BTreeMap<PersistentExactTypeId, usize>,
    dependents: Vec<BTreeSet<usize>>,
    blocked: Vec<bool>,
    pending: Vec<usize>,
    meter: &'m mut BudgetMeter,
}

impl<'a, 'm> Closure<'a, 'm> {
    fn new(
        export: &ExportHir,
        local: &'a LocalConcreteHir,
        nominals: &[ConcreteNominal<'_>],
        meter: &'m mut BudgetMeter,
    ) -> Result<Self, Error> {
        let path = WirePath::root();
        meter
            .check_table_entries(nominals.len() as u64, &path)
            .map_err(resource)?;
        meter
            .charge_collection_slots((nominals.len() as u64).saturating_mul(3), &path)
            .map_err(resource)?;
        let mut roots = BTreeMap::new();
        for (index, nominal) in nominals.iter().enumerate() {
            inheritance::source_resources::work(meter, roots.len())?;
            if roots.insert(nominal.exact, index).is_some() {
                return Err(Error::ExactIdentityMismatch(nominal.exact));
            }
            if let NominalLocalId::Object(id) = nominal.local {
                let backing = &export.classes[export.objects[id].backing_class];
                let ty = export.class_applications[backing.self_application].canonical_type;
                let exact = export.type_identities[ty]
                    .exact()
                    .ok_or(Error::MissingExactIdentity)?
                    .id();
                meter.charge_collection_slots(1, &path).map_err(resource)?;
                if let Some(previous) = roots.insert(exact, index)
                    && previous != index
                {
                    return Err(Error::ExactIdentityMismatch(exact));
                }
            }
        }
        Ok(Self {
            local,
            roots,
            dependents: vec![BTreeSet::new(); nominals.len()],
            blocked: vec![false; nominals.len()],
            pending: Vec::new(),
            meter,
        })
    }

    fn require(&mut self, owner: usize, exact: PersistentExactTypeId) -> Result<(), Error> {
        self.visit(owner, exact, 1, &mut BTreeSet::new())
    }

    fn visit(
        &mut self,
        owner: usize,
        exact: PersistentExactTypeId,
        depth: u64,
        seen: &mut BTreeSet<PersistentExactTypeId>,
    ) -> Result<(), Error> {
        let path = WirePath::root();
        self.meter
            .check_semantic_depth(depth, &path)
            .map_err(resource)?;
        inheritance::source_resources::work(self.meter, seen.len())?;
        if seen.contains(&exact) {
            return Ok(());
        }
        self.meter
            .check_table_entries(seen.len() as u64 + 1, &path)
            .map_err(resource)?;
        self.meter
            .charge_collection_slots(1, &path)
            .map_err(resource)?;
        self.meter.charge_nodes(1, &path).map_err(resource)?;
        seen.insert(exact);
        inheritance::source_resources::work(self.meter, self.roots.len())?;
        if let Some(&dependency) = self.roots.get(&exact) {
            inheritance::source_resources::work(self.meter, self.dependents[dependency].len())?;
            if !self.dependents[dependency].contains(&owner) {
                self.meter
                    .charge_collection_slots(1, &path)
                    .map_err(resource)?;
                self.meter.charge_edges(1, &path).map_err(resource)?;
                self.dependents[dependency].insert(owner);
            }
            return Ok(());
        }
        let ty = self
            .local
            .exact_type_identities
            .type_for_identity(exact)
            .ok_or(Error::MissingConcreteType(exact))?;
        let local = self.local;
        let key = local.exact_type_identities[ty].key();
        match key {
            ExactTypeKey::NominalApplication { .. } => self.block(owner),
            ExactTypeKey::Nominal(_) => Ok(()),
            ExactTypeKey::Tuple(elements) => {
                self.sequence(elements.as_slice().len())?;
                for child in elements.as_slice() {
                    self.visit(owner, *child, depth + 1, seen)?;
                }
                Ok(())
            }
            ExactTypeKey::Function {
                parameters, result, ..
            }
            | ExactTypeKey::NativeFunctionPointer {
                parameters, result, ..
            } => {
                self.sequence(parameters.len().saturating_add(1))?;
                for child in parameters.iter().chain(std::iter::once(result)) {
                    self.visit(owner, *child, depth + 1, seen)?;
                }
                Ok(())
            }
            ExactTypeKey::RawPointer(pointee) => self.visit(owner, *pointee, depth + 1, seen),
        }
    }

    fn block(&mut self, owner: usize) -> Result<(), Error> {
        if !self.blocked[owner] {
            self.meter
                .try_reserve_collection_slots(&mut self.pending, 1, &WirePath::root())
                .map_err(resource)?;
            self.blocked[owner] = true;
            self.pending.push(owner);
        }
        Ok(())
    }

    fn propagate(&mut self) -> Result<(), Error> {
        while let Some(owner) = self.pending.pop() {
            self.sequence(self.dependents[owner].len())?;
            for dependent in std::mem::take(&mut self.dependents[owner]) {
                self.block(dependent)?;
            }
        }
        Ok(())
    }

    fn sequence(&mut self, count: usize) -> Result<(), Error> {
        let path = WirePath::root();
        self.meter
            .check_table_entries(count as u64, &path)
            .map_err(resource)?;
        self.meter
            .charge_work(count as u64, &path)
            .map_err(resource)
    }
}

fn resource(error: scoop_wire::WireError) -> Error {
    Error::SourceInventory(SourceInventoryError::Resource(error))
}
