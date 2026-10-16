use inkwell::GlobalVisibility;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::types::{IntType, StructType};
use inkwell::values::{GlobalValue, StructValue};
use scoop_lir::{
    ConeImagePlanV1, DigestPatchIntentId, ObjectDefinitionAtomId, ObjectDefinitionPlanId,
    PersistentSymbolRequest,
};

use super::RuntimeMetadataV1Types;

const METADATA_ABI_VERSION: u64 = scoop_lir::RUNTIME_METADATA_ABI_VERSION_V1 as u64;
const IMAGE_DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5049_4d47;
const IMAGE_DESCRIPTOR_SIZE: u64 = 240;
const RUNTIME_IMAGE_FINGERPRINT_OFFSET: u64 = 96;
const DIGEST_SIZE: u64 = 32;

/// The sole graph-managed patch site inside one emitted image descriptor.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeImagePatchSiteV1<'ctx> {
    intent: DigestPatchIntentId,
    definition: ObjectDefinitionPlanId,
    atom: ObjectDefinitionAtomId,
    owner: GlobalValue<'ctx>,
}

impl<'ctx> RuntimeImagePatchSiteV1<'ctx> {
    pub const fn intent(self) -> DigestPatchIntentId {
        self.intent
    }

    pub const fn definition(self) -> ObjectDefinitionPlanId {
        self.definition
    }

    pub const fn atom(self) -> ObjectDefinitionAtomId {
        self.atom
    }

    pub const fn owner(self) -> GlobalValue<'ctx> {
        self.owner
    }

    pub const fn byte_offset(self) -> u64 {
        RUNTIME_IMAGE_FINGERPRINT_OFFSET
    }

    pub const fn byte_size(self) -> u64 {
        DIGEST_SIZE
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EmittedConeImageSupportAtomV1<'ctx> {
    atom: ObjectDefinitionAtomId,
    global: GlobalValue<'ctx>,
}

impl<'ctx> EmittedConeImageSupportAtomV1<'ctx> {
    pub const fn atom(self) -> ObjectDefinitionAtomId {
        self.atom
    }

    pub const fn global(self) -> GlobalValue<'ctx> {
        self.global
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EmittedConeImageSupportAtomsV1<'ctx> {
    coordinate_group: EmittedConeImageSupportAtomV1<'ctx>,
    coordinate_name: EmittedConeImageSupportAtomV1<'ctx>,
    coordinate_version: EmittedConeImageSupportAtomV1<'ctx>,
    dependencies: EmittedConeImageSupportAtomV1<'ctx>,
    static_storages: EmittedConeImageSupportAtomV1<'ctx>,
    immortal_objects: EmittedConeImageSupportAtomV1<'ctx>,
    initialization_units: EmittedConeImageSupportAtomV1<'ctx>,
    type_registrations: EmittedConeImageSupportAtomV1<'ctx>,
    safepoints: EmittedConeImageSupportAtomV1<'ctx>,
    callables: EmittedConeImageSupportAtomV1<'ctx>,
    array_bounds_message: EmittedConeImageSupportAtomV1<'ctx>,
    array_size_overflow_message: EmittedConeImageSupportAtomV1<'ctx>,
}

impl<'ctx> EmittedConeImageSupportAtomsV1<'ctx> {
    pub const fn coordinate_group(self) -> EmittedConeImageSupportAtomV1<'ctx> {
        self.coordinate_group
    }

    pub const fn coordinate_name(self) -> EmittedConeImageSupportAtomV1<'ctx> {
        self.coordinate_name
    }

    pub const fn coordinate_version(self) -> EmittedConeImageSupportAtomV1<'ctx> {
        self.coordinate_version
    }

    pub const fn dependencies(self) -> EmittedConeImageSupportAtomV1<'ctx> {
        self.dependencies
    }

    pub const fn static_storages(self) -> EmittedConeImageSupportAtomV1<'ctx> {
        self.static_storages
    }

    pub const fn immortal_objects(self) -> EmittedConeImageSupportAtomV1<'ctx> {
        self.immortal_objects
    }

    pub const fn initialization_units(self) -> EmittedConeImageSupportAtomV1<'ctx> {
        self.initialization_units
    }

    pub const fn type_registrations(self) -> EmittedConeImageSupportAtomV1<'ctx> {
        self.type_registrations
    }

    pub const fn safepoints(self) -> EmittedConeImageSupportAtomV1<'ctx> {
        self.safepoints
    }

    pub const fn callables(self) -> EmittedConeImageSupportAtomV1<'ctx> {
        self.callables
    }

    pub const fn array_bounds_message(self) -> EmittedConeImageSupportAtomV1<'ctx> {
        self.array_bounds_message
    }

    pub const fn array_size_overflow_message(self) -> EmittedConeImageSupportAtomV1<'ctx> {
        self.array_size_overflow_message
    }
}

/// Fully emitted per-Cone image and its single provisional digest slot.
#[derive(Clone, Copy, Debug)]
pub struct EmittedConeImageV1<'ctx> {
    image: GlobalValue<'ctx>,
    primary_atom: ObjectDefinitionAtomId,
    support_atoms: EmittedConeImageSupportAtomsV1<'ctx>,
    patch: RuntimeImagePatchSiteV1<'ctx>,
}

