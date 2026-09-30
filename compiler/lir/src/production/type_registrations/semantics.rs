use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{
    ConeIdentity, PersistentDispatchTableId, PersistentExactTypeId, PersistentScanId,
};

use crate::{
    ArrayType, CallableRef, DispatchEntry, ExternalCallable, ExternalTypeDescriptor, Function,
    ItableRecord, Layout, LayoutKind, LirTargetProfile, Module, TypeDescriptor,
    TypeDescriptorInlineScanV1, TypeDescriptorRef,
};

mod references;
pub use references::*;
mod plans;
pub use plans::*;
mod v2;

struct DescriptorSemanticInputs<'a> {
    descriptors: &'a la_arena::Arena<TypeDescriptor>,
    external_descriptors: &'a la_arena::Arena<ExternalTypeDescriptor>,
    layouts: &'a la_arena::Arena<Layout>,
    arrays: &'a la_arena::Arena<ArrayType>,
    functions: &'a [Function],
    external_callables: &'a la_arena::Arena<ExternalCallable>,
}

impl StrongTypeDescriptorSemanticPlanSetV1 {
    pub fn from_module(
        module: &Module,
    ) -> Result<Self, StrongTypeDescriptorSemanticPlanBuildError> {
        Self::from_components(
            module.cone,
            module.meta.target_profile,
            DescriptorSemanticInputs {
                descriptors: &module.meta.type_descriptors,
                external_descriptors: &module.meta.external_type_descriptors,
                layouts: &module.meta.layouts,
                arrays: &module.meta.arrays,
                functions: &module.functions,
                external_callables: &module.meta.external_callables,
            },
        )
    }

    fn from_components(
        producer: ConeIdentity,
        target: LirTargetProfile,
        inputs: DescriptorSemanticInputs<'_>,
    ) -> Result<Self, StrongTypeDescriptorSemanticPlanBuildError> {
        let mut canonical = BTreeMap::new();
        for (_, descriptor) in inputs.descriptors.iter() {
            let plan = build_descriptor(target, descriptor, &inputs)?;
            if canonical.insert(plan.exact_type, plan).is_some() {
                return Err(
                    StrongTypeDescriptorSemanticPlanBuildError::DuplicateExactType(
                        descriptor.identity.exact_type(),
                    ),
                );
            }
        }
        Ok(Self {
            producer,
            target: target.wire_id(),
            descriptors: canonical.into_values().collect(),
        })
    }
}

fn build_descriptor(
    target: LirTargetProfile,
    descriptor: &TypeDescriptor,
    inputs: &DescriptorSemanticInputs<'_>,
) -> Result<StrongTypeDescriptorSemanticPlanV1, StrongTypeDescriptorSemanticPlanBuildError> {
    let exact_type = descriptor.identity.exact_type();
    if descriptor.diagnostic_name.is_empty() {
        return Err(StrongTypeDescriptorSemanticPlanBuildError::EmptyDiagnosticName(exact_type));
    }
    if !descriptor
        .instance_layout
        .is_managed_instance_of(exact_type, target)
    {
        return Err(StrongTypeDescriptorSemanticPlanBuildError::InstanceLayoutMismatch(exact_type));
    }
    if !descriptor.vtable.belongs_to_exact_type(exact_type) {
        return Err(StrongTypeDescriptorSemanticPlanBuildError::VtableOwnerMismatch(exact_type));
    }
    validate_inline_scan(descriptor, inputs.layouts, inputs.arrays)?;

    let parent = descriptor
        .parent
        .map(|reference| resolve_descriptor_ref(reference, inputs))
        .transpose()?;
    let vtable = StrongTypeVtableSemanticPlanV1 {
        table: descriptor.vtable.identity_record().id(),
        slots: resolve_slots(
            exact_type,
            descriptor.vtable.slots(),
            inputs.functions,
            inputs.external_callables,
        )?,
    };
    let mut tables = BTreeSet::from([vtable.table]);
    let mut interfaces = BTreeSet::new();
    let mut itables = Vec::with_capacity(descriptor.itables.len());
    for itable in &descriptor.itables {
        let semantic = build_itable(exact_type, itable, inputs)?;
        if !tables.insert(semantic.table) {
            return Err(
                StrongTypeDescriptorSemanticPlanBuildError::DuplicateDispatchTable {
                    exact_type,
                    table: semantic.table,
                },
            );
        }
        if !interfaces.insert(semantic.interface.exact_type()) {
            return Err(
                StrongTypeDescriptorSemanticPlanBuildError::DuplicateInterface {
                    exact_type,
                    interface: semantic.interface.exact_type(),
                },
            );
        }
        itables.push(semantic);
    }

    Ok(StrongTypeDescriptorSemanticPlanV1 {
        exact_type,
        diagnostic_name: descriptor.diagnostic_name.clone(),
        instance_layout: descriptor.instance_layout.layout_record().id(),
        instance_scan: descriptor.instance_layout.scan_record().id(),
        instance_shape: descriptor.instance_shape.clone(),
        inline_scan: descriptor.inline_scan,
        parent,
        vtable,
        itables,
        relations: descriptor.relations.clone().try_map(|reference| {
            reference
                .map(|reference| resolve_descriptor_ref(reference, inputs))
                .transpose()
        })?,
    })
}

