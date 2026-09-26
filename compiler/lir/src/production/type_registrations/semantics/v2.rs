//! Writer projection for `strong-production/4` descriptor semantics.

use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::PersistentExactTypeId;

use super::*;
use crate::{StrongTypeDescriptorRefV2, StrongTypeDispatchCallableRefV2};

impl StrongTypeDescriptorSemanticPlanSetV2 {
    /// Projects the complete V2 descriptor graph from final LIR. Ordinary
    /// references are accepted only when the supplied request-local
    /// layout/ABI selection retains both their terminal semantic record and
    /// physical import.
    pub fn from_module(
        module: &Module,
        selected: &crate::StrongProductionDependencySelectionV2<'_>,
    ) -> Result<Self, StrongTypeDescriptorSemanticPlanBuildError> {
        if selected.consumer() != module.cone {
            return Err(
                StrongTypeDescriptorSemanticPlanBuildError::SelectionConsumer {
                    expected: module.cone,
                    actual: selected.consumer(),
                },
            );
        }
        if selected.target_profile() != module.meta.target_profile {
            return Err(
                StrongTypeDescriptorSemanticPlanBuildError::SelectionTarget {
                    expected: module.meta.target_profile,
                    actual: selected.target_profile(),
                },
            );
        }
        for (_, descriptor) in module.meta.external_type_descriptors.iter() {
            validate_descriptor_selection(selected, *descriptor)?;
        }
        let mut canonical = BTreeMap::new();
        for (_, descriptor) in module.meta.type_descriptors.iter() {
            let plan = build_descriptor_v2(module, descriptor, selected)?;
            if canonical.insert(plan.exact_type, plan).is_some() {
                return Err(
                    StrongTypeDescriptorSemanticPlanBuildError::DuplicateExactType(
                        descriptor.identity.exact_type(),
                    ),
                );
            }
        }
        Ok(Self {
            producer: module.cone,
            target: module.meta.target_profile.wire_id(),
            descriptors: canonical.into_values().collect(),
        })
    }
}

fn build_descriptor_v2(
    module: &Module,
    descriptor: &TypeDescriptor,
    selected: &crate::StrongProductionDependencySelectionV2<'_>,
) -> Result<StrongTypeDescriptorSemanticPlanV2, StrongTypeDescriptorSemanticPlanBuildError> {
    let exact_type = descriptor.identity.exact_type();
    if descriptor.diagnostic_name.is_empty() {
        return Err(StrongTypeDescriptorSemanticPlanBuildError::EmptyDiagnosticName(exact_type));
    }
    if !descriptor
        .instance_layout
        .is_managed_instance_of(exact_type, module.meta.target_profile)
    {
        return Err(StrongTypeDescriptorSemanticPlanBuildError::InstanceLayoutMismatch(exact_type));
    }
    if !descriptor.vtable.belongs_to_exact_type(exact_type) {
        return Err(StrongTypeDescriptorSemanticPlanBuildError::VtableOwnerMismatch(exact_type));
    }
    validate_inline_scan(descriptor, &module.meta.layouts, &module.meta.arrays)?;

    let parent = descriptor
        .parent
        .map(|reference| descriptor_ref(module, reference))
        .transpose()?;
    let vtable = StrongTypeVtableSemanticPlanV2::from_artifact(
        descriptor.vtable.identity_record().id(),
        slots(module, exact_type, descriptor.vtable.slots(), selected)?,
    );
    let mut tables = BTreeSet::from([vtable.table()]);
    let mut interfaces = BTreeSet::new();
    let mut itables = Vec::with_capacity(descriptor.itables.len());
    for itable in &descriptor.itables {
        if !itable.belongs_to_exact_type(exact_type) {
            return Err(
                StrongTypeDescriptorSemanticPlanBuildError::ItableOwnerMismatch(exact_type),
            );
        }
        let interface = descriptor_ref(module, itable.interface())?;
        if !itable.belongs_to_interface_exact_type(interface.exact_type()) {
            return Err(
                StrongTypeDescriptorSemanticPlanBuildError::ItableInterfaceMismatch {
                    exact_type,
                    interface: interface.exact_type(),
                },
            );
        }
        let table = itable.identity_record().id();
        if !tables.insert(table) {
            return Err(
                StrongTypeDescriptorSemanticPlanBuildError::DuplicateDispatchTable {
                    exact_type,
                    table,
                },
            );
        }
        if !interfaces.insert(interface.exact_type()) {
            return Err(
                StrongTypeDescriptorSemanticPlanBuildError::DuplicateInterface {
                    exact_type,
                    interface: interface.exact_type(),
                },
            );
        }
        itables.push(StrongTypeItableSemanticPlanV2::from_artifact(
            table,
            interface,
            slots(module, exact_type, itable.slots(), selected)?,
        ));
    }

    Ok(StrongTypeDescriptorSemanticPlanV2::from_artifact(
        exact_type,
        descriptor.diagnostic_name.clone(),
        descriptor.instance_layout.layout_record().id(),
        descriptor.instance_layout.scan_record().id(),
        descriptor.instance_shape.clone(),
        descriptor.inline_scan,
        parent,
        vtable,
        itables,
    ))
}