impl<'ctx> EmittedConeImageV1<'ctx> {
    pub const fn image(self) -> GlobalValue<'ctx> {
        self.image
    }

    pub const fn primary_atom(self) -> ObjectDefinitionAtomId {
        self.primary_atom
    }

    pub const fn support_atoms(self) -> EmittedConeImageSupportAtomsV1<'ctx> {
        self.support_atoms
    }

    pub const fn patch(self) -> RuntimeImagePatchSiteV1<'ctx> {
        self.patch
    }
}

/// Emit the canonical runtime metadata v1 image for one validated Cone plan.
///
/// Registration records may be defined before or after this call in the same
/// LLVM module. This routine declares their exact strong symbols and types;
/// object production resolves the referenced definitions.
pub(crate) fn emit_cone_image_v1<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    plan: &ConeImagePlanV1,
    registrations: &scoop_lir::RegistrationIdentitySurfaceV1,
    array_bounds_message: GlobalValue<'ctx>,
    array_size_overflow_message: GlobalValue<'ctx>,
) -> EmittedConeImageV1<'ctx> {
    let types = RuntimeMetadataV1Types::new(context);
    let image_symbol = plan.symbol().symbol();
    let support_atoms = plan.support_atoms();
    let coordinate = plan.cone().coordinate();
    let prefix = format!("{}.metadata", image_symbol.as_str());
    let (group, group_global) = emit_byte_span(
        context,
        llvm,
        types.byte_span,
        &format!("{prefix}.coordinate.group"),
        coordinate.group().as_bytes(),
    );
    let (name, name_global) = emit_byte_span(
        context,
        llvm,
        types.byte_span,
        &format!("{prefix}.coordinate.name"),
        coordinate.name().as_bytes(),
    );
    let (version, version_global) = emit_byte_span(
        context,
        llvm,
        types.byte_span,
        &format!("{prefix}.coordinate.version"),
        coordinate.version().as_bytes(),
    );
    let cone = types.cone_record.const_named_struct(&[
        group.into(),
        name.into(),
        version.into(),
        digest_value(
            context.i8_type(),
            types.digest,
            plan.cone().identity().as_array(),
        )
        .into(),
    ]);

    let dependencies = plan
        .dependencies()
        .iter()
        .map(|identity| digest_value(context.i8_type(), types.digest, identity.as_array()))
        .collect::<Vec<_>>();
    let dependency_table = emit_digest_table(
        llvm,
        types.digest,
        &format!("{prefix}.dependencies"),
        dependencies,
    );

    let tables = plan.tables();
    let static_storages = emit_registration_table(
        llvm,
        types.static_storage_descriptor,
        &format!("{prefix}.static_storages"),
        registrations.static_storage_symbol_requests(),
    );
    let immortal_objects = emit_registration_table(
        llvm,
        types.immortal_object_descriptor,
        &format!("{prefix}.immortal_objects"),
        registrations.immortal_object_symbol_requests(),
    );
    let initialization_units = emit_registration_table(
        llvm,
        types.initialization_unit_descriptor,
        &format!("{prefix}.initialization_units"),
        registrations.initialization_unit_symbol_requests(),
    );
    let type_registrations = emit_registration_table(
        llvm,
        types.type_registration_descriptor,
        &format!("{prefix}.type_registrations"),
        registrations.type_registration_symbol_requests(),
    );
    let safepoints = emit_registration_table(
        llvm,
        types.safepoint_registration_descriptor,
        &format!("{prefix}.safepoints"),
        registrations.safepoint_symbol_requests(),
    );
    let callables = emit_registration_table(
        llvm,
        types.callable_registration_descriptor,
        &format!("{prefix}.callables"),
        registrations.callable_symbol_requests(),
    );

    let i32 = context.i32_type();
    let i64 = context.i64_type();
    let prefix_value = types.descriptor_prefix.const_named_struct(&[
        i64.const_int(IMAGE_DESCRIPTOR_MAGIC, false).into(),
        i32.const_int(METADATA_ABI_VERSION, false).into(),
        i32.const_int(IMAGE_DESCRIPTOR_SIZE, false).into(),
    ]);
    let zero_digest = types.digest.const_zero();
    let image_value = types.image_descriptor.const_named_struct(&[
        prefix_value.into(),
        cone.into(),
        zero_digest.into(),
        dependency_table.as_pointer_value().into(),
        i64.const_int(plan.dependencies().len() as u64, false)
            .into(),
        static_storages.as_pointer_value().into(),
        i64.const_int(tables.static_storages().len() as u64, false)
            .into(),
        immortal_objects.as_pointer_value().into(),
        i64.const_int(tables.immortal_objects().len() as u64, false)
            .into(),
        initialization_units.as_pointer_value().into(),
        i64.const_int(tables.initialization_units().len() as u64, false)
            .into(),
        type_registrations.as_pointer_value().into(),
        i64.const_int(tables.type_registrations().len() as u64, false)
            .into(),
        safepoints.as_pointer_value().into(),
        i64.const_int(tables.safepoints().len() as u64, false)
            .into(),
        callables.as_pointer_value().into(),
        i64.const_int(tables.callables().len() as u64, false).into(),
    ]);
    let image = llvm.add_global(types.image_descriptor, None, image_symbol.as_str());
    image.set_linkage(Linkage::External);
    image.set_visibility(GlobalVisibility::Hidden);
    image.set_constant(true);
    image.set_initializer(&image_value);

    EmittedConeImageV1 {
        image,
        primary_atom: plan.primary_atom(),
        support_atoms: EmittedConeImageSupportAtomsV1 {
            coordinate_group: emitted_support_atom(support_atoms.coordinate_group(), group_global),
            coordinate_name: emitted_support_atom(support_atoms.coordinate_name(), name_global),
            coordinate_version: emitted_support_atom(
                support_atoms.coordinate_version(),
                version_global,
            ),
            dependencies: emitted_support_atom(support_atoms.dependencies(), dependency_table),
            static_storages: emitted_support_atom(support_atoms.static_storages(), static_storages),
            immortal_objects: emitted_support_atom(
                support_atoms.immortal_objects(),
                immortal_objects,
            ),
            initialization_units: emitted_support_atom(
                support_atoms.initialization_units(),
                initialization_units,
            ),
            type_registrations: emitted_support_atom(
                support_atoms.type_registrations(),
                type_registrations,
            ),
            safepoints: emitted_support_atom(support_atoms.safepoints(), safepoints),
            callables: emitted_support_atom(support_atoms.callables(), callables),
            array_bounds_message: emitted_support_atom(
                support_atoms.array_bounds_message(),
                array_bounds_message,
            ),
            array_size_overflow_message: emitted_support_atom(
                support_atoms.array_size_overflow_message(),
                array_size_overflow_message,
            ),
        },
        patch: RuntimeImagePatchSiteV1 {
            intent: plan.fingerprint_patch(),
            definition: plan.definition_plan(),
            atom: plan.primary_atom(),
            owner: image,
        },
    }
}

