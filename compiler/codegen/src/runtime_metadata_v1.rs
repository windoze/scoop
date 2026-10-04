//! LLVM mirror of `runtime/include/scoop_runtime_metadata_v1.h`.
//!
//! Field order is defined once here for codegen and checked against the same
//! closed size/alignment/offset golden values enforced by the C header. Target
//! qualification rejects a backend before emission if LLVM cannot represent
//! the current metadata ABI exactly.

use inkwell::AddressSpace;
use inkwell::context::Context;
use inkwell::targets::TargetData;
use inkwell::types::StructType;

use crate::CodegenError;

mod image;
pub(crate) use image::emit_cone_image_v1;
pub use image::{
    EmittedConeImageSupportAtomV1, EmittedConeImageSupportAtomsV1, EmittedConeImageV1,
    RuntimeImagePatchSiteV1,
};

mod entry;
pub(crate) use entry::emit_entry_production_v1;
pub use entry::{EmittedEntryProductionV1, EmittedRootEntryV1, RootEntryPatchSiteV1};

mod safepoint;
pub(crate) use safepoint::emit_strong_safepoint_registrations_v1;
pub use safepoint::{
    EmittedStrongSafepointRegistrationSetV1, EmittedStrongSafepointRegistrationV1,
    SafepointRegistrationPatchSiteV1,
};

mod callable;
pub(crate) use callable::emit_strong_callable_registrations_v1;
pub use callable::{
    CallableRegistrationPatchSiteV1, EmittedStrongCallableRegistrationSetV1,
    EmittedStrongCallableRegistrationV1,
};

mod type_registration;
pub(crate) use type_registration::emit_strong_type_registrations_v1;
pub use type_registration::{
    EmittedStrongTypeRegistrationSetV1, EmittedStrongTypeRegistrationV1,
    TypeRegistrationPatchSiteV1,
};

mod immortal_registration;
pub(crate) use immortal_registration::emit_strong_immortal_object_registrations_v1;
pub use immortal_registration::{
    EmittedStrongImmortalObjectRegistrationSetV1, EmittedStrongImmortalObjectRegistrationV1,
    ImmortalObjectRegistrationPatchSiteV1,
};

mod initialization;
pub(crate) use initialization::emit_strong_initialization_unit_registrations_v1;
pub use initialization::{
    EmittedStrongInitializationUnitRegistrationSetV1,
    EmittedStrongInitializationUnitRegistrationV1, InitializationRegistrationPatchSiteV1,
};

mod static_storage;
pub(crate) use static_storage::emit_strong_static_storage_registrations_v1;
pub use static_storage::{
    EmittedStaticStorageInitialStateV1, EmittedStaticStorageRelocationTableV1,
    EmittedStrongStaticStorageRegistrationSetV1, EmittedStrongStaticStorageRegistrationV1,
    StaticStorageRegistrationPatchSiteV1,
};

mod production;
pub use production::{EmittedStrongRuntimeMetadataV1, ProvisionalStrongDigestPatchLocationV1};
pub(crate) use production::{
    emit_context_callable_metadata_v1, emit_strong_runtime_metadata_v1, validate_patch_coverage,
};

mod registration_identity;
use registration_identity::registration_identity_value;

#[derive(Clone, Copy)]
struct ExpectedField {
    name: &'static str,
    offset: u64,
}

#[derive(Clone, Copy)]
struct ExpectedStruct {
    name: &'static str,
    size: u64,
    alignment: u32,
    fields: &'static [ExpectedField],
}

macro_rules! expected_fields {
    ($($name:literal => $offset:literal),+ $(,)?) => {
        &[$(ExpectedField { name: $name, offset: $offset }),+]
    };
}

