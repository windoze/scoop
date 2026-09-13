use inkwell::GlobalVisibility;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::types::{AnyType, IntType, StructType};
use inkwell::values::{GlobalValue, PointerValue, StructValue};
use scoop_lir::{
    ConeImagePlanV1, DigestPatchIntentId, LinkageClass, ObjectDefinitionPlanId,
    PersistentSymbolRequest,
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
    owner: GlobalValue<'ctx>,
}

impl<'ctx> RuntimeImagePatchSiteV1<'ctx> {
    pub const fn intent(self) -> DigestPatchIntentId {
        self.intent
    }

    pub const fn definition(self) -> ObjectDefinitionPlanId {
        self.definition
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

/// Fully emitted per-Cone image and its single provisional digest slot.
#[derive(Clone, Copy, Debug)]
pub struct EmittedConeImageV1<'ctx> {
    image: GlobalValue<'ctx>,
    patch: RuntimeImagePatchSiteV1<'ctx>,
}

impl<'ctx> EmittedConeImageV1<'ctx> {
    pub const fn image(self) -> GlobalValue<'ctx> {
        self.image
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
pub fn emit_cone_image_v1<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    plan: &ConeImagePlanV1,
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

    let coordinate = plan.cone().coordinate();
    let prefix = format!("{}.metadata", image_symbol.as_str());
    let group = emit_byte_span(
        context,
        llvm,
        types.byte_span,
        &format!("{prefix}.coordinate.group"),
        coordinate.group().as_bytes(),
    );
    let name = emit_byte_span(
        context,
        llvm,
        types.byte_span,
        &format!("{prefix}.coordinate.name"),
        coordinate.name().as_bytes(),
    );
    let version = emit_byte_span(
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
        dependency_table.into(),
        i64.const_int(plan.dependencies().len() as u64, false)
            .into(),
        static_storages.into(),
        i64.const_int(tables.static_storages().len() as u64, false)
            .into(),
        immortal_objects.into(),
        i64.const_int(tables.immortal_objects().len() as u64, false)
            .into(),
        initialization_units.into(),
        i64.const_int(tables.initialization_units().len() as u64, false)
            .into(),
        type_registrations.into(),
        i64.const_int(tables.type_registrations().len() as u64, false)
            .into(),
        safepoints.into(),
        i64.const_int(tables.safepoints().len() as u64, false)
            .into(),
        callables.into(),
        i64.const_int(tables.callables().len() as u64, false).into(),
    ]);
    let image = llvm.add_global(types.image_descriptor, None, image_symbol.as_str());
    image.set_linkage(Linkage::External);
    image.set_visibility(GlobalVisibility::Hidden);
    image.set_constant(true);
    image.set_initializer(&image_value);

    Ok(EmittedConeImageV1 {
        image,
        patch: RuntimeImagePatchSiteV1 {
            intent: plan.fingerprint_patch(),
            definition: plan.definition_plan(),
            owner: image,
        },
    })
}

fn emit_byte_span<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    span_type: StructType<'ctx>,
    symbol: &str,
    bytes: &[u8],
) -> StructValue<'ctx> {
    let initializer = context.const_string(bytes, false);
    let global = llvm.add_global(initializer.get_type(), None, symbol);
    global.set_linkage(Linkage::Private);
    global.set_constant(true);
    global.set_initializer(&initializer);
    span_type.const_named_struct(&[
        global.as_pointer_value().into(),
        context
            .i64_type()
            .const_int(bytes.len() as u64, false)
            .into(),
    ])
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
) -> PointerValue<'ctx> {
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
) -> Result<PointerValue<'ctx>, CodegenError> {
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
) -> PointerValue<'ctx> {
    let global = llvm.add_global(initializer.get_type(), None, symbol);
    global.set_linkage(Linkage::Private);
    global.set_constant(true);
    global.set_initializer(&initializer);
    global.as_pointer_value()
}

#[cfg(test)]
mod tests {
    use inkwell::GlobalVisibility;
    use inkwell::context::Context;
    use inkwell::module::Linkage;
    use inkwell::values::AnyValue;
    use scoop_identity::{
        CborIdentityRecord, ConeCoordinate, CoreBuiltinNominal, DefinitionAtomRole,
        DefinitionAtomSubkey, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
        DigestSemanticFieldRole, ExactTypeKey, LinkageClass, ObjectDefinitionAtomKey,
        ObjectDefinitionPlanKey, PersistentExactTypeId, PersistentSymbolKey,
        PersistentSymbolRequest, PersistentSymbolRequestTable, StrongDefinitionEntity,
        StrongDefinitionRole,
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
        let plan = ConeImagePlanV1::new(coordinate, &foundation, &registrations, &digests).unwrap();
        let context = Context::create();
        let llvm = context.create_module("image");

        let emitted = emit_cone_image_v1(&context, &llvm, &plan).unwrap();

        assert_eq!(emitted.image().get_linkage(), Linkage::External);
        assert_eq!(emitted.image().get_visibility(), GlobalVisibility::Hidden);
        assert!(emitted.image().is_constant());
        assert_eq!(emitted.patch().intent(), plan.fingerprint_patch());
        assert_eq!(emitted.patch().definition(), plan.definition_plan());
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
        let plan = ConeImagePlanV1::new(coordinate, &foundation, &registrations, &digests).unwrap();
        let context = Context::create();
        let llvm = context.create_module("image");

        emit_cone_image_v1(&context, &llvm, &plan).unwrap();
        let error = emit_cone_image_v1(&context, &llvm, &plan).unwrap_err();

        assert!(error.0.contains("already declared"), "{error}");
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
        let image_owner = image_plan_key.owner();
        let image_plan = CborIdentityRecord::from_key(image_plan_key).unwrap();
        let image_atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
            image_plan.id(),
            DefinitionAtomRole::Primary,
            DefinitionAtomSubkey::Singleton,
        ))
        .unwrap();
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
        canonical.set_definition_atoms(vec![image_atom]).unwrap();
        canonical
            .set_symbol_requests(PersistentSymbolRequestTable::new(vec![image_symbol]).unwrap());
        let foundation = OdrFreeLirFoundation::try_new(producer, canonical).unwrap();

        let image_key = DigestNodeKey::runtime_image(producer);
        let image_node_id = DigestNodeId::from_key(&image_key).unwrap();
        let image_patch = DigestPatchIntentKey::new(
            image_node_id,
            image_owner,
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::RuntimeImage,
        );
        let inputs = nodes.iter().map(DigestInputRefV1::from_node).collect();
        nodes.push(DigestNodeV1::new(image_key, inputs, vec![image_patch]).unwrap());
        let digest_plan = StrongDigestFinalizationPlanV1::new(nodes, &foundation).unwrap();
        (foundation, digest_plan)
    }

    fn unit_exact_type() -> PersistentExactTypeId {
        PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap()
    }
}
