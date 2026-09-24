use scoop_hir as hir;
use scoop_identity::{PersistentInitializationUnitId, PersistentObjectValueId, SignatureTypeKey};
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WirePath};
use std::collections::BTreeSet;

mod callables;
mod errors;
use SharedMirObjectComponent as Component;
use SharedMirObjectValidationError as Error;
pub use errors::{SharedMirObjectComponent, SharedMirObjectValidationError};

/// Joins local object values and their two generated initialization entries to
/// the shared source declaration, representation and original HIR unit key.
pub fn validate_shared_mir_objects(
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    callables: &mir::CanonicalMirCallableBindingsV1,
    objects: &mir::CanonicalMirObjectValuesV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let metadata = source.metadata();
    let units = metadata.object_initialization_units(meter)?;
    let mut required_values = BTreeSet::new();
    let mut required_units = BTreeSet::new();
    for representation in source.representations().table().records() {
        meter.charge_work(1, &WirePath::root())?;
        let hir::NominalRepresentationShapeV1::Object { backing_class, .. } =
            representation.shape()
        else {
            continue;
        };
        let owner = representation.owner();
        lookup(
            metadata.public.nominal_interfaces().declaration_count(),
            meter,
        )?;
        let nominal = metadata
            .public
            .nominal_interfaces()
            .declaration(hir::SourceNominalId::Concrete(owner))
            .ok_or(Error::MissingSource(owner))?;
        let hir::NominalSourceShapeV1::Object(shape) = nominal.source_shape() else {
            return Err(Error::MissingSource(owner));
        };
        let value = shape.value();
        lookup(units.len(), meter)?;
        let unit = *units.get(&owner).ok_or(Error::MissingUnit(owner))?;
        lookup(objects.records().len(), meter)?;
        let record = objects.get(value).ok_or(Error::MissingObject(value))?;
        let exact = metadata.signature_exact_type(&SignatureTypeKey::Nominal(owner), meter)?;
        let backing =
            metadata.signature_exact_type(&SignatureTypeKey::Nominal(*backing_class), meter)?;
        Error::object(
            value,
            Component::Provider,
            record.provider() == source.provider(),
        )?;
        Error::object(value, Component::Backing, record.backing() == backing)?;
        Error::object(
            value,
            Component::ReadPlan,
            record.read()
                == mir::MirObjectValueReadPlanV1::PublishedSingletonRoot { object: exact },
        )?;
        Error::object(value, Component::Unit, record.unit() == unit)?;
        let ensure = callables::validate(metadata, callables, unit, meter)?;
        Error::object(value, Component::Ensure, record.ensure() == ensure)?;
        insert(&mut required_values, value, meter)?;
        insert(&mut required_units, unit, meter)?;
    }
    for record in objects.records() {
        lookup(required_values.len(), meter)?;
        if !required_values.contains(&record.value()) {
            return Err(Error::UnexpectedObject(record.value()));
        }
    }
    for binding in callables.entries() {
        meter.charge_work(1, &WirePath::root())?;
        if let mir::MirCallableOriginV1::Generated {
            callable,
            role: scoop_identity::GeneratedCallableKey::Initialization { unit, .. },
        } = binding.origin()
        {
            lookup(required_units.len(), meter)?;
            if !required_units.contains(unit) {
                return Err(Error::UnexpectedCallable(*callable));
            }
        }
    }
    Ok(())
}

fn insert<T: Ord>(set: &mut BTreeSet<T>, value: T, meter: &mut BudgetMeter) -> Result<(), Error> {
    lookup(set.len(), meter)?;
    meter.check_table_entries(set.len() as u64 + 1, &WirePath::root())?;
    meter.charge_collection_slots(1, &WirePath::root())?;
    set.insert(value);
    Ok(())
}

fn lookup(length: usize, meter: &mut BudgetMeter) -> Result<(), Error> {
    Ok(meter.charge_work(u64::from(length.max(1).ilog2()) + 1, &WirePath::root())?)
}
