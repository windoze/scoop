use inkwell::GlobalVisibility;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::types::{AnyType, IntType, StructType};
use inkwell::values::{AnyValue, GlobalValue, StructValue};
use scoop_lir::{
    ConeImagePlanV1, DigestPatchIntentId, LinkageClass, ObjectDefinitionAtomId,
    ObjectDefinitionPlanId, PersistentSymbolKey, PersistentSymbolRequest,
};

use super::RuntimeMetadataV1Types;
use crate::CodegenError;

const METADATA_ABI_VERSION: u64 = 1;
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
/// final object verification requires every declaration to resolve to a
/// definition owned by the current Cone.
pub(crate) fn emit_cone_image_v1<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    plan: &ConeImagePlanV1,
    array_bounds_message: GlobalValue<'ctx>,
    array_size_overflow_message: GlobalValue<'ctx>,
) -> Result<EmittedConeImageV1<'ctx>, CodegenError> {
    let types = RuntimeMetadataV1Types::new(context);
    let image_symbol = plan.symbol().symbol();
    if plan.symbol().linkage() != LinkageClass::ConeStrong {
        return Err(CodegenError(format!(
            "Cone image `{image_symbol}` does not have strong Cone linkage"
        )));
    }
    if llvm.get_global(image_symbol.as_str()).is_some() {
        return Err(CodegenError(format!(
            "Cone image `{image_symbol}` is already declared"
        )));
    }
    let support_atoms = plan.support_atoms();
    validate_trap_message(
        context,
        support_atoms.array_bounds_message(),
        array_bounds_message,
        b"array index out of bounds",
    )?;
    validate_trap_message(
        context,
        support_atoms.array_size_overflow_message(),
        array_size_overflow_message,
        b"array size overflow",
    )?;

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
        tables.static_storage_symbol_requests(),
    )?;
    let immortal_objects = emit_registration_table(
        llvm,
        types.immortal_object_descriptor,
        &format!("{prefix}.immortal_objects"),
        tables.immortal_object_symbol_requests(),
    )?;
    let initialization_units = emit_registration_table(
        llvm,
        types.initialization_unit_descriptor,
        &format!("{prefix}.initialization_units"),
        tables.initialization_unit_symbol_requests(),
    )?;
    let type_registrations = emit_registration_table(
        llvm,
        types.type_registration_descriptor,
        &format!("{prefix}.type_registrations"),
        tables.type_registration_symbol_requests(),
    )?;
    let safepoints = emit_registration_table(
        llvm,
        types.safepoint_registration_descriptor,
        &format!("{prefix}.safepoints"),
        tables.safepoint_symbol_requests(),
    )?;
    let callables = emit_registration_table(
        llvm,
        types.callable_registration_descriptor,
        &format!("{prefix}.callables"),
        tables.callable_symbol_requests(),
    )?;

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

    Ok(EmittedConeImageV1 {
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
    })
}

