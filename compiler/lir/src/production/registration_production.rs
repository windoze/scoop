//! Closed registration-production authority for the M23-3 strong profile.

use std::fmt;

use scoop_wire::{Encoder, WireEncode};

use crate::{
    ImmortalObjectTypeRegistrationRefV1, Module, OdrFreeLirFoundation, RefScan,
    StaticStorageRelocationTableArtifactV1, StrongCallableRegistrationPlanBuildError,
    StrongCallableRegistrationPlanSetV1, StrongCallableRegistrationPlanV1,
    StrongCallableRuntimeScanPlanError, StrongCallableRuntimeScanPlanSetV1,
    StrongDigestFinalizationPlanV1, StrongImmortalObjectRegistrationPlanBuildError,
    StrongImmortalObjectRegistrationPlanSetV1, StrongImmortalObjectRegistrationPlanV1,
    StrongImmortalObjectSemanticPlanBuildError, StrongImmortalObjectSemanticPlanSetV1,
    StrongInitializationCallableRefPlanV1, StrongInitializationRegistrationSchedulePlanV1,
    StrongInitializationStaticStorageRefPlanV1, StrongInitializationUnitRegistrationPlanBuildError,
    StrongInitializationUnitRegistrationPlanSetV1, StrongInitializationUnitRegistrationPlanV1,
    StrongInitializationUnitSemanticPlanBuildError, StrongInitializationUnitSemanticPlanSetV1,
    StrongRegistrationIdentityBuildError, StrongRegistrationIdentitySurfaceV1,
    StrongSafepointRegistrationPlanBuildError, StrongSafepointRegistrationPlanSetV1,
    StrongSafepointRegistrationPlanV1, StrongSafepointSemanticPlanError,
    StrongSafepointSemanticPlanSetV1, StrongSafepointSemanticPlanV1,
    StrongStaticStorageInitialArtifactPlanV1, StrongStaticStorageInitialStatePlanV1,
    StrongStaticStorageRegistrationPlanBuildError, StrongStaticStorageRegistrationPlanSetV1,
    StrongStaticStorageRegistrationPlanV1, StrongTypeDescriptorRefV1,
    StrongTypeDescriptorSemanticPlanBuildError, StrongTypeDescriptorSemanticPlanSetV1,
    StrongTypeDispatchCallableRefV1, StrongTypeItableSemanticPlanV1,
    StrongTypeRegistrationPlanBuildError, StrongTypeRegistrationPlanSetV1,
    StrongTypeRegistrationPlanV1, StrongTypeVtableSemanticPlanV1,
};

mod wire;
pub use wire::*;

mod validation;
pub use validation::*;

/// Complete member-independent registration authority produced from one final
/// LIR module. Every table has already been checked against the same foundation
/// and digest graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongRegistrationProductionSurfaceV1 {
    identities: StrongRegistrationIdentitySurfaceV1,
    safepoints: StrongSafepointRegistrationPlanSetV1,
    callables: StrongCallableRegistrationPlanSetV1,
    types: StrongTypeRegistrationPlanSetV1,
    immortal_objects: StrongImmortalObjectRegistrationPlanSetV1,
    static_storages: StrongStaticStorageRegistrationPlanSetV1,
    initialization_units: StrongInitializationUnitRegistrationPlanSetV1,
}

impl StrongRegistrationProductionSurfaceV1 {
    pub fn empty(
        target: crate::LirTargetProfile,
        foundation: &OdrFreeLirFoundation,
        digests: &StrongDigestFinalizationPlanV1,
    ) -> Result<Self, StrongRegistrationProductionBuildError> {
        let identities = StrongRegistrationIdentitySurfaceV1::from_foundation(foundation, digests)
            .map_err(StrongRegistrationProductionBuildError::Identities)?;
        let type_semantics = StrongTypeDescriptorSemanticPlanSetV1::from_artifact(
            foundation.producer(),
            target.wire_id(),
            Vec::new(),
        );
        let callable_runtime_scans =
            StrongCallableRuntimeScanPlanSetV1::from_foundation_without_scans(foundation)
                .map_err(StrongRegistrationProductionBuildError::CallableRuntimeScans)?;
        Self::from_semantics(
            target,
            foundation,
            digests,
            identities,
            callable_runtime_scans,
            type_semantics,
            StrongSafepointSemanticPlanSetV1::from_artifact(foundation.producer(), Vec::new()),
            StrongImmortalObjectSemanticPlanSetV1::from_artifact(foundation.producer(), Vec::new()),
            StrongInitializationUnitSemanticPlanSetV1::from_artifact(
                crate::StrongStaticStorageSemanticPlanSetV1::from_artifact(
                    foundation.producer(),
                    Vec::new(),
                ),
                Vec::new(),
            ),
        )
    }

