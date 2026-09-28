//! Complete semantic plans for strong static-storage registrations.

use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{
    ConeIdentity, LinkageClass, PersistentImmortalObjectId, PersistentLayoutId,
    PersistentSymbolRequest, RepresentationRole, ScanRole,
};
pub use scoop_identity::{PersistentScanId, PersistentStaticStorageId};

use crate::{
    BackendScalarKind, EnumRepr, GlobalInit, LirConstantImage, LirStaticInitialState,
    LirTargetProfile, LirType, PointerKind, RefScan, StaticStorageLayout,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticStorageScanKindV1 {
    None,
    Recursive,
}

impl StaticStorageScanKindV1 {
    pub const fn tag(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Recursive => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct StaticImmortalRelocationPlanV1 {
    pointer_offset: u64,
    target: PersistentImmortalObjectId,
}

impl StaticImmortalRelocationPlanV1 {
    pub(crate) const fn from_artifact(
        pointer_offset: u64,
        target: PersistentImmortalObjectId,
    ) -> Self {
        Self {
            pointer_offset,
            target,
        }
    }

    pub const fn pointer_offset(self) -> u64 {
        self.pointer_offset
    }

    pub const fn target(self) -> PersistentImmortalObjectId {
        self.target
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongStaticStorageInitialStatePlanV1 {
    ZeroedForRuntimeUnit,
    EncodedStaticValue {
        initial_template: Vec<u8>,
        immortal_relocations: Vec<StaticImmortalRelocationPlanV1>,
    },
}

impl StrongStaticStorageInitialStatePlanV1 {
    pub const fn tag(&self) -> u32 {
        match self {
            Self::ZeroedForRuntimeUnit => 1,
            Self::EncodedStaticValue { .. } => 2,
        }
    }

    pub fn immortal_relocations(&self) -> &[StaticImmortalRelocationPlanV1] {
        match self {
            Self::ZeroedForRuntimeUnit => &[],
            Self::EncodedStaticValue {
                immortal_relocations,
                ..
            } => immortal_relocations,
        }
    }

    pub fn initial_template(&self) -> &[u8] {
        match self {
            Self::ZeroedForRuntimeUnit => &[],
            Self::EncodedStaticValue {
                initial_template, ..
            } => initial_template,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongStaticStorageSemanticPlanV1 {
    storage: PersistentStaticStorageId,
    symbol: PersistentSymbolRequest,
    layout: StaticStorageLayout,
    layout_provider: ConeIdentity,
    scan_program: RefScan,
    scan_kind: StaticStorageScanKindV1,
    byte_size: u64,
    allocation_extent: u64,
    required_alignment: u64,
    initial_state: StrongStaticStorageInitialStatePlanV1,
}

impl StrongStaticStorageSemanticPlanV1 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_artifact(
        storage: PersistentStaticStorageId,
        symbol: PersistentSymbolRequest,
        layout: StaticStorageLayout,
        layout_provider: ConeIdentity,
        scan_program: RefScan,
        scan_kind: StaticStorageScanKindV1,
        byte_size: u64,
        allocation_extent: u64,
        required_alignment: u64,
        initial_state: StrongStaticStorageInitialStatePlanV1,
    ) -> Self {
        Self {
            storage,
            symbol,
            layout,
            layout_provider,
            scan_program,
            scan_kind,
            byte_size,
            allocation_extent,
            required_alignment,
            initial_state,
        }
    }

    pub const fn storage(&self) -> PersistentStaticStorageId {
        self.storage
    }

    pub const fn symbol(&self) -> PersistentSymbolRequest {
        self.symbol
    }

    pub fn layout(&self) -> PersistentLayoutId {
        self.layout.layout()
    }

    pub fn scan(&self) -> PersistentScanId {
        self.layout.scan()
    }

    pub const fn layout_provider(&self) -> ConeIdentity {
        self.layout_provider
    }

    pub const fn value_layout(&self) -> &StaticStorageLayout {
        &self.layout
    }

    pub const fn scan_program(&self) -> &RefScan {
        &self.scan_program
    }

    pub const fn scan_kind(&self) -> StaticStorageScanKindV1 {
        self.scan_kind
    }

    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }

    pub const fn allocation_extent(&self) -> u64 {
        self.allocation_extent
    }

    pub const fn required_alignment(&self) -> u64 {
        self.required_alignment
    }

    pub const fn initial_state(&self) -> &StrongStaticStorageInitialStatePlanV1 {
        &self.initial_state
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongStaticStorageSemanticPlanSetV1 {
    producer: ConeIdentity,
    storages: Vec<StrongStaticStorageSemanticPlanV1>,
}

impl StrongStaticStorageSemanticPlanSetV1 {
    pub fn from_module(
        module: &crate::Module,
    ) -> Result<Self, StrongStaticStorageSemanticPlanBuildError> {
        Self::from_parts(
            module.cone,
            module.meta.target_profile,
            &module.globals,
            &module.structs,
            &module.enums,
        )
    }

    pub(crate) const fn from_artifact(
        producer: ConeIdentity,
        storages: Vec<StrongStaticStorageSemanticPlanV1>,
    ) -> Self {
        Self { producer, storages }
    }

    pub(crate) fn from_parts(
        producer: ConeIdentity,
        target: LirTargetProfile,
        globals: &la_arena::Arena<crate::Global>,
        structs: &crate::StructDefs,
        enums: &crate::EnumDefs,
    ) -> Result<Self, StrongStaticStorageSemanticPlanBuildError> {
        let mut storages = BTreeMap::new();
        for (_, global) in globals.iter() {
            let GlobalInit::Storage {
                identity,
                layout,
                ty,
                initial_state,
                thread_local,
            } = &global.init
            else {
                continue;
            };
            let storage = identity.identity_record().id();
            if global.address_kind != PointerKind::Raw {
                return Err(StrongStaticStorageSemanticPlanBuildError::AddressKind {
                    storage,
                    actual: global.address_kind,
                });
            }
            if *thread_local {
                return Err(StrongStaticStorageSemanticPlanBuildError::ThreadLocal(
                    storage,
                ));
            }
            if !matches!(
                identity.symbol_request().linkage(),
                LinkageClass::ConeStrong | LinkageClass::OdrWeak
            ) {
                return Err(StrongStaticStorageSemanticPlanBuildError::Linkage {
                    storage,
                    actual: identity.symbol_request().linkage(),
                });
            }
            let layout_key = layout.layout_key();
            if layout_key.target_profile() != &target.wire_id() {
                return Err(StrongStaticStorageSemanticPlanBuildError::LayoutTarget { storage });
            }
            if layout_key.representation() == RepresentationRole::ManagedObject {
                return Err(
                    StrongStaticStorageSemanticPlanBuildError::LayoutRepresentation {
                        storage,
                        actual: layout_key.representation(),
                    },
                );
            }
            if let StaticStorageLayout::Local(identity) = layout
                && (identity.scan_record().key().layout() != layout.layout()
                    || identity.scan_record().key().role() != ScanRole::InlineValue)
            {
                return Err(StrongStaticStorageSemanticPlanBuildError::ScanIdentity { storage });
            }
            let (byte_size, required_alignment) =
                storage_shape(target, structs, enums, ty).map_err(|kind| {
                    StrongStaticStorageSemanticPlanBuildError::Shape { storage, kind }
                })?;
            let allocation_extent = byte_size.max(1);
            let expected_scan = storage_scan(target, structs, enums, ty, 0).map_err(|kind| {
                StrongStaticStorageSemanticPlanBuildError::Shape { storage, kind }
            })?;
            if global.scan != expected_scan {
                return Err(StrongStaticStorageSemanticPlanBuildError::Scan {
                    storage,
                    actual: global.scan.clone(),
                    expected: expected_scan,
                });
            }
            if !layout.matches_value(byte_size, required_alignment, &global.scan) {
                return Err(
                    StrongStaticStorageSemanticPlanBuildError::DependencyLayout { storage },
                );
            }
            validate_static_scan(storage, target, byte_size, &global.scan)?;
            let scan_kind = if global.scan.contains_reference() {
                StaticStorageScanKindV1::Recursive
            } else if global.scan == RefScan::None {
                StaticStorageScanKindV1::None
            } else {
                return Err(
                    StrongStaticStorageSemanticPlanBuildError::NonCanonicalEmptyScan(storage),
                );
            };
            let initial_state = match initial_state {
                LirStaticInitialState::ZeroedForRuntimeUnit => {
                    StrongStaticStorageInitialStatePlanV1::ZeroedForRuntimeUnit
                }
                LirStaticInitialState::EncodedStaticValue { payload } => {
                    let template_length = usize::try_from(allocation_extent).map_err(|_| {
                        StrongStaticStorageSemanticPlanBuildError::TemplateExtent {
                            storage,
                            allocation_extent,
                        }
                    })?;
                    let mut initial_template = Vec::new();
                    initial_template
                        .try_reserve_exact(template_length)
                        .map_err(
                            |_| StrongStaticStorageSemanticPlanBuildError::TemplateExtent {
                                storage,
                                allocation_extent,
                            },
                        )?;
                    initial_template.resize(template_length, 0);
                    let mut immortal_relocations = Vec::new();
                    StaticInitialStateEncoder {
                        globals,
                        structs,
                        enums,
                        storage,
                        template: &mut initial_template,
                        relocations: &mut immortal_relocations,
                    }
                    .collect(ty, payload, 0)?;
                    immortal_relocations.sort_unstable();
                    if let Some(pair) = immortal_relocations
                        .windows(2)
                        .find(|pair| pair[0].pointer_offset == pair[1].pointer_offset)
                    {
                        return Err(
                            StrongStaticStorageSemanticPlanBuildError::DuplicateRelocation {
                                storage,
                                offset: pair[0].pointer_offset,
                            },
                        );
                    }
                    StrongStaticStorageInitialStatePlanV1::EncodedStaticValue {
                        initial_template,
                        immortal_relocations,
                    }
                }
            };
            let plan = StrongStaticStorageSemanticPlanV1 {
                storage,
                symbol: identity.symbol_request(),
                layout: layout.clone(),
                layout_provider: layout.provider(producer),
                scan_program: global.scan.clone(),
                scan_kind,
                byte_size,
                allocation_extent,
                required_alignment,
                initial_state,
            };
            if storages.insert(storage, plan).is_some() {
                return Err(StrongStaticStorageSemanticPlanBuildError::DuplicateStorage(
                    storage,
                ));
            }
        }
        Ok(Self {
            producer,
            storages: storages.into_values().collect(),
        })
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn storages(&self) -> &[StrongStaticStorageSemanticPlanV1] {
        &self.storages
    }
}

fn validate_static_scan(
    storage: PersistentStaticStorageId,
    target: LirTargetProfile,
    byte_size: u64,
    scan: &RefScan,
) -> Result<(), StrongStaticStorageSemanticPlanBuildError> {
    let RefScan::References(offsets) = scan else {
        return if scan == &RefScan::None {
            Ok(())
        } else {
            Err(StrongStaticStorageSemanticPlanBuildError::NonCanonicalRecursiveScan(storage))
        };
    };
    if offsets.is_empty() || offsets.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(StrongStaticStorageSemanticPlanBuildError::NonCanonicalRecursiveScan(storage));
    }
    let pointer = target.pointer_layout(PointerKind::Managed);
    for offset in offsets {
        let end = offset.checked_add(pointer.size_bytes()).ok_or(
            StrongStaticStorageSemanticPlanBuildError::InvalidScanSlot {
                storage,
                offset: *offset,
                byte_size,
            },
        )?;
        if offset % pointer.alignment_bytes() != 0 || end > byte_size {
            return Err(StrongStaticStorageSemanticPlanBuildError::InvalidScanSlot {
                storage,
                offset: *offset,
                byte_size,
            });
        }
    }
    Ok(())
}

fn storage_shape(
    target: LirTargetProfile,
    structs: &crate::StructDefs,
    enums: &crate::EnumDefs,
    ty: &LirType,
) -> Result<(u64, u64), StaticStorageShapeFailureV1> {
    let scalar = |kind| {
        let layout = target.scalar_layout(kind);
        (layout.size_bytes(), layout.alignment_bytes())
    };
    let shape = match ty {
        LirType::I1 => scalar(BackendScalarKind::I1),
        LirType::I8 => scalar(BackendScalarKind::I8),
        LirType::I16 => scalar(BackendScalarKind::I16),
        LirType::I32 => scalar(BackendScalarKind::I32),
        LirType::I64 => scalar(BackendScalarKind::I64),
        LirType::Ptr(kind) => {
            let layout = target.pointer_layout(*kind);
            (layout.size_bytes(), layout.alignment_bytes())
        }
        LirType::Aggregate(fields) => {
            let mut size = 0_u64;
            let mut alignment = 1_u64;
            for field in fields {
                let (field_size, field_alignment) = storage_shape(target, structs, enums, field)?;
                size = align_up(size, field_alignment)?;
                size = size
                    .checked_add(field_size)
                    .ok_or(StaticStorageShapeFailureV1::Overflow)?;
                alignment = alignment.max(field_alignment);
            }
            (align_up(size, alignment)?, alignment)
        }
        LirType::Struct(id) => {
            let index = id.into_raw().into_u32() as usize;
            if index >= structs.len() {
                return Err(StaticStorageShapeFailureV1::UnknownStruct);
            }
            (structs[*id].size, structs[*id].align)
        }
        LirType::Enum(id) => {
            let index = id.into_raw().into_u32() as usize;
            if index >= enums.len() {
                return Err(StaticStorageShapeFailureV1::UnknownEnum);
            }
            match &enums[*id].repr {
                EnumRepr::Niche { .. } => {
                    let layout = target.metadata_pointer_layout();
                    (layout.size_bytes(), layout.alignment_bytes())
                }
                EnumRepr::Tagged { size, align, .. } => (*size, *align),
            }
        }
        LirType::Void => return Err(StaticStorageShapeFailureV1::Void),
        LirType::MachineScalar(_) => return Err(StaticStorageShapeFailureV1::MachineScalar),
        LirType::ExceptionRecord => {
            return Err(StaticStorageShapeFailureV1::ExceptionRecord);
        }
    };
    if shape.1 == 0 || !shape.1.is_power_of_two() {
        return Err(StaticStorageShapeFailureV1::InvalidAlignment(shape.1));
    }
    Ok(shape)
}

fn align_up(value: u64, alignment: u64) -> Result<u64, StaticStorageShapeFailureV1> {
    value
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
        .ok_or(StaticStorageShapeFailureV1::Overflow)
}

fn storage_scan(
    target: LirTargetProfile,
    structs: &crate::StructDefs,
    enums: &crate::EnumDefs,
    ty: &LirType,
    base_offset: u64,
) -> Result<RefScan, StaticStorageShapeFailureV1> {
    match ty {
        LirType::Ptr(PointerKind::Managed) => Ok(RefScan::References(vec![base_offset])),
        LirType::Aggregate(fields) => {
            let mut offset = 0;
            let mut scans = Vec::with_capacity(fields.len());
            for field in fields {
                let (field_size, field_alignment) = storage_shape(target, structs, enums, field)?;
                offset = align_up(offset, field_alignment)?;
                let field_base = base_offset
                    .checked_add(offset)
                    .ok_or(StaticStorageShapeFailureV1::Overflow)?;
                scans.push(storage_scan(target, structs, enums, field, field_base)?);
                offset = offset
                    .checked_add(field_size)
                    .ok_or(StaticStorageShapeFailureV1::Overflow)?;
            }
            Ok(flatten_scans(scans))
        }
        LirType::Struct(id) => {
            let index = id.into_raw().into_u32() as usize;
            if index >= structs.len() {
                return Err(StaticStorageShapeFailureV1::UnknownStruct);
            }
            let definition = &structs[*id];
            let scans = (0..definition.field_count())
                .map(|index| {
                    let field = definition
                        .field_storage_type(index)
                        .ok_or(StaticStorageShapeFailureV1::UnknownStruct)?;
                    let offset = definition
                        .field_layout(index)
                        .and_then(|layout| base_offset.checked_add(layout.offset))
                        .ok_or(StaticStorageShapeFailureV1::Overflow)?;
                    storage_scan(target, structs, enums, &field, offset)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(flatten_scans(scans))
        }
        LirType::Enum(id) => {
            let index = id.into_raw().into_u32() as usize;
            if index >= enums.len() {
                return Err(StaticStorageShapeFailureV1::UnknownEnum);
            }
            shift_scan(&enums[*id].scan, base_offset)
        }
        LirType::I1
        | LirType::I8
        | LirType::I16
        | LirType::I32
        | LirType::I64
        | LirType::Ptr(_) => Ok(RefScan::None),
        LirType::Void => Err(StaticStorageShapeFailureV1::Void),
        LirType::MachineScalar(_) => Err(StaticStorageShapeFailureV1::MachineScalar),
        LirType::ExceptionRecord => Err(StaticStorageShapeFailureV1::ExceptionRecord),
    }
}

fn flatten_scans(scans: impl IntoIterator<Item = RefScan>) -> RefScan {
    fn collect(scan: RefScan, references: &mut Vec<u64>) {
        match scan {
            RefScan::None => {}
            RefScan::References(offsets) => references.extend(offsets),
            RefScan::Sequence(parts) => {
                for part in parts {
                    collect(part, references);
                }
            }
            RefScan::Array { .. } => {
                unreachable!("inline value storage cannot contain a variable object scan")
            }
        }
    }

    let mut references = Vec::new();
    for scan in scans {
        collect(scan, &mut references);
    }
    if references.is_empty() {
        RefScan::None
    } else {
        RefScan::References(references)
    }
}

fn shift_scan(scan: &RefScan, base_offset: u64) -> Result<RefScan, StaticStorageShapeFailureV1> {
    match scan {
        RefScan::None => Ok(RefScan::None),
        RefScan::References(offsets) => offsets
            .iter()
            .map(|offset| {
                base_offset
                    .checked_add(*offset)
                    .ok_or(StaticStorageShapeFailureV1::Overflow)
            })
            .collect::<Result<Vec<_>, _>>()
            .map(RefScan::References),
        RefScan::Sequence(parts) => parts
            .iter()
            .map(|part| shift_scan(part, base_offset))
            .collect::<Result<Vec<_>, _>>()
            .map(RefScan::Sequence),
        RefScan::Array { .. } => Err(StaticStorageShapeFailureV1::VariableObjectScan),
    }
}

struct StaticInitialStateEncoder<'a> {
    globals: &'a la_arena::Arena<crate::Global>,
    structs: &'a crate::StructDefs,
    enums: &'a crate::EnumDefs,
    storage: PersistentStaticStorageId,
    template: &'a mut [u8],
    relocations: &'a mut Vec<StaticImmortalRelocationPlanV1>,
}

impl StaticInitialStateEncoder<'_> {
    fn collect(
        &mut self,
        expected: &LirType,
        value: &LirConstantImage,
        base_offset: u64,
    ) -> Result<(), StrongStaticStorageSemanticPlanBuildError> {
        let storage = self.storage;
        let type_mismatch = || StrongStaticStorageSemanticPlanBuildError::InitialValue {
            storage,
            kind: StaticStorageInitialValueFailureV1::TypeMismatch,
        };
        match value {
            LirConstantImage::Integer(value) if &value.scalar_type() == expected => {
                let width = usize::try_from(value.kind().width().bits() / 8)
                    .expect("source integer widths fit usize");
                self.write(base_offset, &value.raw_bits().to_le_bytes()[..width])
            }
            LirConstantImage::Bool(value) if expected == &LirType::I1 => {
                self.write(base_offset, &[u8::from(*value)])
            }
            LirConstantImage::NullPointer(kind) if expected == &LirType::Ptr(*kind) => Ok(()),
            LirConstantImage::GlobalPointer { global, kind }
                if expected == &LirType::Ptr(*kind) && *kind == PointerKind::Managed =>
            {
                let index = global.into_raw().into_u32() as usize;
                if index >= self.globals.len() {
                    return Err(StrongStaticStorageSemanticPlanBuildError::InitialValue {
                        storage,
                        kind: StaticStorageInitialValueFailureV1::MissingGlobal,
                    });
                }
                let target = &self.globals[*global];
                let GlobalInit::StringConst { identity, .. } = &target.init else {
                    return Err(StrongStaticStorageSemanticPlanBuildError::InitialValue {
                        storage,
                        kind: StaticStorageInitialValueFailureV1::NonImmortalTarget,
                    });
                };
                if target.address_kind != PointerKind::Managed {
                    return Err(StrongStaticStorageSemanticPlanBuildError::InitialValue {
                        storage,
                        kind: StaticStorageInitialValueFailureV1::TargetAddressKind,
                    });
                }
                self.relocations.push(StaticImmortalRelocationPlanV1 {
                    pointer_offset: base_offset,
                    target: identity.identity_record().id(),
                });
                Ok(())
            }
            LirConstantImage::EnumUnit { variant } => {
                let LirType::Enum(expected_enum) = expected else {
                    return Err(type_mismatch());
                };
                if variant.definition() != *expected_enum || !self.enums.contains_variant(*variant)
                {
                    return Err(StrongStaticStorageSemanticPlanBuildError::InitialValue {
                        storage,
                        kind: StaticStorageInitialValueFailureV1::InvalidEnumVariant,
                    });
                }
                let has_payload = match &self.enums[*expected_enum].repr {
                    EnumRepr::Niche {
                        payload_variant, ..
                    } => variant.index() == *payload_variant,
                    EnumRepr::Tagged { variants, .. } => {
                        !variants[variant.index() as usize].fields.is_empty()
                    }
                };
                if has_payload {
                    return Err(StrongStaticStorageSemanticPlanBuildError::InitialValue {
                        storage,
                        kind: StaticStorageInitialValueFailureV1::PayloadEnumVariant,
                    });
                }
                match &self.enums[*expected_enum].repr {
                    EnumRepr::Niche { .. } => Ok(()),
                    EnumRepr::Tagged { .. } => {
                        self.write(base_offset, &u64::from(variant.index()).to_le_bytes())
                    }
                }
            }
            LirConstantImage::Struct { struct_id, fields } => {
                if expected != &LirType::Struct(*struct_id) {
                    return Err(type_mismatch());
                }
                let index = struct_id.into_raw().into_u32() as usize;
                if index >= self.structs.len() {
                    return Err(StrongStaticStorageSemanticPlanBuildError::InitialValue {
                        storage,
                        kind: StaticStorageInitialValueFailureV1::UnknownStruct,
                    });
                }
                let definition = &self.structs[*struct_id];
                if fields.len() != definition.field_count() {
                    return Err(StrongStaticStorageSemanticPlanBuildError::InitialValue {
                        storage,
                        kind: StaticStorageInitialValueFailureV1::StructFieldCount,
                    });
                }
                for (index, field) in fields.iter().enumerate() {
                    let field_type = definition.field_storage_type(index).ok_or(
                        StrongStaticStorageSemanticPlanBuildError::InitialValue {
                            storage,
                            kind: StaticStorageInitialValueFailureV1::UnknownStruct,
                        },
                    )?;
                    let offset = definition
                        .field_layout(index)
                        .and_then(|layout| base_offset.checked_add(layout.offset))
                        .ok_or(StrongStaticStorageSemanticPlanBuildError::InitialValue {
                            storage,
                            kind: StaticStorageInitialValueFailureV1::OffsetOverflow,
                        })?;
                    self.collect(&field_type, field, offset)?;
                }
                Ok(())
            }
            _ => Err(type_mismatch()),
        }
    }

    fn write(
        &mut self,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), StrongStaticStorageSemanticPlanBuildError> {
        let storage = self.storage;
        let start = usize::try_from(offset).map_err(|_| {
            StrongStaticStorageSemanticPlanBuildError::InitialValue {
                storage,
                kind: StaticStorageInitialValueFailureV1::TemplateBounds,
            }
        })?;
        let end = start.checked_add(bytes.len()).ok_or(
            StrongStaticStorageSemanticPlanBuildError::InitialValue {
                storage,
                kind: StaticStorageInitialValueFailureV1::TemplateBounds,
            },
        )?;
        let target = self.template.get_mut(start..end).ok_or(
            StrongStaticStorageSemanticPlanBuildError::InitialValue {
                storage,
                kind: StaticStorageInitialValueFailureV1::TemplateBounds,
            },
        )?;
        target.copy_from_slice(bytes);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticStorageShapeFailureV1 {
    Void,
    MachineScalar,
    ExceptionRecord,
    UnknownStruct,
    UnknownEnum,
    InvalidAlignment(u64),
    Overflow,
    VariableObjectScan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticStorageInitialValueFailureV1 {
    TypeMismatch,
    MissingGlobal,
    NonImmortalTarget,
    TargetAddressKind,
    InvalidEnumVariant,
    PayloadEnumVariant,
    UnknownStruct,
    StructFieldCount,
    OffsetOverflow,
    TemplateBounds,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongStaticStorageSemanticPlanBuildError {
    DuplicateStorage(PersistentStaticStorageId),
    AddressKind {
        storage: PersistentStaticStorageId,
        actual: PointerKind,
    },
    ThreadLocal(PersistentStaticStorageId),
    Linkage {
        storage: PersistentStaticStorageId,
        actual: LinkageClass,
    },
    DependencyLayout {
        storage: PersistentStaticStorageId,
    },
    LayoutTarget {
        storage: PersistentStaticStorageId,
    },
    LayoutRepresentation {
        storage: PersistentStaticStorageId,
        actual: RepresentationRole,
    },
    ScanIdentity {
        storage: PersistentStaticStorageId,
    },
    Scan {
        storage: PersistentStaticStorageId,
        actual: RefScan,
        expected: RefScan,
    },
    NonCanonicalEmptyScan(PersistentStaticStorageId),
    NonCanonicalRecursiveScan(PersistentStaticStorageId),
    InvalidScanSlot {
        storage: PersistentStaticStorageId,
        offset: u64,
        byte_size: u64,
    },
    Shape {
        storage: PersistentStaticStorageId,
        kind: StaticStorageShapeFailureV1,
    },
    InitialValue {
        storage: PersistentStaticStorageId,
        kind: StaticStorageInitialValueFailureV1,
    },
    DuplicateRelocation {
        storage: PersistentStaticStorageId,
        offset: u64,
    },
    TemplateExtent {
        storage: PersistentStaticStorageId,
        allocation_extent: u64,
    },
}

impl fmt::Display for StrongStaticStorageSemanticPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong static-storage semantics: {self:?}"
        )
    }
}

impl std::error::Error for StrongStaticStorageSemanticPlanBuildError {}

mod registrations;
pub use registrations::*;

#[cfg(test)]
mod tests;
