//! Final-LIR references built from the same complete dependency selection.

use super::*;
use scoop_identity::{ExactTypeKey, GeneratedNominalKey};

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
            mir::Type::Context(storage) => self
                .generated
                .contains_key(&mir::GeneratedExactTypeLocation::Context(*storage)),
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
    input: &mir::ConeMirInput,
    target: lir::LirTargetProfile,
    selected: Option<&lir::StrongProductionDependencySelectionV2<'_>>,
    external: &mut Arena<lir::ExternalTypeDescriptor>,
) -> Result<DependencyTypeDescriptors, LirLoweringError> {
    use LirLoweringError as Error;
    let mut result = DependencyTypeDescriptors::default();
    let roots = input
        .materialization()
        .dependency_generated_nominal_shapes();
    let module = input.module();
    if let Some(selected) = selected {
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
    }
    let mut supplied = std::collections::HashSet::new();
    for (_, descriptor) in external.iter() {
        if descriptor.provider() == module.cone
            || !supplied.insert(descriptor.target())
            || !module.meta.source_exact_types.iter().any(|source| {
                source.identity_record().id() == descriptor.target()
                    && source.owner() == mir::SourceExactTypeOwner::Cone(descriptor.provider())
            })
        {
            return Err(Error::DependencyDescriptorBinding(descriptor.target()));
        }
    }

    for root in roots {
        let selected = selected.ok_or(Error::MissingDependencyLayoutSelection {
            provider: root.provider(),
            exact: root.exact(),
        })?;
        let descriptor =
            if matches!(root.location(), mir::GeneratedExactTypeLocation::Context(_)) {
                selected.materialize_type_descriptor(root.provider(), root.exact())
            } else {
                selected.materialize_shape_type_descriptor(
                    root.provider(),
                    root.source(),
                    root.exact(),
                )
            }
            .map_err(Error::DependencyLayout)?;
        let id = intern(external, descriptor)?;
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
                    lir_type(module, boxed.payload()),
                )
                .map_err(Error::DependencyLayout)?;
            result.boxed.push((boxed.payload().clone(), descriptor));
        }
    }

    for source in module.meta.source_exact_types.iter() {
        if !matches!(
            source.ty(),
            mir::Type::Class(_) | mir::Type::Interface(_) | mir::Type::String
        ) {
            continue;
        }
        let (ExactTypeKey::Nominal(_), mir::SourceExactTypeOwner::Cone(provider)) =
            (source.identity_record().key(), source.owner())
        else {
            continue;
        };
        if provider == module.cone {
            continue;
        }
        let exact = source.identity_record().id();
        let existing = external.iter().find_map(|(id, descriptor)| {
            (descriptor.provider() == provider && descriptor.target() == exact).then_some(id)
        });
        let id = match existing {
            Some(id) => id,
            None => {
                let selected =
                    selected.ok_or(Error::MissingDependencyLayoutSelection { provider, exact })?;
                let descriptor = selected
                    .materialize_type_descriptor(provider, exact)
                    .map_err(Error::DependencyLayout)?;
                intern(external, descriptor)?
            }
        };

        result
            .source
            .push((source.ty().clone(), lir::TypeDescriptorRef::External(id)));
    }
    Ok(result)
}

fn intern(
    external: &mut Arena<lir::ExternalTypeDescriptor>,
    descriptor: lir::ExternalTypeDescriptor,
) -> Result<lir::ExternalTypeDescriptorId, LirLoweringError> {
    if let Some((id, existing)) = external
        .iter()
        .find(|(_, entry)| entry.target() == descriptor.target())
    {
        return if existing == &descriptor {
            Ok(id)
        } else {
            Err(LirLoweringError::DependencyDescriptorBinding(
                descriptor.target(),
            ))
        };
    }

    Ok(external.alloc(descriptor))
}