    pub fn from_module(
        module: &Module,
        foundation: &OdrFreeLirFoundation,
        digests: &StrongDigestFinalizationPlanV1,
    ) -> Result<Self, StrongRegistrationProductionBuildError> {
        if module.cone != foundation.producer() {
            return Err(StrongRegistrationProductionBuildError::ProducerMismatch {
                module: module.cone,
                foundation: foundation.producer(),
            });
        }

        let identities = StrongRegistrationIdentitySurfaceV1::from_foundation(foundation, digests)
            .map_err(StrongRegistrationProductionBuildError::Identities)?;
        let safepoint_semantics = StrongSafepointSemanticPlanSetV1::from_module(module)
            .map_err(StrongRegistrationProductionBuildError::SafepointSemantics)?;
        let type_semantics = StrongTypeDescriptorSemanticPlanSetV1::from_module(module)
            .map_err(StrongRegistrationProductionBuildError::TypeSemantics)?;
        let immortal_semantics = StrongImmortalObjectSemanticPlanSetV1::from_module(module)
            .map_err(StrongRegistrationProductionBuildError::ImmortalSemantics)?;
        let initialization_semantics =
            StrongInitializationUnitSemanticPlanSetV1::from_module(module)
                .map_err(StrongRegistrationProductionBuildError::InitializationSemantics)?;
        let callable_runtime_scans = StrongCallableRuntimeScanPlanSetV1::from_module(module)
            .map_err(StrongRegistrationProductionBuildError::CallableRuntimeScans)?;

        Self::from_semantics(
            module.meta.target_profile,
            foundation,
            digests,
            identities,
            callable_runtime_scans,
            type_semantics,
            safepoint_semantics,
            immortal_semantics,
            initialization_semantics,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_semantics(
        target: crate::LirTargetProfile,
        foundation: &OdrFreeLirFoundation,
        digests: &StrongDigestFinalizationPlanV1,
        identities: StrongRegistrationIdentitySurfaceV1,
        callable_runtime_scans: StrongCallableRuntimeScanPlanSetV1,
        type_semantics: StrongTypeDescriptorSemanticPlanSetV1,
        safepoint_semantics: StrongSafepointSemanticPlanSetV1,
        immortal_semantics: StrongImmortalObjectSemanticPlanSetV1,
        initialization_semantics: StrongInitializationUnitSemanticPlanSetV1,
    ) -> Result<Self, StrongRegistrationProductionBuildError> {
        let safepoints = StrongSafepointRegistrationPlanSetV1::new(
            foundation,
            &identities,
            &safepoint_semantics,
            digests,
        )
        .map_err(StrongRegistrationProductionBuildError::Safepoints)?;
        let callables = StrongCallableRegistrationPlanSetV1::new(
            foundation,
            &identities,
            callable_runtime_scans,
            digests,
        )
        .map_err(StrongRegistrationProductionBuildError::Callables)?;
        let types = StrongTypeRegistrationPlanSetV1::new(
            target,
            foundation,
            &identities,
            &type_semantics,
            digests,
        )
        .map_err(StrongRegistrationProductionBuildError::Types)?;
        let immortal_objects = StrongImmortalObjectRegistrationPlanSetV1::new(
            foundation,
            &identities,
            &immortal_semantics,
            digests,
        )
        .map_err(StrongRegistrationProductionBuildError::ImmortalObjects)?;
        let static_storages = StrongStaticStorageRegistrationPlanSetV1::new(
            foundation,
            &identities,
            initialization_semantics.static_storages(),
            digests,
        )
        .map_err(StrongRegistrationProductionBuildError::StaticStorages)?;
        let initialization_units = StrongInitializationUnitRegistrationPlanSetV1::new(
            foundation,
            &identities,
            &initialization_semantics,
            digests,
        )
        .map_err(StrongRegistrationProductionBuildError::InitializationUnits)?;

        Ok(Self {
            identities,
            safepoints,
            callables,
            types,
            immortal_objects,
            static_storages,
            initialization_units,
        })
    }

    pub const fn identities(&self) -> &StrongRegistrationIdentitySurfaceV1 {
        &self.identities
    }

    pub const fn safepoints(&self) -> &StrongSafepointRegistrationPlanSetV1 {
        &self.safepoints
    }

    pub const fn callables(&self) -> &StrongCallableRegistrationPlanSetV1 {
        &self.callables
    }

    pub const fn types(&self) -> &StrongTypeRegistrationPlanSetV1 {
        &self.types
    }

    pub const fn immortal_objects(&self) -> &StrongImmortalObjectRegistrationPlanSetV1 {
        &self.immortal_objects
    }

    pub const fn static_storages(&self) -> &StrongStaticStorageRegistrationPlanSetV1 {
        &self.static_storages
    }

    pub const fn initialization_units(&self) -> &StrongInitializationUnitRegistrationPlanSetV1 {
        &self.initialization_units
    }

    pub fn safepoint_semantics(&self) -> StrongSafepointSemanticPlanSetV1 {
        StrongSafepointSemanticPlanSetV1::from_artifact(
            self.safepoints.producer(),
            self.safepoints
                .registrations()
                .iter()
                .map(|plan| {
                    StrongSafepointSemanticPlanV1::from_artifact(
                        plan.site(),
                        plan.safepoint(),
                        plan.owner(),
                        plan.role(),
                        plan.root_pair_count(),
                    )
                })
                .collect(),
        )
    }
}

impl WireEncode for StrongRegistrationProductionSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
        encode_field(encoder, 1, &self.identities)?;
        encode_array_field(encoder, 2, self.safepoints.registrations())?;
        encode_array_field(encoder, 3, self.callables.registrations())?;
        encode_array_field(encoder, 4, self.types.registrations())?;
        encode_array_field(encoder, 5, self.immortal_objects.registrations())?;
        encode_array_field(encoder, 6, self.static_storages.registrations())?;
        encode_array_field(encoder, 7, self.initialization_units.registrations())?;
        encoder.field(8)?;
        encode_callable_runtime_scans(encoder, self.callables.runtime_scans())
    }
}