const DESCRIPTOR_PREFIX: ExpectedStruct = ExpectedStruct {
    name: "ScoopDescriptorPrefixV1",
    size: 16,
    alignment: 8,
    fields: expected_fields!("magic" => 0, "abi_version" => 8, "struct_size" => 12),
};
const DIGEST: ExpectedStruct = ExpectedStruct {
    name: "ScoopDigest256V1",
    size: 32,
    alignment: 1,
    fields: expected_fields!("bytes" => 0),
};
const BYTE_SPAN: ExpectedStruct = ExpectedStruct {
    name: "ScoopByteSpanV1",
    size: 16,
    alignment: 8,
    fields: expected_fields!("data" => 0, "length" => 8),
};
const CONE_RECORD: ExpectedStruct = ExpectedStruct {
    name: "ScoopConeRecordV1",
    size: 80,
    alignment: 8,
    fields: expected_fields!("group" => 0, "name" => 16, "version" => 32, "identity" => 48),
};
const INITIALIZATION_CELL: ExpectedStruct = ExpectedStruct {
    name: "ScoopInitializationCell",
    size: 16,
    alignment: 8,
    fields: expected_fields!("state" => 0, "owner_thread" => 8),
};
const TYPE_INSTANCE_SHAPE: ExpectedStruct = ExpectedStruct {
    name: "ScoopTypeInstanceShapeV1",
    size: 64,
    alignment: 8,
    fields: expected_fields!(
        "instance_kind" => 0,
        "inline_storage_kind" => 4,
        "minimum_size" => 8,
        "instance_alignment" => 16,
        "inline_offset" => 24,
        "inline_size" => 32,
        "inline_stride" => 40,
        "inline_alignment" => 48,
        "inline_scan" => 56,
    ),
};
const ITABLE_ENTRY: ExpectedStruct = ExpectedStruct {
    name: "ScoopItableEntryV1",
    size: 16,
    alignment: 8,
    fields: expected_fields!("interface" => 0, "slots" => 8),
};
const TYPE_DESCRIPTOR: ExpectedStruct = ExpectedStruct {
    name: "ScoopTypeDescriptor",
    size: 152,
    alignment: 8,
    fields: expected_fields!(
        "type_id" => 0,
        "instance_shape" => 8,
        "object_scan" => 72,
        "parent" => 80,
        "vtable" => 88,
        "itables" => 96,
        "itable_count" => 104,
        "diagnostic_name" => 112,
        "relation_kind" => 128,
        "related_type_count" => 132,
        "function_result" => 136,
        "release_hook" => 144,
    ),
};
const REGISTRATION_IDENTITY: ExpectedStruct = ExpectedStruct {
    name: "ScoopRegistrationIdentityV1",
    size: 136,
    alignment: 4,
    fields: expected_fields!(
        "linkage_kind" => 0,
        "reserved_zero" => 4,
        "semantic_id" => 8,
        "odr_group_id" => 40,
        "odr_member_id" => 72,
        "definition_fingerprint" => 104,
    ),
};
const STATIC_IMMORTAL_RELOCATION: ExpectedStruct = ExpectedStruct {
    name: "ScoopStaticImmortalRelocationV1",
    size: 16,
    alignment: 8,
    fields: expected_fields!("pointer_offset" => 0, "target" => 8),
};
const STATIC_STORAGE_DESCRIPTOR: ExpectedStruct = ExpectedStruct {
    name: "ScoopStaticStorageDescriptorV1",
    size: 296,
    alignment: 8,
    fields: expected_fields!(
        "prefix" => 0,
        "registration" => 16,
        "scan_kind" => 152,
        "initial_state_kind" => 156,
        "writable_base" => 160,
        "byte_size" => 168,
        "allocation_extent" => 176,
        "required_alignment" => 184,
        "scan_program" => 192,
        "scan_fingerprint" => 200,
        "layout_fingerprint" => 232,
        "initial_template" => 264,
        "initial_relocations" => 280,
        "initial_relocation_count" => 288,
    ),
};
const TYPE_REGISTRATION_DESCRIPTOR: ExpectedStruct = ExpectedStruct {
    name: "ScoopTypeRegistrationDescriptorV1",
    size: 240,
    alignment: 8,
    fields: expected_fields!(
        "prefix" => 0,
        "registration" => 16,
        "runtime_type_id" => 152,
        "reserved_zero" => 160,
        "descriptor" => 168,
        "descriptor_fingerprint" => 176,
        "layout_fingerprint" => 208,
    ),
};
const CALLABLE_REGISTRATION_DESCRIPTOR: ExpectedStruct = ExpectedStruct {
    name: "ScoopCallableRegistrationDescriptorV1",
    size: 208,
    alignment: 8,
    fields: expected_fields!(
        "prefix" => 0,
        "registration" => 16,
        "body_definition_fingerprint" => 152,
        "entry" => 184,
        "context_keys" => 192,
        "context_key_count" => 200,
    ),
};
const IMMORTAL_OBJECT_DESCRIPTOR: ExpectedStruct = ExpectedStruct {
    name: "ScoopImmortalObjectDescriptorV1",
    size: 184,
    alignment: 8,
    fields: expected_fields!(
        "prefix" => 0,
        "registration" => 16,
        "object_start" => 152,
        "object_size" => 160,
        "required_alignment" => 168,
        "type_registration" => 176,
    ),
};
const INITIALIZATION_UNIT_DESCRIPTOR: ExpectedStruct = ExpectedStruct {
    name: "ScoopInitializationUnitDescriptorV1",
    size: 352,
    alignment: 8,
    fields: expected_fields!(
        "prefix" => 0,
        "registration" => 16,
        "schedule_kind" => 152,
        "reserved_zero" => 156,
        "diagnostic_path" => 160,
        "cell" => 176,
        "storage" => 184,
        "failure_root" => 192,
        "initializer_callable_id" => 200,
        "ensure_callable_id" => 232,
        "initializer_entry" => 264,
        "ensure_entry" => 272,
        "startup_gateway_callable_id" => 280,
        "startup_gateway_definition_fingerprint" => 312,
        "startup_gateway" => 344,
    ),
};
const SAFEPOINT_REGISTRATION_DESCRIPTOR: ExpectedStruct = ExpectedStruct {
    name: "ScoopSafepointRegistrationDescriptorV1",
    size: 232,
    alignment: 8,
    fields: expected_fields!(
        "prefix" => 0,
        "registration" => 16,
        "safepoint_id" => 152,
        "site_role" => 160,
        "root_pair_count" => 164,
        "owner_callable_id" => 168,
        "normalized_stackmap_fingerprint" => 200,
    ),
};
const ROOT_ENTRY_DESCRIPTOR: ExpectedStruct = ExpectedStruct {
    name: "ScoopRootEntryDescriptorV1",
    size: 192,
    alignment: 8,
    fields: expected_fields!(
        "prefix" => 0,
        "owner_cone_identity" => 16,
        "callable_id" => 48,
        "source_signature_fingerprint" => 80,
        "gateway_callable_id" => 112,
        "gateway_definition_fingerprint" => 144,
        "failure_root" => 176,
        "gateway" => 184,
    ),
};
const IMAGE_DESCRIPTOR: ExpectedStruct = ExpectedStruct {
    name: "ScoopImageDescriptorV1",
    size: 240,
    alignment: 8,
    fields: expected_fields!(
        "prefix" => 0,
        "cone" => 16,
        "runtime_image_fingerprint" => 96,
        "dependencies" => 128,
        "dependency_count" => 136,
        "static_storages" => 144,
        "static_storage_count" => 152,
        "immortal_objects" => 160,
        "immortal_object_count" => 168,
        "initialization_units" => 176,
        "initialization_unit_count" => 184,
        "type_registrations" => 192,
        "type_registration_count" => 200,
        "safepoints" => 208,
        "safepoint_count" => 216,
        "callables" => 224,
        "callable_count" => 232,
    ),
};
/// All LLVM aggregate types in the private runtime metadata ABI.
pub(crate) struct RuntimeMetadataV1Types<'ctx> {
    descriptor_prefix: StructType<'ctx>,
    digest: StructType<'ctx>,
    byte_span: StructType<'ctx>,
    cone_record: StructType<'ctx>,
    initialization_cell: StructType<'ctx>,
    type_instance_shape: StructType<'ctx>,
    itable_entry: StructType<'ctx>,
    type_descriptor: StructType<'ctx>,
    registration_identity: StructType<'ctx>,
    static_immortal_relocation: StructType<'ctx>,
    static_storage_descriptor: StructType<'ctx>,
    type_registration_descriptor: StructType<'ctx>,
    callable_registration_descriptor: StructType<'ctx>,
    immortal_object_descriptor: StructType<'ctx>,
    initialization_unit_descriptor: StructType<'ctx>,
    safepoint_registration_descriptor: StructType<'ctx>,
    root_entry_descriptor: StructType<'ctx>,
    image_descriptor: StructType<'ctx>,
}

