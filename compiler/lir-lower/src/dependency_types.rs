//! Final-LIR references built from the same complete dependency selection.

use super::*;
use scoop_identity::{ExactTypeKey, GeneratedNominalKey};
use scoop_wire::{BudgetMeter, WirePath};

#[derive(Default)]
pub(crate) struct DependencyTypeDescriptors {
    pub source: Vec<(mir::Type, lir::TypeDescriptorRef)>,
    pub generated: HashMap<mir::GeneratedExactTypeLocation, lir::TypeDescriptorRef>,
    pub boxed: Vec<(mir::Type, lir::BoxedValueDescriptor)>,
}

impl DependencyTypeDescriptors {
    pub fn contains(&self, ty: &mir::Type) -> bool {
        if self.source.iter().any(|(source, _)| source == ty) {
            return true;
        }
        match ty {
            mir::Type::Class(id) => self
                .generated
                .contains_key(&mir::GeneratedExactTypeLocation::Class(*id)),
            mir::Type::Enum(id, _) => self
                .generated
                .contains_key(&mir::GeneratedExactTypeLocation::Enum(*id)),
            _ => false,
        }
    }
}

pub(super) fn lower(
    input: &mir::SingleConeStrongMirInput,
    target: lir::LirTargetProfile,
    selected: Option<&lir::StrongProductionDependencySelectionV2<'_>>,
    external: &mut Arena<lir::ExternalTypeDescriptor>,
    meter: &mut BudgetMeter,
) -> Result<DependencyTypeDescriptors, StrongLirLoweringError> {
    use StrongLirLoweringError as Error;
    let mut result = DependencyTypeDescriptors::default();
    let roots = input
        .materialization()
        .dependency_generated_nominal_shapes();
    let Some(selected) = selected else {
        if let Some(root) = roots.first() {
            return Err(Error::MissingDependencyLayoutSelection {
                provider: root.provider(),
                exact: root.exact(),
            });
        }
        return Ok(result);
    };
    let module = input.module();
    if selected.consumer() != module.cone {
        return Err(Error::DependencyLayoutConsumer {
            expected: module.cone,
            actual: selected.consumer(),
        });
    }
    if selected.target_profile() != target {
        return Err(Error::DependencyLayoutTarget {
            expected: target,
            actual: selected.target_profile(),
        });
    }
    meter
        .charge_work(roots.len() as u64, &WirePath::root())
        .map_err(resource)?;
    meter
        .charge_collection_slots((roots.len() as u64).saturating_mul(2), &WirePath::root())
        .map_err(resource)?;
    for root in roots {
        let descriptor = selected
            .materialize_shape_type_descriptor(root.provider(), root.source(), root.exact(), meter)
            .map_err(Error::DependencyLayout)?;
        let id = intern(external, descriptor, meter)?;
        result
            .generated
            .insert(root.location(), lir::TypeDescriptorRef::External(id));
        let identity = module
            .meta
            .generated_exact_types
            .get(root.location())
            .ok_or(Error::DependencyDescriptorBinding(root.exact()))?;
        if matches!(
            identity.nominal_record().key(),
            GeneratedNominalKey::BoxedValue { .. }
        ) {
            meter
                .charge_work(module.meta.boxed_types.len() as u64, &WirePath::root())
                .map_err(resource)?;
            let boxed = module
                .meta
                .boxed_types
                .iter()
                .find(|boxed| {
                    root.location() == mir::GeneratedExactTypeLocation::Class(boxed.class())
                })
                .ok_or(Error::DependencyDescriptorBinding(root.exact()))?;
            if exact_type_record(module, boxed.payload()).id() != root.source_exact() {
                return Err(Error::DependencyDescriptorBinding(root.exact()));
            }
            let descriptor = selected
                .materialize_boxed_value_descriptor(
                    root.provider(),
                    root.source(),
                    external,
                    id,
                    lir_type(boxed.payload()),
                    meter,
                )
                .map_err(Error::DependencyLayout)?;
            result.boxed.push((boxed.payload().clone(), descriptor));
        }
    }
    meter
        .charge_work(
            module.meta.source_exact_types.len() as u64,
            &WirePath::root(),
        )
        .map_err(resource)?;
    for source in module.meta.source_exact_types.iter() {
        if !matches!(
            source.ty(),
            mir::Type::Class(_) | mir::Type::Interface(_) | mir::Type::String
        ) {
            continue;
        }
        let (ExactTypeKey::Nominal(nominal), mir::SourceExactTypeOwner::Cone(provider)) =
            (source.identity_record().key(), source.owner())
        else {
            continue;
        };
        if provider == module.cone
            || selected
                .selected_shape_support(provider, *nominal, meter)
                .map_err(resource)?
                .is_none()
        {
            continue;
        }
        let exact = source.identity_record().id();
        let descriptor = selected
            .materialize_shape_type_descriptor(provider, *nominal, exact, meter)
            .map_err(Error::DependencyLayout)?;
        let id = intern(external, descriptor, meter)?;
        meter
            .charge_collection_slots(1, &WirePath::root())
            .map_err(resource)?;
        result
            .source
            .push((source.ty().clone(), lir::TypeDescriptorRef::External(id)));
    }
    Ok(result)
}

fn intern(
    external: &mut Arena<lir::ExternalTypeDescriptor>,
    descriptor: lir::ExternalTypeDescriptor,
    meter: &mut BudgetMeter,
) -> Result<lir::ExternalTypeDescriptorId, StrongLirLoweringError> {
    meter
        .charge_work(external.len() as u64, &WirePath::root())
        .map_err(resource)?;
    if let Some((id, existing)) = external
        .iter()
        .find(|(_, entry)| entry.target() == descriptor.target())
    {
        return if existing == &descriptor {
            Ok(id)
        } else {
            Err(StrongLirLoweringError::DependencyDescriptorBinding(
                descriptor.target(),
            ))
        };
    }
    meter
        .charge_collection_slots(1, &WirePath::root())
        .map_err(resource)?;
    Ok(external.alloc(descriptor))
}

fn resource(error: scoop_wire::WireError) -> StrongLirLoweringError {
    StrongLirLoweringError::DependencyLayout(lir::LayoutExternalMaterializationError::Resource(
        error,
    ))
}