fn validate_inline_scan(
    descriptor: &TypeDescriptor,
    layouts: &la_arena::Arena<Layout>,
    arrays: &la_arena::Arena<ArrayType>,
) -> Result<(), StrongTypeDescriptorSemanticPlanBuildError> {
    let exact_type = descriptor.identity.exact_type();
    let contains_reference = descriptor.instance_shape.inline_scan().contains_reference();
    let TypeDescriptorInlineScanV1::Defined(scan) = descriptor.inline_scan else {
        return if contains_reference {
            Err(StrongTypeDescriptorSemanticPlanBuildError::MissingInlineScan(exact_type))
        } else {
            Ok(())
        };
    };
    if !contains_reference {
        return Err(
            StrongTypeDescriptorSemanticPlanBuildError::UnexpectedInlineScan { exact_type, scan },
        );
    }
    let mut payloads = layouts
        .iter()
        .filter(|(_, layout)| layout.identity.scan_record().id() == scan)
        .map(|(_, layout)| match &layout.kind {
            LayoutKind::Plain { scan } | LayoutKind::Enum { scan } => scan.clone(),
            LayoutKind::Intrinsic(_) => crate::RefScan::None,
        })
        .chain(
            arrays
                .iter()
                .filter(|(_, array)| array.identity.scan_record().id() == scan)
                .map(|(_, array)| array.layout.instance().inline_scan().clone()),
        );
    let Some(payload) = payloads.next() else {
        return Err(
            StrongTypeDescriptorSemanticPlanBuildError::UnknownInlineScan { exact_type, scan },
        );
    };
    if payloads.next().is_some() {
        return Err(
            StrongTypeDescriptorSemanticPlanBuildError::DuplicateInlineScanDefinition {
                exact_type,
                scan,
            },
        );
    }
    if &payload != descriptor.instance_shape.inline_scan() {
        return Err(
            StrongTypeDescriptorSemanticPlanBuildError::InlineScanPayloadMismatch {
                exact_type,
                scan,
            },
        );
    }
    Ok(())
}

fn build_itable(
    exact_type: PersistentExactTypeId,
    itable: &ItableRecord,
    inputs: &DescriptorSemanticInputs<'_>,
) -> Result<StrongTypeItableSemanticPlanV1, StrongTypeDescriptorSemanticPlanBuildError> {
    if !itable.belongs_to_exact_type(exact_type) {
        return Err(StrongTypeDescriptorSemanticPlanBuildError::ItableOwnerMismatch(exact_type));
    }
    let interface = resolve_descriptor_ref(itable.interface(), inputs)?;
    if !itable.belongs_to_interface_exact_type(interface.exact_type()) {
        return Err(
            StrongTypeDescriptorSemanticPlanBuildError::ItableInterfaceMismatch {
                exact_type,
                interface: interface.exact_type(),
            },
        );
    }
    Ok(StrongTypeItableSemanticPlanV1 {
        table: itable.identity_record().id(),
        interface,
        slots: resolve_slots(
            exact_type,
            itable.slots(),
            inputs.functions,
            inputs.external_callables,
        )?,
    })
}