impl<'ctx> RuntimeMetadataV1Types<'ctx> {
    pub(crate) fn new(context: &'ctx Context) -> Self {
        let i8 = context.i8_type();
        let i32 = context.i32_type();
        let i64 = context.i64_type();
        let ptr = context.ptr_type(AddressSpace::default());
        let descriptor_prefix = context.struct_type(&[i64.into(), i32.into(), i32.into()], false);
        let digest = context.struct_type(&[i8.array_type(32).into()], false);
        let byte_span = context.struct_type(&[ptr.into(), i64.into()], false);
        let cone_record = context.struct_type(
            &[
                byte_span.into(),
                byte_span.into(),
                byte_span.into(),
                digest.into(),
            ],
            false,
        );
        let initialization_cell = context.struct_type(&[i64.into(), ptr.into()], false);
        let type_instance_shape = context.struct_type(
            &[
                i32.into(),
                i32.into(),
                i64.into(),
                i64.into(),
                i64.into(),
                i64.into(),
                i64.into(),
                i64.into(),
                ptr.into(),
            ],
            false,
        );
        let itable_entry = context.struct_type(&[ptr.into(), ptr.into()], false);
        let type_descriptor = context.struct_type(
            &[
                i64.into(),
                type_instance_shape.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
                i64.into(),
                byte_span.into(),
                i32.into(),
                i32.into(),
                ptr.into(),
                ptr.into(),
            ],
            false,
        );
        let registration_identity = context.struct_type(
            &[
                i32.into(),
                i32.into(),
                digest.into(),
                digest.into(),
                digest.into(),
                digest.into(),
            ],
            false,
        );
        let static_immortal_relocation = context.struct_type(&[i64.into(), ptr.into()], false);
        let static_storage_descriptor = context.struct_type(
            &[
                descriptor_prefix.into(),
                registration_identity.into(),
                i32.into(),
                i32.into(),
                ptr.into(),
                i64.into(),
                i64.into(),
                i64.into(),
                ptr.into(),
                digest.into(),
                digest.into(),
                byte_span.into(),
                ptr.into(),
                i64.into(),
            ],
            false,
        );
        let type_registration_descriptor = context.struct_type(
            &[
                descriptor_prefix.into(),
                registration_identity.into(),
                i64.into(),
                i64.into(),
                ptr.into(),
                digest.into(),
                digest.into(),
            ],
            false,
        );
        let callable_registration_descriptor = context.struct_type(
            &[
                descriptor_prefix.into(),
                registration_identity.into(),
                digest.into(),
                ptr.into(),
                ptr.into(),
                i64.into(),
            ],
            false,
        );
        let immortal_object_descriptor = context.struct_type(
            &[
                descriptor_prefix.into(),
                registration_identity.into(),
                ptr.into(),
                i64.into(),
                i64.into(),
                ptr.into(),
            ],
            false,
        );
        let initialization_unit_descriptor = context.struct_type(
            &[
                descriptor_prefix.into(),
                registration_identity.into(),
                i32.into(),
                i32.into(),
                byte_span.into(),
                ptr.into(),
                ptr.into(),
                ptr.into(),
                digest.into(),
                digest.into(),
                ptr.into(),
                ptr.into(),
                digest.into(),
                digest.into(),
                ptr.into(),
            ],
            false,
        );
        let safepoint_registration_descriptor = context.struct_type(
            &[
                descriptor_prefix.into(),
                registration_identity.into(),
                i64.into(),
                i32.into(),
                i32.into(),
                digest.into(),
                digest.into(),
            ],
            false,
        );
        let root_entry_descriptor = context.struct_type(
            &[
                descriptor_prefix.into(),
                digest.into(),
                digest.into(),
                digest.into(),
                digest.into(),
                digest.into(),
                ptr.into(),
                ptr.into(),
            ],
            false,
        );
        let image_descriptor = context.struct_type(
            &[
                descriptor_prefix.into(),
                cone_record.into(),
                digest.into(),
                ptr.into(),
                i64.into(),
                ptr.into(),
                i64.into(),
                ptr.into(),
                i64.into(),
                ptr.into(),
                i64.into(),
                ptr.into(),
                i64.into(),
                ptr.into(),
                i64.into(),
                ptr.into(),
                i64.into(),
            ],
            false,
        );
        Self {
            descriptor_prefix,
            digest,
            byte_span,
            cone_record,
            initialization_cell,
            type_instance_shape,
            itable_entry,
            type_descriptor,
            registration_identity,
            static_immortal_relocation,
            static_storage_descriptor,
            type_registration_descriptor,
            callable_registration_descriptor,
            immortal_object_descriptor,
            initialization_unit_descriptor,
            safepoint_registration_descriptor,
            root_entry_descriptor,
            image_descriptor,
        }
    }

