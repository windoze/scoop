use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{
    ConeIdentity, PersistentCallableBodyId, PersistentDispatchTableId, PersistentExactTypeId,
    PersistentLayoutId, PersistentScanId,
};

use crate::{
    ArrayType, CallableRef, CoreExternalCallable, CoreExternalTypeDescriptor, DispatchEntry,
    Function, ItableRecord, Layout, LayoutKind, LirTargetProfile, Module, RuntimeFunction,
    TypeDescriptor, TypeDescriptorInlineScanV1, TypeDescriptorRef, TypeInstanceShapeV1,
};

/// Typed origin of a descriptor pointer stored inside a local TypeDescriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongTypeDescriptorRefV1 {
    Local(PersistentExactTypeId),
    CoreExternal(PersistentExactTypeId),
}

impl StrongTypeDescriptorRefV1 {
    pub const fn exact_type(self) -> PersistentExactTypeId {
        match self {
            Self::Local(exact_type) | Self::CoreExternal(exact_type) => exact_type,
        }
    }
}

/// Typed semantic target of one vtable or itable slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongTypeDispatchCallableRefV1 {
    Local(PersistentCallableBodyId),
    CoreExternal(PersistentCallableBodyId),
    Runtime(RuntimeFunction),
}

/// Canonical semantic payload of one exact type's vtable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongTypeVtableSemanticPlanV1 {
    table: PersistentDispatchTableId,
    slots: Vec<StrongTypeDispatchCallableRefV1>,
}

impl StrongTypeVtableSemanticPlanV1 {
    pub(crate) const fn from_artifact(
        table: PersistentDispatchTableId,
        slots: Vec<StrongTypeDispatchCallableRefV1>,
    ) -> Self {
        Self { table, slots }
    }

    pub const fn table(&self) -> PersistentDispatchTableId {
        self.table
    }

    pub fn slots(&self) -> &[StrongTypeDispatchCallableRefV1] {
        &self.slots
    }
}

/// Canonical semantic payload of one exact type's implementation table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongTypeItableSemanticPlanV1 {
    table: PersistentDispatchTableId,
    interface: StrongTypeDescriptorRefV1,
    slots: Vec<StrongTypeDispatchCallableRefV1>,
}

impl StrongTypeItableSemanticPlanV1 {
    pub(crate) const fn from_artifact(
        table: PersistentDispatchTableId,
        interface: StrongTypeDescriptorRefV1,
        slots: Vec<StrongTypeDispatchCallableRefV1>,
    ) -> Self {
        Self {
            table,
            interface,
            slots,
        }
    }

    pub const fn table(&self) -> PersistentDispatchTableId {
        self.table
    }

    pub const fn interface(&self) -> StrongTypeDescriptorRefV1 {
        self.interface
    }

    pub fn slots(&self) -> &[StrongTypeDispatchCallableRefV1] {
        &self.slots
    }
}

/// Arena-independent semantic definition of one local M23 TypeDescriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongTypeDescriptorSemanticPlanV1 {
    exact_type: PersistentExactTypeId,
    diagnostic_name: String,
    instance_layout: PersistentLayoutId,
    instance_scan: PersistentScanId,
    instance_shape: TypeInstanceShapeV1,
    inline_scan: TypeDescriptorInlineScanV1,
    parent: Option<StrongTypeDescriptorRefV1>,
    vtable: StrongTypeVtableSemanticPlanV1,
    itables: Vec<StrongTypeItableSemanticPlanV1>,
}

impl StrongTypeDescriptorSemanticPlanV1 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_artifact(
        exact_type: PersistentExactTypeId,
        diagnostic_name: String,
        instance_layout: PersistentLayoutId,
        instance_scan: PersistentScanId,
        instance_shape: TypeInstanceShapeV1,
        inline_scan: TypeDescriptorInlineScanV1,
        parent: Option<StrongTypeDescriptorRefV1>,
        vtable: StrongTypeVtableSemanticPlanV1,
        itables: Vec<StrongTypeItableSemanticPlanV1>,
    ) -> Self {
        Self {
            exact_type,
            diagnostic_name,
            instance_layout,
            instance_scan,
            instance_shape,
            inline_scan,
            parent,
            vtable,
            itables,
        }
    }

    pub const fn exact_type(&self) -> PersistentExactTypeId {
        self.exact_type
    }

    pub fn diagnostic_name(&self) -> &str {
        &self.diagnostic_name
    }

    pub const fn instance_layout(&self) -> PersistentLayoutId {
        self.instance_layout
    }

    pub const fn instance_scan(&self) -> PersistentScanId {
        self.instance_scan
    }

    pub const fn instance_shape(&self) -> &TypeInstanceShapeV1 {
        &self.instance_shape
    }

    pub const fn inline_scan(&self) -> TypeDescriptorInlineScanV1 {
        self.inline_scan
    }

    pub const fn parent(&self) -> Option<StrongTypeDescriptorRefV1> {
        self.parent
    }

    pub const fn vtable(&self) -> &StrongTypeVtableSemanticPlanV1 {
        &self.vtable
    }

    pub fn itables(&self) -> &[StrongTypeItableSemanticPlanV1] {
        &self.itables
    }
}