impl WireEncode for StrongSafepointRegistrationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(12)?;
        encode_field(encoder, 1, &self.site())?;
        encode_field(encoder, 2, &self.safepoint())?;
        encode_field(encoder, 3, &self.owner())?;
        encode_field(encoder, 4, &self.role())?;
        encode_unsigned_field(encoder, 5, u64::from(self.root_pair_count()))?;
        encode_field(encoder, 6, &self.symbol())?;
        encode_field(encoder, 7, &self.definition_plan())?;
        encode_field(encoder, 8, &self.primary_atom())?;
        encode_field(encoder, 9, &self.registration_fingerprint_node())?;
        encode_field(encoder, 10, &self.normalized_stackmap_fingerprint_node())?;
        encode_field(encoder, 11, &self.registration_definition_patch())?;
        encode_field(encoder, 12, &self.normalized_stackmap_patch())
    }
}

impl WireEncode for StrongCallableRegistrationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(12)?;
        encode_field(encoder, 1, &self.body())?;
        encode_field(encoder, 2, &self.symbol())?;
        encode_field(encoder, 3, &self.definition_plan())?;
        encode_field(encoder, 4, &self.primary_atom())?;
        encode_field(encoder, 5, &self.entry_symbol())?;
        encode_field(encoder, 6, &self.body_definition_plan())?;
        encode_field(encoder, 7, &self.body_primary_atom())?;
        encode_field(encoder, 8, &self.registration_object_node())?;
        encode_field(encoder, 9, &self.body_definition_node())?;
        encode_field(encoder, 10, &self.registration_fingerprint_node())?;
        encode_field(encoder, 11, &self.registration_definition_patch())?;
        encode_field(encoder, 12, &self.body_definition_patch())
    }
}