    pub(crate) fn validate_layout(&self, target_data: &TargetData) -> Result<(), CodegenError> {
        for (expected, actual) in [
            (DESCRIPTOR_PREFIX, self.descriptor_prefix),
            (DIGEST, self.digest),
            (BYTE_SPAN, self.byte_span),
            (CONE_RECORD, self.cone_record),
            (INITIALIZATION_CELL, self.initialization_cell),
            (TYPE_INSTANCE_SHAPE, self.type_instance_shape),
            (ITABLE_ENTRY, self.itable_entry),
            (TYPE_DESCRIPTOR, self.type_descriptor),
            (REGISTRATION_IDENTITY, self.registration_identity),
            (STATIC_IMMORTAL_RELOCATION, self.static_immortal_relocation),
            (STATIC_STORAGE_DESCRIPTOR, self.static_storage_descriptor),
            (
                TYPE_REGISTRATION_DESCRIPTOR,
                self.type_registration_descriptor,
            ),
            (
                CALLABLE_REGISTRATION_DESCRIPTOR,
                self.callable_registration_descriptor,
            ),
            (IMMORTAL_OBJECT_DESCRIPTOR, self.immortal_object_descriptor),
            (
                INITIALIZATION_UNIT_DESCRIPTOR,
                self.initialization_unit_descriptor,
            ),
            (
                SAFEPOINT_REGISTRATION_DESCRIPTOR,
                self.safepoint_registration_descriptor,
            ),
            (ROOT_ENTRY_DESCRIPTOR, self.root_entry_descriptor),
            (IMAGE_DESCRIPTOR, self.image_descriptor),
        ] {
            validate_struct_layout(target_data, expected, actual)?;
        }
        Ok(())
    }

