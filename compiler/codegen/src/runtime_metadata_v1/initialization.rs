use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::types::{AnyType, StructType};
use inkwell::values::{FunctionValue, GlobalValue, StructValue, UnnamedAddress};
use scoop_lir::{
    ConeIdentity, DigestPatchIntentId, ObjectDefinitionAtomId, ObjectDefinitionPlanId,
    PersistentInitializationUnitId, PersistentSymbolRequest, StrongInitializationCallableRefPlanV1,
    StrongInitializationRegistrationSchedulePlanV1, StrongInitializationStaticStorageRefPlanV1,
    StrongInitializationUnitRegistrationPlan, StrongInitializationUnitRegistrationPlanSet,
};

use super::RuntimeMetadataV1Types;
use crate::CodegenError;
use crate::target::ValidatedBackendProfile;

const METADATA_ABI_VERSION: u64 = 2;
const INITIALIZATION_UNIT_DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5049_4e49;
const INITIALIZATION_UNIT_DESCRIPTOR_SIZE: u64 = 352;
const REGISTRATION_DEFINITION_FINGERPRINT_OFFSET: u64 = 120;
const GATEWAY_DEFINITION_FINGERPRINT_OFFSET: u64 = 312;
const DIGEST_SIZE: u64 = 32;

#[derive(Clone, Copy, Debug)]
pub struct InitializationRegistrationPatchSiteV1<'ctx> {
    intent: DigestPatchIntentId,
    definition: ObjectDefinitionPlanId,
    atom: ObjectDefinitionAtomId,
    owner: GlobalValue<'ctx>,
    byte_offset: u64,
}

impl<'ctx> InitializationRegistrationPatchSiteV1<'ctx> {
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
        self.byte_offset
    }

    pub const fn byte_size(self) -> u64 {
        DIGEST_SIZE
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EmittedStrongInitializationUnitRegistrationV1<'ctx> {
    unit: PersistentInitializationUnitId,
    cell: GlobalValue<'ctx>,
    coordinator_descriptor: GlobalValue<'ctx>,
    diagnostic_atom: ObjectDefinitionAtomId,
    diagnostic: GlobalValue<'ctx>,
    registration_descriptor: GlobalValue<'ctx>,
    registration_definition_patch: InitializationRegistrationPatchSiteV1<'ctx>,
    gateway_definition_patch: Option<InitializationRegistrationPatchSiteV1<'ctx>>,
}

impl<'ctx> EmittedStrongInitializationUnitRegistrationV1<'ctx> {
    pub const fn unit(self) -> PersistentInitializationUnitId {
        self.unit
    }

    pub const fn cell(self) -> GlobalValue<'ctx> {
        self.cell
    }

    pub const fn coordinator_descriptor(self) -> GlobalValue<'ctx> {
        self.coordinator_descriptor
    }

    pub const fn diagnostic_atom(self) -> ObjectDefinitionAtomId {
        self.diagnostic_atom
    }

    pub const fn diagnostic(self) -> GlobalValue<'ctx> {
        self.diagnostic
    }

    pub const fn registration_descriptor(self) -> GlobalValue<'ctx> {
        self.registration_descriptor
    }

    pub const fn registration_definition_patch(
        self,
    ) -> InitializationRegistrationPatchSiteV1<'ctx> {
        self.registration_definition_patch
    }

    pub const fn gateway_definition_patch(
        self,
    ) -> Option<InitializationRegistrationPatchSiteV1<'ctx>> {
        self.gateway_definition_patch
    }
}

#[derive(Clone, Debug)]
pub struct EmittedStrongInitializationUnitRegistrationSetV1<'ctx> {
    producer: ConeIdentity,
    registrations: Vec<EmittedStrongInitializationUnitRegistrationV1<'ctx>>,
}

impl<'ctx> EmittedStrongInitializationUnitRegistrationSetV1<'ctx> {
    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn registrations(&self) -> &[EmittedStrongInitializationUnitRegistrationV1<'ctx>] {
        &self.registrations
    }
}