fn encode_callable_runtime_scans(
    encoder: &mut Encoder,
    plans: &StrongCallableRuntimeScanPlanSetV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(plans.callables().len() as u64)?;
    for callable in plans.callables() {
        encoder.map(2)?;
        encode_field(encoder, 1, &callable.body())?;
        encoder.field(2)?;
        encoder.array(callable.atoms().len() as u64)?;
        for atom in callable.atoms() {
            encoder.map(2)?;
            encode_field(encoder, 1, &atom.atom())?;
            encoder.field(2)?;
            encode_ref_scan(encoder, atom.scan())?;
        }
    }
    Ok(())
}

impl WireEncode for StrongTypeRegistrationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let semantic = self.semantic();
        encoder.map(28)?;
        encode_field(encoder, 1, &self.exact_type())?;
        encode_field(encoder, 2, &self.runtime_type())?;
        encode_field(encoder, 3, &self.symbol())?;
        encode_field(encoder, 4, &self.definition_plan())?;
        encode_field(encoder, 5, &self.primary_atom())?;
        encode_field(encoder, 6, &self.descriptor_symbol())?;
        encode_field(encoder, 7, &self.descriptor_definition_plan())?;
        encode_field(encoder, 8, &self.descriptor_primary_atom())?;
        encode_field(encoder, 9, &self.layout())?;
        encode_field(encoder, 10, &self.layout_symbol())?;
        encode_field(encoder, 11, &self.layout_definition_plan())?;
        encode_field(encoder, 12, &self.layout_primary_atom())?;
        encode_field(encoder, 13, &self.registration_object_node())?;
        encode_field(encoder, 14, &self.descriptor_definition_node())?;
        encode_field(encoder, 15, &self.layout_fingerprint_node())?;
        encode_field(encoder, 16, &self.registration_fingerprint_node())?;
        encode_field(encoder, 17, &self.registration_definition_patch())?;
        encode_field(encoder, 18, &self.descriptor_definition_patch())?;
        encode_field(encoder, 19, &self.layout_fingerprint_patch())?;
        encoder.field(20)?;
        encoder.text(semantic.diagnostic_name())?;
        encode_field(encoder, 21, &semantic.instance_scan())?;
        encoder.field(22)?;
        encode_type_instance_shape(encoder, semantic.instance_shape())?;
        encoder.field(23)?;
        encode_optional_type_descriptor_ref(encoder, semantic.parent())?;
        encoder.field(24)?;
        encode_type_vtable(encoder, semantic.vtable())?;
        encoder.field(25)?;
        encoder.array(semantic.itables().len() as u64)?;
        for itable in semantic.itables() {
            encode_type_itable(encoder, itable)?;
        }
        encode_field(encoder, 26, &self.diagnostic_atom())?;
        encoder.field(27)?;
        encode_type_descriptor_inline_scan(encoder, semantic.inline_scan())?;
        encoder.field(28)?;
        encode_type_descriptor_itable_directory(encoder, self.itable_directory())
    }
}

fn encode_type_descriptor_itable_directory(
    encoder: &mut Encoder,
    directory: crate::TypeDescriptorITableDirectoryV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match directory {
        crate::TypeDescriptorITableDirectoryV1::Null => {
            encoder.map(2)?;
            encode_unsigned_field(encoder, 0, 1)?;
            encode_unsigned_field(encoder, 1, 0)
        }
        crate::TypeDescriptorITableDirectoryV1::Defined(atom) => {
            encode_value_sum(encoder, 2, &atom)
        }
    }
}

fn encode_type_descriptor_inline_scan(
    encoder: &mut Encoder,
    inline_scan: crate::TypeDescriptorInlineScanV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match inline_scan {
        crate::TypeDescriptorInlineScanV1::Null => {
            encoder.map(2)?;
            encode_unsigned_field(encoder, 0, 1)?;
            encode_unsigned_field(encoder, 1, 0)
        }
        crate::TypeDescriptorInlineScanV1::Defined(scan) => encode_value_sum(encoder, 2, &scan),
    }
}

impl WireEncode for StrongImmortalObjectRegistrationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(15)?;
        encode_field(encoder, 1, &self.object())?;
        encode_field(encoder, 2, &self.object_symbol())?;
        encode_unsigned_field(encoder, 3, self.object_size())?;
        encode_unsigned_field(encoder, 4, self.required_alignment())?;
        encoder.field(5)?;
        encode_type_registration_ref(encoder, self.semantic().type_registration_ref())?;
        encode_field(encoder, 6, &self.registration_symbol())?;
        encode_field(encoder, 7, &self.registration_definition_plan())?;
        encode_field(encoder, 8, &self.registration_primary_atom())?;
        encode_field(encoder, 9, &self.object_definition_plan())?;
        encode_field(encoder, 10, &self.object_primary_atom())?;
        encode_field(encoder, 11, &self.type_registration_symbol())?;
        encode_field(encoder, 12, &self.registration_object_node())?;
        encode_field(encoder, 13, &self.object_definition_node())?;
        encode_field(encoder, 14, &self.registration_fingerprint_node())?;
        encode_field(encoder, 15, &self.registration_definition_patch())
    }
}

impl WireEncode for StrongStaticStorageRegistrationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let semantic = self.semantic();
        encoder.map(31)?;
        encode_field(encoder, 1, &semantic.storage())?;
        encode_field(encoder, 2, &semantic.symbol())?;
        encode_field(encoder, 3, &semantic.layout())?;
        encode_field(encoder, 4, &semantic.scan())?;
        encoder.field(5)?;
        encode_ref_scan(encoder, semantic.scan_program())?;
        encode_unsigned_field(encoder, 6, u64::from(semantic.scan_kind().tag()))?;
        encode_unsigned_field(encoder, 7, semantic.byte_size())?;
        encode_unsigned_field(encoder, 8, semantic.allocation_extent())?;
        encode_unsigned_field(encoder, 9, semantic.required_alignment())?;
        encoder.field(10)?;
        encode_static_initial_state(encoder, semantic.initial_state())?;
        encode_field(encoder, 11, &self.registration_symbol())?;
        encode_field(encoder, 12, &self.registration_definition_plan())?;
        encode_field(encoder, 13, &self.registration_primary_atom())?;
        encode_field(encoder, 14, &self.storage_definition_plan())?;
        encode_field(encoder, 15, &self.storage_primary_atom())?;
        encoder.field(16)?;
        encode_static_initial_artifacts(encoder, self.initial_artifacts())?;
        encode_array_field(encoder, 17, self.immortal_registration_symbols())?;
        encode_field(encoder, 18, &self.layout_symbol())?;
        encode_field(encoder, 19, &self.layout_definition_plan())?;
        encode_field(encoder, 20, &self.layout_primary_atom())?;
        encode_field(encoder, 21, &self.scan_symbol())?;
        encode_field(encoder, 22, &self.scan_definition_plan())?;
        encode_field(encoder, 23, &self.scan_primary_atom())?;
        encode_field(encoder, 24, &self.registration_object_node())?;
        encode_field(encoder, 25, &self.storage_definition_node())?;
        encode_field(encoder, 26, &self.layout_fingerprint_node())?;
        encode_field(encoder, 27, &self.scan_fingerprint_node())?;
        encode_field(encoder, 28, &self.registration_fingerprint_node())?;
        encode_field(encoder, 29, &self.registration_definition_patch())?;
        encode_field(encoder, 30, &self.layout_fingerprint_patch())?;
        encode_field(encoder, 31, &self.scan_fingerprint_patch())
    }
}