fn validate_trap_message(
    context: &Context,
    atom: ObjectDefinitionAtomId,
    global: GlobalValue<'_>,
    bytes: &[u8],
) -> Result<(), CodegenError> {
    let request = PersistentSymbolRequest::new(
        PersistentSymbolKey::DefinitionBoundaryStart(atom),
        LinkageClass::ConeStrong,
    )
    .map_err(|error| CodegenError(format!("cannot derive trap-message boundary: {error}")))?;
    let expected = context.const_string(bytes, true);
    let initializer = global.get_initializer();
    if global.get_name().to_bytes() != request.symbol().as_str().as_bytes()
        || global.get_value_type() != expected.get_type().as_any_type_enum()
        || global.get_linkage() != Linkage::External
        || !global.is_constant()
        || initializer.map(|value| value.print_to_string()).as_ref()
            != Some(&expected.print_to_string())
    {
        return Err(CodegenError(format!(
            "Cone trap-message atom {atom} does not match its canonical strong definition"
        )));
    }
    Ok(())
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
) -> Result<GlobalValue<'ctx>, CodegenError> {
    let pointer_type = llvm.get_context().ptr_type(Default::default());
    let mut records = Vec::new();
    for request in requests {
        if request.linkage() != LinkageClass::ConeStrong {
            return Err(CodegenError(format!(
                "registration `{}` does not have strong Cone linkage",
                request.symbol()
            )));
        }
        let symbol = request.symbol();
        let record = if let Some(record) = llvm.get_global(symbol.as_str()) {
            if record.get_value_type() != record_type.as_any_type_enum()
                || record.get_linkage() != Linkage::External
            {
                return Err(CodegenError(format!(
                    "registration `{symbol}` has an incompatible LLVM declaration"
                )));
            }
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
    Ok(emit_constant_array(
        llvm,
        table_symbol,
        pointer_type.const_array(&records).into(),
    ))
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
mod tests {
    use inkwell::GlobalVisibility;
    use inkwell::context::Context;
    use inkwell::module::Linkage;
    use inkwell::values::AnyValue;
    use scoop_identity::{
        CborIdentityRecord, ConeCoordinate, ConeImageSupportRole, CoreBuiltinNominal,
        DefinitionAtomRole, DefinitionAtomSubkey, DigestNodeId, DigestNodeKey,
        DigestPatchIntentKey, DigestSemanticFieldRole, ExactTypeKey, LinkageClass,
        ObjectDefinitionAtomKey, ObjectDefinitionPlanKey, PersistentExactTypeId,
        PersistentSymbolKey, PersistentSymbolRequest, PersistentSymbolRequestTable,
        StrongDefinitionEntity, StrongDefinitionRole,
    };
    use scoop_lir::{
        CanonicalLirFoundation, ConeImagePlanV1, DigestInputRefV1, DigestNodeV1,
        OdrFreeLirFoundation, StrongDigestFinalizationPlanV1, StrongRegistrationIdentitySurfaceV1,
    };

    use super::{DIGEST_SIZE, IMAGE_DESCRIPTOR_SIZE, emit_cone_image_v1};

    #[test]
    fn image_emission_preserves_the_closed_plan_and_zero_patch() {
        let coordinate = ConeCoordinate::reserved_single_file();
        let (foundation, digests) = image_fixture(coordinate.clone(), Some(unit_exact_type()));
        let registrations =
            StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
        let plan = ConeImagePlanV1::new(
            coordinate,
            &[scoop_identity::ConeIdentity::CORE],
            &foundation,
            &registrations,
            &digests,
        )
        .unwrap();
        let context = Context::create();
        let llvm = context.create_module("image");
        let (bounds_message, array_size_message) = trap_messages(&context, &llvm, &plan);

        let emitted =
            emit_cone_image_v1(&context, &llvm, &plan, bounds_message, array_size_message).unwrap();

        assert_eq!(emitted.image().get_linkage(), Linkage::External);
        assert_eq!(emitted.image().get_visibility(), GlobalVisibility::Hidden);
        assert!(emitted.image().is_constant());
        assert_eq!(emitted.patch().intent(), plan.fingerprint_patch());
        assert_eq!(emitted.patch().definition(), plan.definition_plan());
        assert_eq!(emitted.primary_atom(), plan.primary_atom());
        assert_eq!(emitted.patch().atom(), plan.primary_atom());
        assert_eq!(
            emitted.support_atoms().dependencies().atom(),
            plan.support_atoms().dependencies()
        );
        assert_eq!(
            emitted.support_atoms().callables().atom(),
            plan.support_atoms().callables()
        );
        assert_eq!(
            emitted.support_atoms().array_bounds_message().atom(),
            plan.support_atoms().array_bounds_message()
        );
        assert_eq!(
            emitted.support_atoms().array_size_overflow_message().atom(),
            plan.support_atoms().array_size_overflow_message()
        );
        assert!(
            emitted
                .support_atoms()
                .dependencies()
                .global()
                .is_constant()
        );
        assert_eq!(emitted.patch().byte_offset(), 96);
        assert_eq!(emitted.patch().byte_size(), DIGEST_SIZE);
        let image_ir = emitted.image().print_to_string().to_string();
        assert!(image_ir.contains("hidden constant"), "{image_ir}");
        assert!(image_ir.contains("zeroinitializer"), "{image_ir}");
        assert!(
            image_ir.contains(&IMAGE_DESCRIPTOR_SIZE.to_string()),
            "{image_ir}"
        );
        assert!(
            llvm.get_global(&format!(
                "{}.metadata.dependencies",
                plan.symbol().symbol().as_str()
            ))
            .is_some()
        );
        let type_registration = PersistentSymbolRequest::new(
            PersistentSymbolKey::TypeRegistration(unit_exact_type()),
            LinkageClass::ConeStrong,
        )
        .unwrap()
        .symbol();
        assert!(llvm.get_global(type_registration.as_str()).is_some());
        llvm.verify().unwrap();
    }

    #[test]
    fn image_emission_rejects_a_duplicate_image_definition() {
        let coordinate = ConeCoordinate::reserved_core();
        let (foundation, digests) = image_fixture(coordinate.clone(), None);
        let registrations =
            StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
        let plan =
            ConeImagePlanV1::new(coordinate, &[], &foundation, &registrations, &digests).unwrap();
        let context = Context::create();
        let llvm = context.create_module("image");
        let (bounds_message, array_size_message) = trap_messages(&context, &llvm, &plan);

        emit_cone_image_v1(&context, &llvm, &plan, bounds_message, array_size_message).unwrap();
        let error = emit_cone_image_v1(&context, &llvm, &plan, bounds_message, array_size_message)
            .unwrap_err();

        assert!(error.0.contains("already declared"), "{error}");
    }

    #[test]
    fn image_emission_rejects_a_noncanonical_trap_message() {
        let coordinate = ConeCoordinate::reserved_single_file();
        let (foundation, digests) = image_fixture(coordinate.clone(), None);
        let registrations =
            StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
        let plan = ConeImagePlanV1::new(
            coordinate,
            &[scoop_identity::ConeIdentity::CORE],
            &foundation,
            &registrations,
            &digests,
        )
        .unwrap();
        let context = Context::create();
        let llvm = context.create_module("image");
        let (bounds_message, array_size_message) = trap_messages(&context, &llvm, &plan);
        bounds_message.set_initializer(&context.const_string(b"array index out of boundx", true));

        let error = emit_cone_image_v1(&context, &llvm, &plan, bounds_message, array_size_message)
            .unwrap_err();

        assert!(error.0.contains("canonical strong definition"), "{error}");
        assert!(llvm.get_global(plan.symbol().symbol().as_str()).is_none());
    }

    fn trap_messages<'ctx>(
        context: &'ctx Context,
        llvm: &inkwell::module::Module<'ctx>,
        plan: &ConeImagePlanV1,
    ) -> (
        inkwell::values::GlobalValue<'ctx>,
        inkwell::values::GlobalValue<'ctx>,
    ) {
        let define = |atom, bytes: &[u8]| {
            let request = PersistentSymbolRequest::new(
                PersistentSymbolKey::DefinitionBoundaryStart(atom),
                LinkageClass::ConeStrong,
            )
            .unwrap();
            let initializer = context.const_string(bytes, true);
            let global = llvm.add_global(initializer.get_type(), None, request.symbol().as_str());
            global.set_linkage(Linkage::External);
            global.set_constant(true);
            global.set_initializer(&initializer);
            global
        };
        (
            define(
                plan.support_atoms().array_bounds_message(),
                b"array index out of bounds",
            ),
            define(
                plan.support_atoms().array_size_overflow_message(),
                b"array size overflow",
            ),
        )
    }

    fn image_fixture(
        coordinate: ConeCoordinate,
        registration: Option<PersistentExactTypeId>,
    ) -> (OdrFreeLirFoundation, StrongDigestFinalizationPlanV1) {
        let producer = coordinate.identity().unwrap();
        let image_plan_key = ObjectDefinitionPlanKey::strong(
            producer,
            StrongDefinitionEntity::cone_image(producer),
            StrongDefinitionRole::ImageDescriptor,
        )
        .unwrap();
        let image_plan = CborIdentityRecord::from_key(image_plan_key).unwrap();
        let image_plan_id = image_plan.id();
        let image_atoms = image_atoms(image_plan.id());
        let image_symbol = PersistentSymbolRequest::new(
            PersistentSymbolKey::ImageDescriptor(producer),
            LinkageClass::ConeStrong,
        )
        .unwrap();

        let mut plans = vec![image_plan];
        let mut nodes = Vec::new();
        if let Some(exact) = registration {
            let registration_plan = CborIdentityRecord::from_key(
                ObjectDefinitionPlanKey::strong(
                    producer,
                    StrongDefinitionEntity::exact_type(exact),
                    StrongDefinitionRole::TypeRegistration,
                )
                .unwrap(),
            )
            .unwrap();
            let registration_node = DigestNodeV1::new(
                DigestNodeKey::strong_registration(registration_plan.id()),
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
            plans.push(registration_plan);
            nodes.push(registration_node);
        }

        let mut canonical = CanonicalLirFoundation::empty();
        canonical.set_definition_plans(plans).unwrap();
        canonical.set_definition_atoms(image_atoms).unwrap();
        canonical
            .set_symbol_requests(PersistentSymbolRequestTable::new(vec![image_symbol]).unwrap());
        let foundation = OdrFreeLirFoundation::try_new(producer, canonical).unwrap();

        let image_key = DigestNodeKey::runtime_image(producer);
        let image_node_id = DigestNodeId::from_key(&image_key).unwrap();
        let image_patch = DigestPatchIntentKey::new(
            image_node_id,
            image_plan_id,
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::RuntimeImage,
        );
        let inputs = nodes.iter().map(DigestInputRefV1::from_node).collect();
        nodes.push(DigestNodeV1::new(image_key, inputs, vec![image_patch]).unwrap());
        let digest_plan = StrongDigestFinalizationPlanV1::new(nodes, &foundation).unwrap();
        (foundation, digest_plan)
    }

    fn image_atoms(
        plan: scoop_identity::ObjectDefinitionPlanId,
    ) -> Vec<CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>>
    {
        let mut keys = vec![ObjectDefinitionAtomKey::new(
            plan,
            DefinitionAtomRole::Primary,
            DefinitionAtomSubkey::Singleton,
        )];
        keys.extend(
            [
                (
                    DefinitionAtomRole::AddressTakenConstant,
                    ConeImageSupportRole::CoordinateGroup,
                ),
                (
                    DefinitionAtomRole::AddressTakenConstant,
                    ConeImageSupportRole::CoordinateName,
                ),
                (
                    DefinitionAtomRole::AddressTakenConstant,
                    ConeImageSupportRole::CoordinateVersion,
                ),
                (
                    DefinitionAtomRole::RuntimeRecord,
                    ConeImageSupportRole::Dependencies,
                ),
                (
                    DefinitionAtomRole::RuntimeRecord,
                    ConeImageSupportRole::StaticStorages,
                ),
                (
                    DefinitionAtomRole::RuntimeRecord,
                    ConeImageSupportRole::ImmortalObjects,
                ),
                (
                    DefinitionAtomRole::RuntimeRecord,
                    ConeImageSupportRole::InitializationUnits,
                ),
                (
                    DefinitionAtomRole::RuntimeRecord,
                    ConeImageSupportRole::TypeRegistrations,
                ),
                (
                    DefinitionAtomRole::RuntimeRecord,
                    ConeImageSupportRole::Safepoints,
                ),
                (
                    DefinitionAtomRole::RuntimeRecord,
                    ConeImageSupportRole::Callables,
                ),
                (
                    DefinitionAtomRole::AddressTakenConstant,
                    ConeImageSupportRole::ArrayBoundsMessage,
                ),
                (
                    DefinitionAtomRole::AddressTakenConstant,
                    ConeImageSupportRole::ArraySizeOverflowMessage,
                ),
            ]
            .map(|(role, support)| {
                ObjectDefinitionAtomKey::new(
                    plan,
                    role,
                    DefinitionAtomSubkey::ConeImageSupport(support),
                )
            }),
        );
        keys.into_iter()
            .map(|key| CborIdentityRecord::from_key(key).unwrap())
            .collect()
    }

    fn unit_exact_type() -> PersistentExactTypeId {
        PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap()
    }
}