/// Emit separate `ic`, coordinator `id`, and runtime-registration `nr`
/// definitions for every initialization unit. Graph-managed digest slots stay
/// zero until the object-backed finalizer patches them.
pub(crate) fn emit_strong_initialization_unit_registrations_v1<'ctx, D: Clone>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    profile: ValidatedBackendProfile,
    plan: &StrongInitializationUnitRegistrationPlanSet<D>,
) -> Result<EmittedStrongInitializationUnitRegistrationSetV1<'ctx>, CodegenError> {
    let types = RuntimeMetadataV1Types::new(context);
    let coordinator_type = coordinator_descriptor_type(context);
    let prepared = plan
        .registrations()
        .iter()
        .map(|registration| {
            prepare_registration(context, llvm, &types, coordinator_type, registration)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let registrations = prepared
        .into_iter()
        .map(|prepared| {
            emit_registration(context, llvm, profile, &types, coordinator_type, prepared)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(EmittedStrongInitializationUnitRegistrationSetV1 {
        producer: plan.producer(),
        registrations,
    })
}

struct PreparedInitializationRegistrationV1<'ctx, D> {
    plan: StrongInitializationUnitRegistrationPlan<D>,
    storage_value: GlobalValue<'ctx>,
    failure_value: GlobalValue<'ctx>,
    storage_registration: Option<GlobalValue<'ctx>>,
    failure_registration: Option<GlobalValue<'ctx>>,
    initializer: FunctionValue<'ctx>,
    ensure: FunctionValue<'ctx>,
    gateway: Option<FunctionValue<'ctx>>,
    cell: Option<GlobalValue<'ctx>>,
    coordinator: Option<GlobalValue<'ctx>>,
    registration: Option<GlobalValue<'ctx>>,
    diagnostic_symbol: String,
}

fn prepare_registration<'ctx, D: Clone>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    types: &RuntimeMetadataV1Types<'ctx>,
    coordinator_type: StructType<'ctx>,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
) -> Result<PreparedInitializationRegistrationV1<'ctx, D>, CodegenError> {
    let storage_value = require_storage_value(llvm, plan.storage())?;
    let failure_value = require_storage_value(llvm, plan.failure_root())?;
    let storage_registration = prepare_global_declaration(
        llvm,
        plan.storage().registration_symbol(),
        types.static_storage_descriptor,
        "initialization value-storage registration",
        false,
    )?;
    let failure_registration = prepare_global_declaration(
        llvm,
        plan.failure_root().registration_symbol(),
        types.static_storage_descriptor,
        "initialization failure-root registration",
        false,
    )?;
    let initializer = require_callable_entry(
        context,
        llvm,
        plan.initializer(),
        CallableEntryRoleV1::ManagedUnit,
    )?;
    let ensure = require_callable_entry(
        context,
        llvm,
        plan.ensure(),
        CallableEntryRoleV1::ManagedUnit,
    )?;
    let gateway = match plan.schedule() {
        StrongInitializationRegistrationSchedulePlanV1::EagerStartup { gateway, .. } => {
            Some(require_callable_entry(
                context,
                llvm,
                **gateway,
                CallableEntryRoleV1::StartupGateway,
            )?)
        }
        StrongInitializationRegistrationSchedulePlanV1::LazyAccess => None,
    };

    let cell = prepare_global_declaration(
        llvm,
        plan.cell_symbol(),
        types.initialization_cell,
        "initialization cell",
        true,
    )?;
    let coordinator = prepare_global_declaration(
        llvm,
        plan.descriptor_symbol(),
        coordinator_type,
        "initialization coordinator descriptor",
        true,
    )?;
    let registration = prepare_global_declaration(
        llvm,
        plan.registration_symbol(),
        types.initialization_unit_descriptor,
        "initialization registration",
        true,
    )?;
    let diagnostic_symbol = format!("{}.diagnostic", plan.registration_symbol().symbol());
    if llvm.get_global(&diagnostic_symbol).is_some()
        || llvm.get_function(&diagnostic_symbol).is_some()
    {
        return Err(CodegenError(format!(
            "initialization diagnostic bytes `{diagnostic_symbol}` are already declared"
        )));
    }

    Ok(PreparedInitializationRegistrationV1 {
        plan: plan.clone(),
        storage_value,
        failure_value,
        storage_registration,
        failure_registration,
        initializer,
        ensure,
        gateway,
        cell,
        coordinator,
        registration,
        diagnostic_symbol,
    })
}