impl WireEncode for StrongInitializationUnitRegistrationPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let semantic = self.semantic();
        encoder.map(28)?;
        encode_field(encoder, 1, &semantic.unit())?;
        encoder.field(2)?;
        encoder.text(semantic.diagnostic_path())?;
        encoder.field(3)?;
        encode_initialization_semantic_schedule(encoder, semantic.schedule())?;
        encode_field(encoder, 4, &semantic.storage())?;
        encode_field(encoder, 5, &semantic.failure_root())?;
        encode_field(encoder, 6, &semantic.initializer())?;
        encode_field(encoder, 7, &semantic.ensure())?;
        encode_array_field(encoder, 8, semantic.dependencies())?;
        encode_field(encoder, 9, &self.registration_symbol())?;
        encode_field(encoder, 10, &self.registration_definition_plan())?;
        encode_field(encoder, 11, &self.registration_primary_atom())?;
        encode_field(encoder, 12, &self.cell_symbol())?;
        encode_field(encoder, 13, &self.cell_definition_plan())?;
        encode_field(encoder, 14, &self.cell_primary_atom())?;
        encode_field(encoder, 15, &self.descriptor_symbol())?;
        encode_field(encoder, 16, &self.descriptor_definition_plan())?;
        encode_field(encoder, 17, &self.descriptor_primary_atom())?;
        encode_field(encoder, 18, &self.diagnostic_atom())?;
        encoder.field(19)?;
        encode_initialization_storage_ref(encoder, self.storage())?;
        encoder.field(20)?;
        encode_initialization_storage_ref(encoder, self.failure_root())?;
        encoder.field(21)?;
        encode_initialization_callable_ref(encoder, self.initializer())?;
        encoder.field(22)?;
        encode_initialization_callable_ref(encoder, self.ensure())?;
        encoder.field(23)?;
        encode_initialization_registration_schedule(encoder, self.schedule())?;
        encode_field(encoder, 24, &self.registration_object_node())?;
        encode_field(encoder, 25, &self.cell_definition_node())?;
        encode_field(encoder, 26, &self.descriptor_definition_node())?;
        encode_field(encoder, 27, &self.registration_fingerprint_node())?;
        encode_field(encoder, 28, &self.registration_definition_patch())
    }
}

fn encode_type_registration_ref(
    encoder: &mut Encoder,
    registration: ImmortalObjectTypeRegistrationRefV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    let (tag, exact_type) = match registration {
        ImmortalObjectTypeRegistrationRefV1::Local(exact_type) => (1, exact_type),
        ImmortalObjectTypeRegistrationRefV1::CoreExternal(exact_type) => (2, exact_type),
    };
    encode_value_sum(encoder, tag, &exact_type)
}

fn encode_type_instance_shape(
    encoder: &mut Encoder,
    shape: &crate::TypeInstanceShapeV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(10)?;
    encode_unsigned_field(encoder, 1, u64::from(shape.instance_kind().tag()))?;
    encode_unsigned_field(encoder, 2, u64::from(shape.inline_storage_kind().tag()))?;
    encode_unsigned_field(encoder, 3, shape.minimum_size())?;
    encode_unsigned_field(encoder, 4, shape.instance_alignment())?;
    encode_unsigned_field(encoder, 5, shape.inline_offset())?;
    encode_unsigned_field(encoder, 6, shape.inline_size())?;
    encode_unsigned_field(encoder, 7, shape.inline_stride())?;
    encode_unsigned_field(encoder, 8, shape.inline_alignment())?;
    encoder.field(9)?;
    encode_ref_scan(encoder, shape.object_scan())?;
    encoder.field(10)?;
    encode_ref_scan(encoder, shape.inline_scan())
}

fn encode_type_descriptor_ref(
    encoder: &mut Encoder,
    reference: StrongTypeDescriptorRefV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match reference {
        StrongTypeDescriptorRefV1::Local(exact_type) => encode_value_sum(encoder, 1, &exact_type),
        StrongTypeDescriptorRefV1::CoreExternal(exact_type) => {
            encode_value_sum(encoder, 2, &exact_type)
        }
    }
}

fn encode_optional_type_descriptor_ref(
    encoder: &mut Encoder,
    reference: Option<StrongTypeDescriptorRefV1>,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match reference {
        None => {
            encoder.map(2)?;
            encode_unsigned_field(encoder, 0, 1)?;
            encode_unsigned_field(encoder, 1, 0)
        }
        Some(StrongTypeDescriptorRefV1::Local(exact_type)) => {
            encode_value_sum(encoder, 2, &exact_type)
        }
        Some(StrongTypeDescriptorRefV1::CoreExternal(exact_type)) => {
            encode_value_sum(encoder, 3, &exact_type)
        }
    }
}

fn encode_type_dispatch_callable_ref(
    encoder: &mut Encoder,
    reference: StrongTypeDispatchCallableRefV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match reference {
        StrongTypeDispatchCallableRefV1::Local(body) => encode_value_sum(encoder, 1, &body),
        StrongTypeDispatchCallableRefV1::CoreExternal(body) => encode_value_sum(encoder, 2, &body),
        StrongTypeDispatchCallableRefV1::Runtime(function) => {
            encoder.map(2)?;
            encode_unsigned_field(encoder, 0, 3)?;
            encoder.field(1)?;
            encoder.map(2)?;
            encode_unsigned_field(encoder, 1, function.wire_family_tag())?;
            encode_unsigned_field(encoder, 2, function.wire_function_tag())
        }
    }
}