/// Complete canonical semantic authority for every local TypeDescriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongTypeDescriptorSemanticPlanSetV1 {
    producer: ConeIdentity,
    target: scoop_identity::TargetProfileWireId,
    descriptors: Vec<StrongTypeDescriptorSemanticPlanV1>,
}

struct DescriptorSemanticInputs<'a> {
    descriptors: &'a la_arena::Arena<TypeDescriptor>,
    external_descriptors: &'a la_arena::Arena<CoreExternalTypeDescriptor>,
    layouts: &'a la_arena::Arena<Layout>,
    arrays: &'a la_arena::Arena<ArrayType>,
    functions: &'a [Function],
    external_callables: &'a la_arena::Arena<CoreExternalCallable>,
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
                external_descriptors: &module.meta.core_external_type_descriptors,
                layouts: &module.meta.layouts,
                arrays: &module.meta.arrays,
                functions: &module.functions,
                external_callables: &module.meta.core_external_callables,
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

    pub(crate) const fn from_artifact(
        producer: ConeIdentity,
        target: scoop_identity::TargetProfileWireId,
        descriptors: Vec<StrongTypeDescriptorSemanticPlanV1>,
    ) -> Self {
        Self {
            producer,
            target,
            descriptors,
        }
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub const fn target(&self) -> &scoop_identity::TargetProfileWireId {
        &self.target
    }

    pub fn descriptors(&self) -> &[StrongTypeDescriptorSemanticPlanV1] {
        &self.descriptors
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
        .map(|reference| {
            resolve_descriptor_ref(reference, inputs.descriptors, inputs.external_descriptors)
        })
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
        let semantic = build_itable(
            exact_type,
            itable,
            inputs.descriptors,
            inputs.external_descriptors,
            inputs.functions,
            inputs.external_callables,
        )?;
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
                .map(|(_, array)| array.element_scan.clone()),
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
    descriptors: &la_arena::Arena<TypeDescriptor>,
    external_descriptors: &la_arena::Arena<CoreExternalTypeDescriptor>,
    functions: &[Function],
    external_callables: &la_arena::Arena<CoreExternalCallable>,
) -> Result<StrongTypeItableSemanticPlanV1, StrongTypeDescriptorSemanticPlanBuildError> {
    if !itable.belongs_to_exact_type(exact_type) {
        return Err(StrongTypeDescriptorSemanticPlanBuildError::ItableOwnerMismatch(exact_type));
    }
    let interface = resolve_descriptor_ref(itable.interface(), descriptors, external_descriptors)?;
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
        slots: resolve_slots(exact_type, itable.slots(), functions, external_callables)?,
    })
}

fn resolve_descriptor_ref(
    reference: TypeDescriptorRef,
    descriptors: &la_arena::Arena<TypeDescriptor>,
    external_descriptors: &la_arena::Arena<CoreExternalTypeDescriptor>,
) -> Result<StrongTypeDescriptorRefV1, StrongTypeDescriptorSemanticPlanBuildError> {
    match reference {
        TypeDescriptorRef::Local(id) => {
            let index = id.into_raw().into_u32();
            if index as usize >= descriptors.len() {
                return Err(
                    StrongTypeDescriptorSemanticPlanBuildError::MissingLocalDescriptor(index),
                );
            }
            Ok(StrongTypeDescriptorRefV1::Local(
                descriptors[id].identity.exact_type(),
            ))
        }
        TypeDescriptorRef::CoreExternal(id) => {
            let index = id.into_raw().into_u32();
            if index as usize >= external_descriptors.len() {
                return Err(
                    StrongTypeDescriptorSemanticPlanBuildError::MissingCoreExternalDescriptor(
                        index,
                    ),
                );
            }
            Ok(StrongTypeDescriptorRefV1::CoreExternal(
                external_descriptors[id].target(),
            ))
        }
    }
}

fn resolve_slots(
    exact_type: PersistentExactTypeId,
    slots: &[DispatchEntry],
    functions: &[Function],
    external_callables: &la_arena::Arena<CoreExternalCallable>,
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
            CallableRef::CoreExternal(id) => {
                let index = id.into_raw().into_u32();
                if index as usize >= external_callables.len() {
                    return Err(
                        StrongTypeDescriptorSemanticPlanBuildError::MissingCoreExternalCallable {
                            exact_type,
                            index,
                        },
                    );
                }
                Ok(StrongTypeDispatchCallableRefV1::CoreExternal(
                    external_callables[id].body(),
                ))
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
    MissingCoreExternalDescriptor(u32),
    MissingLocalCallable {
        exact_type: PersistentExactTypeId,
        index: u32,
    },
    MissingCoreExternalCallable {
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