fn emit_registration<'ctx, D>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    profile: ValidatedBackendProfile,
    types: &RuntimeMetadataV1Types<'ctx>,
    coordinator_type: StructType<'ctx>,
    prepared: PreparedInitializationRegistrationV1<'ctx, D>,
) -> Result<EmittedStrongInitializationUnitRegistrationV1<'ctx>, CodegenError> {
    let plan = prepared.plan;
    let semantic = plan.semantic();
    let cell = define_global(
        llvm,
        plan.cell_symbol(),
        types.initialization_cell,
        prepared.cell,
        false,
        types.initialization_cell.const_zero().into(),
    );
    // The mutable coordinator cell is also a primary definition atom. Keep
    // its zero initializer file-backed so object verification and definition
    // fingerprinting observe canonical bytes rather than a virtual BSS range.
    cell.set_section(Some(profile.writable_storage_section()));
    let storage_registration = declare_global(
        llvm,
        plan.storage().registration_symbol(),
        types.static_storage_descriptor,
        prepared.storage_registration,
    );
    let failure_registration = declare_global(
        llvm,
        plan.failure_root().registration_symbol(),
        types.static_storage_descriptor,
        prepared.failure_registration,
    );
    let diagnostic_initializer = context.const_string(semantic.diagnostic_path().as_bytes(), true);
    let diagnostic = llvm.add_global(
        diagnostic_initializer.get_type(),
        None,
        &prepared.diagnostic_symbol,
    );
    diagnostic.set_linkage(Linkage::Private);
    diagnostic.set_constant(true);
    diagnostic.set_initializer(&diagnostic_initializer);
    diagnostic.set_section(Some(profile.c_string_section()));

    let coordinator_schedule = match plan.schedule() {
        StrongInitializationRegistrationSchedulePlanV1::EagerStartup { .. } => 0,
        StrongInitializationRegistrationSchedulePlanV1::LazyAccess => 1,
    };
    let coordinator_value = coordinator_type.const_named_struct(&[
        context
            .i64_type()
            .const_int(coordinator_schedule, false)
            .into(),
        digest_bytes(context, semantic.unit().as_array()).into(),
        diagnostic.as_pointer_value().into(),
        cell.as_pointer_value().into(),
        prepared.storage_value.as_pointer_value().into(),
        prepared.failure_value.as_pointer_value().into(),
        prepared
            .initializer
            .as_global_value()
            .as_pointer_value()
            .into(),
        prepared.ensure.as_global_value().as_pointer_value().into(),
    ]);
    let coordinator = define_global(
        llvm,
        plan.descriptor_symbol(),
        coordinator_type,
        prepared.coordinator,
        true,
        coordinator_value.into(),
    );

    let i32 = context.i32_type();
    let i64 = context.i64_type();
    let zero_digest = types.digest.const_zero();
    let prefix = types.descriptor_prefix.const_named_struct(&[
        i64.const_int(INITIALIZATION_UNIT_DESCRIPTOR_MAGIC, false)
            .into(),
        i32.const_int(METADATA_ABI_VERSION, false).into(),
        i32.const_int(INITIALIZATION_UNIT_DESCRIPTOR_SIZE, false)
            .into(),
    ]);
    let identity = super::registration_identity::registration_identity_value(
        context,
        types,
        semantic.unit().as_array(),
        plan.definition_owner(),
    );
    let diagnostic_span = types.byte_span.const_named_struct(&[
        diagnostic.as_pointer_value().into(),
        i64.const_int(semantic.diagnostic_path().len() as u64, false)
            .into(),
    ]);
    let (gateway_id, gateway_entry) = match (plan.schedule(), prepared.gateway) {
        (
            StrongInitializationRegistrationSchedulePlanV1::EagerStartup { gateway, .. },
            Some(entry),
        ) => (
            digest_value(context, types.digest, gateway.body().as_array()),
            entry.as_global_value().as_pointer_value(),
        ),
        (StrongInitializationRegistrationSchedulePlanV1::LazyAccess, None) => (
            zero_digest,
            context
                .ptr_type(inkwell::AddressSpace::default())
                .const_null(),
        ),
        _ => {
            return Err(CodegenError(
                "initialization schedule and prepared gateway disagree".to_string(),
            ));
        }
    };
    let registration_value = types.initialization_unit_descriptor.const_named_struct(&[
        prefix.into(),
        identity.into(),
        i32.const_int(u64::from(semantic.schedule().tag()), false)
            .into(),
        i32.const_zero().into(),
        diagnostic_span.into(),
        cell.as_pointer_value().into(),
        storage_registration.as_pointer_value().into(),
        failure_registration.as_pointer_value().into(),
        digest_value(context, types.digest, semantic.initializer().as_array()).into(),
        digest_value(context, types.digest, semantic.ensure().as_array()).into(),
        prepared
            .initializer
            .as_global_value()
            .as_pointer_value()
            .into(),
        prepared.ensure.as_global_value().as_pointer_value().into(),
        gateway_id.into(),
        zero_digest.into(),
        gateway_entry.into(),
    ]);
    let registration = define_global(
        llvm,
        plan.registration_symbol(),
        types.initialization_unit_descriptor,
        prepared.registration,
        true,
        registration_value.into(),
    );
    let registration_definition_patch = InitializationRegistrationPatchSiteV1 {
        intent: plan.registration_definition_patch(),
        definition: plan.registration_definition_plan(),
        atom: plan.registration_primary_atom(),
        owner: registration,
        byte_offset: REGISTRATION_DEFINITION_FINGERPRINT_OFFSET,
    };
    let gateway_definition_patch = plan.schedule().gateway_definition_patch().map(|intent| {
        InitializationRegistrationPatchSiteV1 {
            intent,
            definition: plan.registration_definition_plan(),
            atom: plan.registration_primary_atom(),
            owner: registration,
            byte_offset: GATEWAY_DEFINITION_FINGERPRINT_OFFSET,
        }
    });
    Ok(EmittedStrongInitializationUnitRegistrationV1 {
        unit: semantic.unit(),
        cell,
        coordinator_descriptor: coordinator,
        diagnostic_atom: plan.diagnostic_atom(),
        diagnostic,
        registration_descriptor: registration,
        registration_definition_patch,
        gateway_definition_patch,
    })
}

