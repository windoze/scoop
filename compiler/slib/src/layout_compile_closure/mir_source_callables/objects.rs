use scoop_hir as hir;
use scoop_identity::{PersistentInitializationUnitId, PersistentObjectValueId, SignatureTypeKey};
use scoop_mir as mir;
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
) -> Result<(), Error> {
    let metadata = source.metadata();
    let units = metadata.object_initialization_units()?;
    let mut required_values = BTreeSet::new();
    let mut required_units = BTreeSet::new();
    for representation in source.representations().table().records() {
        let hir::NominalRepresentationShapeV1::Object { backing_class, .. } =
            representation.shape()
        else {
            continue;
        };
        let owner = representation.owner();

        let nominal = metadata
            .public
            .nominal_interfaces()
            .declaration(hir::SourceNominalId::Concrete(owner))
            .ok_or(Error::MissingSource(owner))?;
        let hir::NominalSourceShapeV1::Object(shape) = nominal.source_shape() else {
            return Err(Error::MissingSource(owner));
        };
        let value = shape.value();

        let unit = *units.get(&owner).ok_or(Error::MissingUnit(owner))?;

        let record = objects.get(value).ok_or(Error::MissingObject(value))?;
        let exact = metadata.signature_exact_type(&SignatureTypeKey::Nominal(owner))?;
        let backing = metadata.signature_exact_type(&SignatureTypeKey::Nominal(*backing_class))?;
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
        let ensure = callables::validate(metadata, callables, unit)?;
        Error::object(value, Component::Ensure, record.ensure() == ensure)?;
        insert(&mut required_values, value)?;
        insert(&mut required_units, unit)?;
    }
    for record in objects.records() {
        if !required_values.contains(&record.value()) {
            return Err(Error::UnexpectedObject(record.value()));
        }
    }
    for binding in callables.entries() {
        if let mir::MirCallableOriginV1::Generated {
            callable,
            role: scoop_identity::GeneratedCallableKey::Initialization { unit, .. },
        } = binding.origin()
        {
            if !required_units.contains(unit) {
                return Err(Error::UnexpectedCallable(*callable));
            }
        }
    }
    Ok(())
}

fn insert<T: Ord>(set: &mut BTreeSet<T>, value: T) -> Result<(), Error> {
    set.insert(value);
    Ok(())
}