fn encode_type_vtable(
    encoder: &mut Encoder,
    vtable: &StrongTypeVtableSemanticPlanV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_field(encoder, 1, &vtable.table())?;
    encoder.field(2)?;
    encoder.array(vtable.slots().len() as u64)?;
    for slot in vtable.slots() {
        encode_type_dispatch_callable_ref(encoder, *slot)?;
    }
    Ok(())
}

fn encode_type_itable(
    encoder: &mut Encoder,
    itable: &StrongTypeItableSemanticPlanV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_field(encoder, 1, &itable.table())?;
    encoder.field(2)?;
    encode_type_descriptor_ref(encoder, itable.interface())?;
    encoder.field(3)?;
    encoder.array(itable.slots().len() as u64)?;
    for slot in itable.slots() {
        encode_type_dispatch_callable_ref(encoder, *slot)?;
    }
    Ok(())
}

fn encode_ref_scan(
    encoder: &mut Encoder,
    scan: &RefScan,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match scan {
        RefScan::None => encode_empty_sum(encoder, 1),
        RefScan::References(offsets) => {
            encoder.map(2)?;
            encode_unsigned_field(encoder, 0, 2)?;
            encoder.field(1)?;
            encoder.array(offsets.len() as u64)?;
            for offset in offsets {
                encoder.unsigned(*offset)?;
            }
            Ok(())
        }
        RefScan::Sequence(parts) => {
            encoder.map(2)?;
            encode_unsigned_field(encoder, 0, 3)?;
            encoder.field(1)?;
            encoder.array(parts.len() as u64)?;
            for part in parts {
                encode_ref_scan(encoder, part)?;
            }
            Ok(())
        }
        RefScan::Array {
            length_offset,
            first_element_offset,
            stride,
            element,
        } => {
            encoder.map(5)?;
            encode_unsigned_field(encoder, 0, 4)?;
            encode_unsigned_field(encoder, 1, *length_offset)?;
            encode_unsigned_field(encoder, 2, *first_element_offset)?;
            encode_unsigned_field(encoder, 3, stride.get())?;
            encoder.field(4)?;
            encode_ref_scan(encoder, element.as_ref_scan())
        }
    }
}

fn encode_static_initial_state(
    encoder: &mut Encoder,
    state: &StrongStaticStorageInitialStatePlanV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match state {
        StrongStaticStorageInitialStatePlanV1::ZeroedForRuntimeUnit => encode_empty_sum(encoder, 1),
        StrongStaticStorageInitialStatePlanV1::EncodedStaticValue {
            initial_template,
            immortal_relocations,
        } => {
            encoder.map(3)?;
            encode_unsigned_field(encoder, 0, 2)?;
            encoder.field(1)?;
            encoder.bytes(initial_template)?;
            encoder.field(2)?;
            encoder.array(immortal_relocations.len() as u64)?;
            for relocation in immortal_relocations {
                encoder.map(2)?;
                encode_unsigned_field(encoder, 1, relocation.pointer_offset())?;
                encode_field(encoder, 2, &relocation.target())?;
            }
            Ok(())
        }
    }
}

fn encode_static_initial_artifacts(
    encoder: &mut Encoder,
    artifacts: StrongStaticStorageInitialArtifactPlanV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match artifacts {
        StrongStaticStorageInitialArtifactPlanV1::ZeroedForRuntimeUnit => {
            encode_empty_sum(encoder, 1)
        }
        StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
            template_atom,
            relocation_table,
        } => {
            encoder.map(3)?;
            encode_unsigned_field(encoder, 0, 2)?;
            encode_field(encoder, 1, &template_atom)?;
            encoder.field(2)?;
            match relocation_table {
                StaticStorageRelocationTableArtifactV1::SharedEmptySentinel => {
                    encode_empty_sum(encoder, 1)
                }
                StaticStorageRelocationTableArtifactV1::Defined { atom } => {
                    encode_value_sum(encoder, 2, &atom)
                }
            }
        }
    }
}