pub(crate) fn coordinator_descriptor_type(context: &Context) -> StructType<'_> {
    let ptr = context.ptr_type(inkwell::AddressSpace::default());
    context.struct_type(
        &[
            context.i64_type().into(),
            context.i8_type().array_type(32).into(),
            ptr.into(),
            ptr.into(),
            ptr.into(),
            ptr.into(),
            ptr.into(),
            ptr.into(),
        ],
        false,
    )
}

fn require_storage_value<'ctx>(
    llvm: &LlvmModule<'ctx>,
    plan: StrongInitializationStaticStorageRefPlanV1,
) -> Result<GlobalValue<'ctx>, CodegenError> {
    let request = plan.storage_symbol();
    let linkage = super::registration_identity::definition_linkage(request)?;
    let symbol = request.symbol();
    if llvm.get_function(symbol.as_str()).is_some() {
        return Err(CodegenError(format!(
            "initialization storage `{symbol}` collides with an LLVM function"
        )));
    }
    let global = llvm.get_global(symbol.as_str()).ok_or_else(|| {
        CodegenError(format!(
            "initialization storage `{symbol}` is not defined in the LLVM module"
        ))
    })?;
    if global.get_linkage() != linkage
        || global.get_unnamed_address() != UnnamedAddress::None
        || global.is_constant()
        || global.get_initializer().is_none()
    {
        return Err(CodegenError(format!(
            "initialization storage `{symbol}` is not a writable address-significant definition with its planned linkage"
        )));
    }
    Ok(global)
}

enum CallableEntryRoleV1 {
    ManagedUnit,
    StartupGateway,
}