    pub(crate) const fn initialization_unit_descriptor(&self) -> StructType<'ctx> {
        self.initialization_unit_descriptor
    }

    pub(crate) const fn byte_span(&self) -> StructType<'ctx> {
        self.byte_span
    }

    pub(crate) const fn type_instance_shape(&self) -> StructType<'ctx> {
        self.type_instance_shape
    }

    pub(crate) const fn itable_entry(&self) -> StructType<'ctx> {
        self.itable_entry
    }

    pub(crate) const fn type_descriptor(&self) -> StructType<'ctx> {
        self.type_descriptor
    }

    pub(crate) fn type_descriptor_storage(&self, parameter_count: u32) -> StructType<'ctx> {
        if parameter_count == 0 {
            return self.type_descriptor;
        }
        let context = self.type_descriptor.get_context();
        let mut fields = self.type_descriptor.get_field_types().to_vec();
        fields.push(
            context
                .ptr_type(AddressSpace::default())
                .array_type(parameter_count)
                .into(),
        );
        context.struct_type(&fields, false)
    }
}

fn validate_struct_layout(
    target_data: &TargetData,
    expected: ExpectedStruct,
    actual: StructType<'_>,
) -> Result<(), CodegenError> {
    let actual_size = target_data.get_abi_size(&actual);
    let actual_alignment = target_data.get_abi_alignment(&actual);
    if actual_size != expected.size || actual_alignment != expected.alignment {
        return Err(CodegenError(format!(
            "LLVM runtime metadata type `{}` has size/alignment {actual_size}/{actual_alignment}, expected {}/{}",
            expected.name, expected.size, expected.alignment,
        )));
    }
    if actual.count_fields() != expected.fields.len() as u32 {
        return Err(CodegenError(format!(
            "LLVM runtime metadata type `{}` has {} fields, expected {}",
            expected.name,
            actual.count_fields(),
            expected.fields.len(),
        )));
    }
    for (index, field) in expected.fields.iter().enumerate() {
        let actual_offset = target_data
            .offset_of_element(&actual, index as u32)
            .ok_or_else(|| {
                CodegenError(format!(
                    "LLVM runtime metadata type `{}` has no field `{}` at index {index}",
                    expected.name, field.name,
                ))
            })?;
        if actual_offset != field.offset {
            return Err(CodegenError(format!(
                "LLVM runtime metadata field `{}.{}` has offset {actual_offset}, expected {}",
                expected.name, field.name, field.offset,
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use inkwell::context::Context;
    use inkwell::targets::TargetData;

    use super::RuntimeMetadataV1Types;

    #[test]
    fn frozen_metadata_layout_matches_the_qualified_data_layout() {
        let context = Context::create();
        let target_data = TargetData::create(
            scoop_lir::LirTargetProfile::DARWIN_AARCH64.canonical_llvm_data_layout(),
        );
        RuntimeMetadataV1Types::new(&context)
            .validate_layout(&target_data)
            .expect("qualified target data must reproduce the private C ABI");
    }

    #[test]
    fn metadata_layout_rejects_a_non_64_bit_pointer_profile() {
        let context = Context::create();
        let target_data = TargetData::create("e-p:32:32-i64:64");
        let error = RuntimeMetadataV1Types::new(&context)
            .validate_layout(&target_data)
            .expect_err("32-bit pointers must not satisfy metadata v1");
        assert!(error.0.contains("ScoopItableEntryV1"), "{error}");
    }
}