fn encode_initialization_semantic_schedule(
    encoder: &mut Encoder,
    schedule: crate::StrongInitializationSchedulePlanV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match schedule {
        crate::StrongInitializationSchedulePlanV1::EagerStartup { gateway } => {
            encode_value_sum(encoder, 1, &gateway)
        }
        crate::StrongInitializationSchedulePlanV1::LazyAccess => encode_empty_sum(encoder, 2),
    }
}

fn encode_initialization_storage_ref(
    encoder: &mut Encoder,
    reference: StrongInitializationStaticStorageRefPlanV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(6)?;
    encode_field(encoder, 1, &reference.storage())?;
    encode_field(encoder, 2, &reference.storage_symbol())?;
    encode_field(encoder, 3, &reference.registration_symbol())?;
    encode_field(encoder, 4, &reference.registration_definition_plan())?;
    encode_field(encoder, 5, &reference.registration_primary_atom())?;
    encode_field(encoder, 6, &reference.registration_fingerprint_node())
}

fn encode_initialization_callable_ref(
    encoder: &mut Encoder,
    reference: StrongInitializationCallableRefPlanV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(9)?;
    encode_field(encoder, 1, &reference.body())?;
    encode_field(encoder, 2, &reference.entry_symbol())?;
    encode_field(encoder, 3, &reference.registration_symbol())?;
    encode_field(encoder, 4, &reference.body_definition_plan())?;
    encode_field(encoder, 5, &reference.body_primary_atom())?;
    encode_field(encoder, 6, &reference.body_definition_node())?;
    encode_field(encoder, 7, &reference.registration_definition_plan())?;
    encode_field(encoder, 8, &reference.registration_primary_atom())?;
    encode_field(encoder, 9, &reference.registration_fingerprint_node())
}

fn encode_initialization_registration_schedule(
    encoder: &mut Encoder,
    schedule: &StrongInitializationRegistrationSchedulePlanV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match schedule {
        StrongInitializationRegistrationSchedulePlanV1::EagerStartup {
            gateway,
            gateway_definition_patch,
        } => {
            encoder.map(3)?;
            encode_unsigned_field(encoder, 0, 1)?;
            encoder.field(1)?;
            encode_initialization_callable_ref(encoder, **gateway)?;
            encode_field(encoder, 2, gateway_definition_patch)
        }
        StrongInitializationRegistrationSchedulePlanV1::LazyAccess => encode_empty_sum(encoder, 2),
    }
}

fn encode_field(
    encoder: &mut Encoder,
    field: u32,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    value.encode(encoder)
}

fn encode_unsigned_field(
    encoder: &mut Encoder,
    field: u32,
    value: u64,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    encoder.unsigned(value)
}

fn encode_array_field<T: WireEncode>(
    encoder: &mut Encoder,
    field: u32,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_unsigned_field(encoder, 0, tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_unsigned_field(encoder, 0, tag)?;
    encode_field(encoder, 1, value)
}

#[derive(Debug)]
pub enum StrongRegistrationProductionBuildError {
    ProducerMismatch {
        module: scoop_identity::ConeIdentity,
        foundation: scoop_identity::ConeIdentity,
    },
    Identities(StrongRegistrationIdentityBuildError),
    CallableRuntimeScans(StrongCallableRuntimeScanPlanError),
    SafepointSemantics(StrongSafepointSemanticPlanError),
    TypeSemantics(StrongTypeDescriptorSemanticPlanBuildError),
    ImmortalSemantics(StrongImmortalObjectSemanticPlanBuildError),
    InitializationSemantics(StrongInitializationUnitSemanticPlanBuildError),
    Safepoints(StrongSafepointRegistrationPlanBuildError),
    Callables(StrongCallableRegistrationPlanBuildError),
    Types(StrongTypeRegistrationPlanBuildError),
    ImmortalObjects(StrongImmortalObjectRegistrationPlanBuildError),
    StaticStorages(StrongStaticStorageRegistrationPlanBuildError),
    InitializationUnits(StrongInitializationUnitRegistrationPlanBuildError),
}

impl fmt::Display for StrongRegistrationProductionBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong registration production surface: {self:?}"
        )
    }
}

impl std::error::Error for StrongRegistrationProductionBuildError {}