fn require_callable_entry<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    plan: StrongInitializationCallableRefPlanV1,
    role: CallableEntryRoleV1,
) -> Result<FunctionValue<'ctx>, CodegenError> {
    let request = plan.entry_symbol();
    let linkage = super::registration_identity::definition_linkage(request)?;
    let symbol = request.symbol();
    if llvm.get_global(symbol.as_str()).is_some() {
        return Err(CodegenError(format!(
            "initialization callable `{symbol}` collides with an LLVM global"
        )));
    }
    let function = llvm.get_function(symbol.as_str()).ok_or_else(|| {
        CodegenError(format!(
            "initialization callable `{symbol}` is not declared in the LLVM module"
        ))
    })?;
    let expected = match role {
        CallableEntryRoleV1::ManagedUnit => context.void_type().fn_type(&[], false),
        CallableEntryRoleV1::StartupGateway => context.i32_type().fn_type(&[], false),
    };
    let expected_linkage = if function.get_first_basic_block().is_some() {
        linkage
    } else {
        Linkage::External
    };
    if function.get_linkage() != expected_linkage
        || function.as_global_value().get_unnamed_address() != UnnamedAddress::None
        || function.get_type() != expected
    {
        return Err(CodegenError(format!(
            "initialization callable `{symbol}` has an incompatible entry declaration with its planned linkage"
        )));
    }
    Ok(function)
}

fn prepare_global_declaration<'ctx>(
    llvm: &LlvmModule<'ctx>,
    request: PersistentSymbolRequest,
    expected_type: StructType<'ctx>,
    kind: &str,
    require_undefined: bool,
) -> Result<Option<GlobalValue<'ctx>>, CodegenError> {
    let definition_linkage = super::registration_identity::definition_linkage(request)?;
    let symbol = request.symbol();
    if llvm.get_function(symbol.as_str()).is_some() {
        return Err(CodegenError(format!(
            "{kind} `{symbol}` collides with an LLVM function"
        )));
    }
    let Some(global) = llvm.get_global(symbol.as_str()) else {
        return Ok(None);
    };
    let expected_linkage = if global.get_initializer().is_some() {
        definition_linkage
    } else {
        Linkage::External
    };
    if global.get_value_type() != expected_type.as_any_type_enum()
        || global.get_linkage() != expected_linkage
        || global.get_unnamed_address() != UnnamedAddress::None
    {
        return Err(CodegenError(format!(
            "{kind} `{symbol}` has an incompatible LLVM declaration"
        )));
    }
    if require_undefined && global.get_initializer().is_some() {
        return Err(CodegenError(format!(
            "{kind} `{symbol}` is already defined"
        )));
    }
    if !require_undefined && global.get_initializer().is_some() && !global.is_constant() {
        return Err(CodegenError(format!(
            "{kind} `{symbol}` has an incompatible mutable LLVM definition"
        )));
    }
    Ok(Some(global))
}

fn declare_global<'ctx>(
    llvm: &LlvmModule<'ctx>,
    request: PersistentSymbolRequest,
    ty: StructType<'ctx>,
    prior: Option<GlobalValue<'ctx>>,
) -> GlobalValue<'ctx> {
    prior.unwrap_or_else(|| {
        let global = llvm.add_global(ty, None, request.symbol().as_str());
        global.set_linkage(Linkage::External);
        global
    })
}

fn define_global<'ctx>(
    llvm: &LlvmModule<'ctx>,
    request: PersistentSymbolRequest,
    ty: StructType<'ctx>,
    prior: Option<GlobalValue<'ctx>>,
    constant: bool,
    initializer: inkwell::values::BasicValueEnum<'ctx>,
) -> GlobalValue<'ctx> {
    let global = declare_global(llvm, request, ty, prior);
    global.set_constant(constant);
    global.set_initializer(&initializer);
    global.set_linkage(
        super::registration_identity::definition_linkage(request)
            .expect("prepare_global_declaration validates the definition linkage"),
    );
    global
}

fn digest_bytes<'ctx>(
    context: &'ctx Context,
    bytes: &[u8; 32],
) -> inkwell::values::ArrayValue<'ctx> {
    let i8 = context.i8_type();
    let values = bytes
        .iter()
        .map(|byte| i8.const_int(u64::from(*byte), false))
        .collect::<Vec<_>>();
    i8.const_array(&values)
}

fn digest_value<'ctx>(
    context: &'ctx Context,
    digest_type: StructType<'ctx>,
    bytes: &[u8; 32],
) -> StructValue<'ctx> {
    digest_type.const_named_struct(&[digest_bytes(context, bytes).into()])
}

#[cfg(test)]
mod tests;