fn descriptor_ref(
    module: &Module,
    reference: TypeDescriptorRef,
) -> Result<StrongTypeDescriptorRefV2, StrongTypeDescriptorSemanticPlanBuildError> {
    Ok(match reference {
        TypeDescriptorRef::Local(id) => {
            let index = id.into_raw().into_u32();
            if index as usize >= module.meta.type_descriptors.len() {
                return Err(
                    StrongTypeDescriptorSemanticPlanBuildError::MissingLocalDescriptor(index),
                );
            }
            let descriptor = &module.meta.type_descriptors[id];
            StrongTypeDescriptorRefV2::Local(descriptor.identity.exact_type())
        }
        TypeDescriptorRef::External(id) => {
            let index = id.into_raw().into_u32();
            if index as usize >= module.meta.external_type_descriptors.len() {
                return Err(
                    StrongTypeDescriptorSemanticPlanBuildError::MissingExternalDescriptor(index),
                );
            }
            let descriptor = module.meta.external_type_descriptors[id];
            StrongTypeDescriptorRefV2::DependencyExternal {
                provider: descriptor.provider(),
                exact: descriptor.target(),
            }
        }
    })
}

fn slots(
    module: &Module,
    exact_type: PersistentExactTypeId,
    entries: &[DispatchEntry],
    selected: &crate::StrongProductionDependencySelectionV2<'_>,
) -> Result<Vec<StrongTypeDispatchCallableRefV2>, StrongTypeDescriptorSemanticPlanBuildError> {
    entries
        .iter()
        .map(|entry| {
            Ok(match entry.callable {
                CallableRef::Local(id) => {
                    let function = module.functions.get(id.into_u32() as usize).ok_or(
                        StrongTypeDescriptorSemanticPlanBuildError::MissingLocalCallable {
                            exact_type,
                            index: id.into_u32(),
                        },
                    )?;
                    StrongTypeDispatchCallableRefV2::Local(function.callable_body.id())
                }
                CallableRef::Runtime(function) => {
                    StrongTypeDispatchCallableRefV2::Runtime(function)
                }
                CallableRef::External(id) => {
                    let index = id.into_raw().into_u32();
                    if index as usize >= module.meta.external_callables.len() {
                        return Err(
                            StrongTypeDescriptorSemanticPlanBuildError::MissingExternalCallable {
                                exact_type,
                                index,
                            },
                        );
                    }
                    let callable = &module.meta.external_callables[id];
                    validate_callable_selection(selected, callable)?;
                    StrongTypeDispatchCallableRefV2::DependencyExternal {
                        provider: callable.provider(),
                        body: callable.body(),
                    }
                }
            })
        })
        .collect()
}

fn validate_descriptor_selection(
    selected: &crate::StrongProductionDependencySelectionV2<'_>,
    descriptor: crate::ExternalTypeDescriptor,
) -> Result<(), StrongTypeDescriptorSemanticPlanBuildError> {
    let replayed = selected
        .materialize_type_descriptor(descriptor.provider(), descriptor.target())
        .map_err(StrongTypeDescriptorSemanticPlanBuildError::ExternalMaterialization)?;
    if replayed != descriptor {
        return Err(
            StrongTypeDescriptorSemanticPlanBuildError::DescriptorSelectionMismatch {
                provider: descriptor.provider(),
                exact: descriptor.target(),
            },
        );
    }
    Ok(())
}

fn validate_callable_selection(
    selected: &crate::StrongProductionDependencySelectionV2<'_>,
    callable: &crate::ExternalCallable,
) -> Result<(), StrongTypeDescriptorSemanticPlanBuildError> {
    let replayed = selected
        .materialize_callable(
            callable.provider(),
            callable.target(),
            callable.signature().clone(),
        )
        .map_err(StrongTypeDescriptorSemanticPlanBuildError::ExternalMaterialization)?;
    if replayed != *callable {
        return Err(
            StrongTypeDescriptorSemanticPlanBuildError::CallableSelectionMismatch {
                provider: callable.provider(),
                target: callable.target(),
            },
        );
    }
    Ok(())
}