fn resolve_descriptor_ref(
    reference: TypeDescriptorRef,
    inputs: &DescriptorSemanticInputs<'_>,
) -> Result<StrongTypeDescriptorRefV1, StrongTypeDescriptorSemanticPlanBuildError> {
    match reference {
        TypeDescriptorRef::Local(id) => {
            let index = id.into_raw().into_u32();
            if index as usize >= inputs.descriptors.len() {
                return Err(
                    StrongTypeDescriptorSemanticPlanBuildError::MissingLocalDescriptor(index),
                );
            }
            Ok(StrongTypeDescriptorRefV1::Local(
                inputs.descriptors[id].identity.exact_type(),
            ))
        }
        TypeDescriptorRef::External(id) => {
            let index = id.into_raw().into_u32();
            if index as usize >= inputs.external_descriptors.len() {
                return Err(
                    StrongTypeDescriptorSemanticPlanBuildError::MissingExternalDescriptor(index),
                );
            }

            let descriptor = inputs.external_descriptors[id];
            Ok(StrongTypeDescriptorRefV1::DependencyExternal {
                provider: descriptor.provider(),
                exact: descriptor.target(),
            })
        }
    }
}

fn resolve_slots(
    exact_type: PersistentExactTypeId,
    slots: &[DispatchEntry],
    functions: &[Function],
    external_callables: &la_arena::Arena<ExternalCallable>,
) -> Result<Vec<StrongTypeDispatchCallableRefV1>, StrongTypeDescriptorSemanticPlanBuildError> {
    slots
        .iter()
        .map(|entry| match entry.callable {
            CallableRef::Local(id) => functions
                .get(id.into_u32() as usize)
                .map(|function| StrongTypeDispatchCallableRefV1::Local(function.callable_body.id()))
                .ok_or(
                    StrongTypeDescriptorSemanticPlanBuildError::MissingLocalCallable {
                        exact_type,
                        index: id.into_u32(),
                    },
                ),
            CallableRef::External(id) => {
                let index = id.into_raw().into_u32();
                if index as usize >= external_callables.len() {
                    return Err(
                        StrongTypeDescriptorSemanticPlanBuildError::MissingExternalCallable {
                            exact_type,
                            index,
                        },
                    );
                }
                Err(
                    StrongTypeDescriptorSemanticPlanBuildError::DependencyCallableInV1 {
                        exact_type,
                        index,
                    },
                )
            }
            CallableRef::Runtime(function) => {
                Ok(StrongTypeDispatchCallableRefV1::Runtime(function))
            }
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongTypeDescriptorSemanticPlanBuildError {
    DuplicateExactType(PersistentExactTypeId),
    EmptyDiagnosticName(PersistentExactTypeId),
    InstanceLayoutMismatch(PersistentExactTypeId),
    MissingInlineScan(PersistentExactTypeId),
    UnexpectedInlineScan {
        exact_type: PersistentExactTypeId,
        scan: PersistentScanId,
    },
    UnknownInlineScan {
        exact_type: PersistentExactTypeId,
        scan: PersistentScanId,
    },
    DuplicateInlineScanDefinition {
        exact_type: PersistentExactTypeId,
        scan: PersistentScanId,
    },
    InlineScanPayloadMismatch {
        exact_type: PersistentExactTypeId,
        scan: PersistentScanId,
    },
    VtableOwnerMismatch(PersistentExactTypeId),
    ItableOwnerMismatch(PersistentExactTypeId),
    ItableInterfaceMismatch {
        exact_type: PersistentExactTypeId,
        interface: PersistentExactTypeId,
    },
    DuplicateDispatchTable {
        exact_type: PersistentExactTypeId,
        table: PersistentDispatchTableId,
    },
    DuplicateInterface {
        exact_type: PersistentExactTypeId,
        interface: PersistentExactTypeId,
    },
    MissingLocalDescriptor(u32),
    MissingExternalDescriptor(u32),

    MissingLocalCallable {
        exact_type: PersistentExactTypeId,
        index: u32,
    },
    MissingExternalCallable {
        exact_type: PersistentExactTypeId,
        index: u32,
    },
    DependencyCallableInV1 {
        exact_type: PersistentExactTypeId,
        index: u32,
    },
}

impl fmt::Display for StrongTypeDescriptorSemanticPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong TypeDescriptor semantics: {self:?}"
        )
    }
}

impl std::error::Error for StrongTypeDescriptorSemanticPlanBuildError {}

#[cfg(test)]
mod tests;