fn emit_byte_span<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    span_type: StructType<'ctx>,
    symbol: &str,
    bytes: &[u8],
) -> (StructValue<'ctx>, GlobalValue<'ctx>) {
    let initializer = context.const_string(bytes, false);
    let global = llvm.add_global(initializer.get_type(), None, symbol);
    global.set_linkage(Linkage::Private);
    global.set_constant(true);
    global.set_initializer(&initializer);
    (
        span_type.const_named_struct(&[
            global.as_pointer_value().into(),
            context
                .i64_type()
                .const_int(bytes.len() as u64, false)
                .into(),
        ]),
        global,
    )
}

fn digest_value<'ctx>(
    i8: IntType<'ctx>,
    digest_type: StructType<'ctx>,
    bytes: &[u8; 32],
) -> StructValue<'ctx> {
    let values = bytes
        .iter()
        .map(|byte| i8.const_int(u64::from(*byte), false))
        .collect::<Vec<_>>();
    digest_type.const_named_struct(&[i8.const_array(&values).into()])
}

fn emit_digest_table<'ctx>(
    llvm: &LlvmModule<'ctx>,
    digest_type: StructType<'ctx>,
    symbol: &str,
    mut values: Vec<StructValue<'ctx>>,
) -> GlobalValue<'ctx> {
    if values.is_empty() {
        values.push(digest_type.const_zero());
    }
    emit_constant_array(llvm, symbol, digest_type.const_array(&values).into())
}

fn emit_registration_table<'ctx>(
    llvm: &LlvmModule<'ctx>,
    record_type: StructType<'ctx>,
    table_symbol: &str,
    requests: impl IntoIterator<Item = PersistentSymbolRequest>,
) -> GlobalValue<'ctx> {
    let pointer_type = llvm.get_context().ptr_type(Default::default());
    let mut records = Vec::new();
    for request in requests {
        let symbol = request.symbol();
        let record = if let Some(record) = llvm.get_global(symbol.as_str()) {
            record
        } else {
            let record = llvm.add_global(record_type, None, symbol.as_str());
            record.set_linkage(Linkage::External);
            record
        };
        records.push(record.as_pointer_value());
    }
    if records.is_empty() {
        records.push(pointer_type.const_null());
    }
    emit_constant_array(
        llvm,
        table_symbol,
        pointer_type.const_array(&records).into(),
    )
}

fn emit_constant_array<'ctx>(
    llvm: &LlvmModule<'ctx>,
    symbol: &str,
    initializer: inkwell::values::BasicValueEnum<'ctx>,
) -> GlobalValue<'ctx> {
    let global = llvm.add_global(initializer.get_type(), None, symbol);
    global.set_linkage(Linkage::Private);
    global.set_constant(true);
    global.set_initializer(&initializer);
    global
}

fn emitted_support_atom<'ctx>(
    atom: ObjectDefinitionAtomId,
    global: GlobalValue<'ctx>,
) -> EmittedConeImageSupportAtomV1<'ctx> {
    EmittedConeImageSupportAtomV1 { atom, global }
}

#[cfg(test)]
mod tests;
