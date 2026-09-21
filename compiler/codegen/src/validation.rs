use std::collections::{HashMap, HashSet};

use super::*;
use scoop_lir::{EnumDefId, GcEffect, StructDefId};

mod constants;
mod root_plans;
mod safepoints;
mod scoop_abi;
mod variants;
use constants::validate_constant_images;
use root_plans::validate_call_root_plans;
use safepoints::validate_safepoint_identities;
use scoop_abi::validate_scoop_abi;
use variants::validate_variant_primitives;

pub(crate) fn validate_module(module: &Module) -> Result<(), CodegenError> {
    validate_core_external_metadata(module)?;
    validate_dependency_external_metadata(module)?;
    validate_type_descriptor_symbols(module)?;
    validate_array_metadata(module)?;
    validate_output(module)?;
    validate_niche_representations(module)?;
    validate_scoop_abi(module)?;
    validate_constant_images(module)?;
    validate_variant_primitives(module)?;
    validate_machine_containers(module)?;
    validate_dispatch_table_identities(module)?;
    validate_dispatch_callable_tables(module)?;
    validate_dispatch_signatures(module)?;
    validate_c_abi(module)?;
    validate_foreign_callbacks(module)?;
    validate_safepoint_identities(module)?;
    validate_call_root_plans(module)?;
    scoop_lir::CanonicalLirFoundation::from_module(module)
        .map_err(|error| CodegenError(format!("invalid LIR identity foundation: {error}")))?;
    Ok(())
}

fn validate_core_external_metadata(module: &Module) -> Result<(), CodegenError> {
    if module.cone == scoop_lir::ConeIdentity::CORE
        && (!module.meta.core_external_callables.is_empty()
            || !module.meta.core_external_type_descriptors.is_empty())
    {
        return Err(CodegenError(
            "the core bootstrap Cone cannot import its own external definitions".to_string(),
        ));
    }

    let mut callable_targets = HashSet::new();
    let mut callable_bodies = HashSet::new();
    for (_, callable) in module.meta.core_external_callables.iter() {
        if !callable_targets.insert(callable.target()) {
            return Err(CodegenError(format!(
                "duplicate core external callable target {:?}",
                callable.target()
            )));
        }
        if !callable_bodies.insert(callable.body()) {
            return Err(CodegenError(format!(
                "duplicate core external callable body {}",
                callable.body()
            )));
        }
        if module
            .functions
            .iter()
            .any(|local| local.callable_body.id() == callable.body())
        {
            return Err(CodegenError(format!(
                "core external callable body {} is also defined locally",
                callable.body()
            )));
        }
    }

    let mut descriptor_targets = HashSet::new();
    for (_, descriptor) in module.meta.core_external_type_descriptors.iter() {
        if !descriptor_targets.insert(descriptor.target()) {
            return Err(CodegenError(format!(
                "duplicate core external TypeDescriptor target {}",
                descriptor.target()
            )));
        }
        if module
            .meta
            .type_descriptors
            .iter()
            .any(|(_, local)| local.identity.exact_type() == descriptor.target())
        {
            return Err(CodegenError(format!(
                "core external TypeDescriptor target {} is also defined locally",
                descriptor.target()
            )));
        }
    }
    Ok(())
}

fn validate_dependency_external_metadata(module: &Module) -> Result<(), CodegenError> {
    let mut descriptor_targets = HashSet::new();
    for (_, descriptor) in module.meta.dependency_external_type_descriptors.iter() {
        if descriptor.provider() == module.cone {
            return Err(CodegenError(format!(
                "dependency external TypeDescriptor {} names the current Cone as provider",
                descriptor.target()
            )));
        }
        if !descriptor_targets.insert(descriptor.target()) {
            return Err(CodegenError(format!(
                "duplicate dependency external TypeDescriptor target {}",
                descriptor.target()
            )));
        }
        if module
            .meta
            .type_descriptors
            .iter()
            .any(|(_, local)| local.identity.exact_type() == descriptor.target())
            || module
                .meta
                .core_external_type_descriptors
                .iter()
                .any(|(_, core)| core.target() == descriptor.target())
        {
            return Err(CodegenError(format!(
                "dependency external TypeDescriptor target {} crosses another descriptor partition",
                descriptor.target()
            )));
        }
    }

    let mut declarations = HashSet::new();
    let mut bodies = HashSet::new();
    for (_, callable) in module.meta.dependency_external_callables.iter() {
        if callable.provider() == module.cone {
            return Err(CodegenError(format!(
                "dependency external callable {:?} names the current Cone as provider",
                callable.target()
            )));
        }
        if let Some(declaration) = callable.legacy_declaration()
            && !declarations.insert((callable.provider(), declaration))
        {
            return Err(CodegenError(format!(
                "duplicate dependency external callable {}:{declaration:?}",
                callable.provider()
            )));
        }
        if !bodies.insert(callable.body()) {
            return Err(CodegenError(format!(
                "duplicate dependency external callable body {}",
                callable.body()
            )));
        }
        if module
            .functions
            .iter()
            .any(|local| local.callable_body.id() == callable.body())
        {
            return Err(CodegenError(format!(
                "dependency external callable body {} is also defined locally",
                callable.body()
            )));
        }
        if module
            .meta
            .core_external_callables
            .iter()
            .any(|(_, core)| core.body() == callable.body())
        {
            return Err(CodegenError(format!(
                "dependency external callable body {} also uses trusted-core authority",
                callable.body()
            )));
        }
    }
    Ok(())
}

fn validate_type_descriptor_symbols(module: &Module) -> Result<(), CodegenError> {
    match module.meta.well_known_type_descriptors.string {
        TypeDescriptorRef::Local(string) => {
            if arena_index(string) >= module.meta.type_descriptors.len() {
                return Err(CodegenError(format!(
                    "the runtime String TypeDescriptor local reference {} is out of bounds",
                    arena_index(string)
                )));
            }
        }
        TypeDescriptorRef::CoreExternal(string) => {
            if arena_index(string) >= module.meta.core_external_type_descriptors.len() {
                return Err(CodegenError(format!(
                    "the runtime String TypeDescriptor core-external reference {} is out of bounds",
                    arena_index(string)
                )));
            }
        }
        TypeDescriptorRef::DependencyExternal(_) => {
            return Err(CodegenError(
                "the runtime String TypeDescriptor cannot use dependency layout authority"
                    .to_string(),
            ));
        }
    }

    for (_, descriptor) in module.meta.type_descriptors.iter() {
        if descriptor.diagnostic_name.is_empty() {
            return Err(CodegenError(format!(
                "type descriptor {} has an empty canonical diagnostic name",
                descriptor.identity.exact_type()
            )));
        }
        if !descriptor
            .instance_layout
            .is_managed_instance_of(descriptor.identity.exact_type(), module.meta.target_profile)
        {
            return Err(CodegenError(format!(
                "type descriptor `{}` carries an instance layout for another exact type, target, representation, or scan role",
                descriptor.diagnostic_name
            )));
        }
    }
    Ok(())
}

fn validate_array_metadata(module: &Module) -> Result<(), CodegenError> {
    for (_, array) in module.meta.arrays.iter() {
        if contains_machine_scalar(&module.structs, &module.enums, &array.element) {
            return Err(CodegenError(format!(
                "array element type {} contains an internal machine scalar",
                array.element.dump()
            )));
        }
        let TypeDescriptorRef::Local(descriptor_id) = array.type_descriptor else {
            return Err(CodegenError(
                "array metadata requires a complete local TypeDescriptor".to_string(),
            ));
        };
        let descriptor_index = arena_index(descriptor_id);
        if descriptor_index >= module.meta.type_descriptors.len() {
            return Err(CodegenError(format!(
                "array metadata has invalid local TypeDescriptor id {descriptor_index}"
            )));
        }
        let descriptor = &module.meta.type_descriptors[descriptor_id];
        let exact = descriptor.identity.exact_type();
        if !array
            .identity
            .is_managed_array_of(exact, module.meta.target_profile)
        {
            return Err(CodegenError(format!(
                "array TypeDescriptor `{}` and its element layout identify different exact types, targets, representations, or scan roles",
                descriptor.diagnostic_name
            )));
        }
        let expected = array.layout.instance();
        if &descriptor.instance_shape != expected {
            return Err(CodegenError(format!(
                "array TypeDescriptor `{}` does not match its closed element size, alignment, and scan shape",
                descriptor.diagnostic_name
            )));
        }
    }
    for function in &module.functions {
        for (_, block) in function.blocks.iter() {
            for instruction in &block.instructions {
                let Instruction::ArrayClone {
                    source_type,
                    array_type,
                    ..
                } = instruction
                else {
                    continue;
                };
                if arena_index(*source_type) >= module.meta.arrays.len()
                    || arena_index(*array_type) >= module.meta.arrays.len()
                {
                    return Err(CodegenError(
                        "array clone source or target metadata is missing".into(),
                    ));
                }
                let source = &module.meta.arrays[*source_type];
                let target = &module.meta.arrays[*array_type];
                if source.element_exact != target.element_exact
                    || source.element != target.element
                    || source.layout != target.layout
                {
                    return Err(CodegenError(
                        "array clone exact element type or storage disagrees".into(),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn executable_entry(
    module: &Module,
    entry: scoop_lir::LocalFunctionRef,
) -> Result<&Function, CodegenError> {
    let (declaration, expected_effect) = match entry {
        scoop_lir::LocalFunctionRef::Managed(reference) => {
            (reference.declaration(), GcEffect::Managed)
        }
        scoop_lir::LocalFunctionRef::NoGc(reference) => (reference.declaration(), GcEffect::NoGc),
    };
    let index = declaration.into_u32() as usize;
    let function = module.functions.get(index).ok_or_else(|| {
        CodegenError(format!(
            "module has invalid executable entry function id {index}"
        ))
    })?;
    if function.gc_effect != expected_effect {
        return Err(CodegenError(format!(
            "executable entry @{} has {:?} body but {:?} typed reference",
            function.symbol(),
            function.gc_effect,
            expected_effect
        )));
    }
    Ok(function)
}

fn validate_output(module: &Module) -> Result<(), CodegenError> {
    let scoop_lir::LirOutput::Executable { entry } = module.output else {
        return Ok(());
    };
    executable_entry(module, entry).map(|_| ())
}

pub(crate) fn validate_executable_entry(
    module: &Module,
    entry: scoop_lir::LocalFunctionRef,
) -> Result<(), CodegenError> {
    let function = executable_entry(module, entry)?;
    if !function.signature.arguments().is_empty()
        || !matches!(function.signature.result(), scoop_lir::AbiReturn::UnitVoid)
    {
        return Err(CodegenError(format!(
            "executable entry @{} must have signature () -> Unit",
            function.symbol()
        )));
    }
    Ok(())
}

fn validate_dispatch_table_identities(module: &Module) -> Result<(), CodegenError> {
    for (_, descriptor) in module.meta.type_descriptors.iter() {
        let owner = descriptor.identity.exact_type();
        if !descriptor.vtable.belongs_to_exact_type(owner) {
            return Err(CodegenError(format!(
                "type descriptor `{}` carries a vtable identity for another exact type or table role",
                descriptor.diagnostic_name
            )));
        }

        for itable in &descriptor.itables {
            if !itable.belongs_to_exact_type(owner) {
                return Err(CodegenError(format!(
                    "type descriptor `{}` carries an itable identity for another exact type or table role",
                    descriptor.diagnostic_name
                )));
            }
            let TypeDescriptorRef::Local(interface_id) = itable.interface() else {
                continue;
            };
            let interface_index = arena_index(interface_id);
            if interface_index >= module.meta.type_descriptors.len() {
                return Err(CodegenError(format!(
                    "type descriptor `{}` has invalid local itable interface id {interface_index}",
                    descriptor.diagnostic_name
                )));
            }
            let interface = &module.meta.type_descriptors[interface_id];
            if !itable.belongs_to_interface_exact_type(interface.identity.exact_type()) {
                return Err(CodegenError(format!(
                    "type descriptor `{}` itable identity and interface descriptor identify different exact types",
                    descriptor.diagnostic_name
                )));
            }
        }
    }
    Ok(())
}

fn validate_variant_ref(
    module: &Module,
    variant: scoop_lir::LirVariantRef,
    owner: &str,
) -> Result<(), CodegenError> {
    if !module.enums.contains_variant(variant) {
        return Err(CodegenError(format!(
            "{owner} carries invalid enum{} variant {} reference",
            variant.definition().into_raw(),
            variant.index()
        )));
    }
    Ok(())
}

fn checked_temp_type<'a>(
    function: &'a Function,
    temp: scoop_lir::TempId,
    owner: &str,
) -> Result<&'a LirType, CodegenError> {
    let index = arena_index(temp);
    if index >= function.temps.len() {
        return Err(CodegenError(format!(
            "{owner} references invalid temporary t{index} in @{}",
            function.symbol()
        )));
    }
    Ok(&function.temps[temp].ty)
}

fn checked_value_type(
    module: &Module,
    function: &Function,
    value: Value,
    owner: &str,
) -> Result<LirType, CodegenError> {
    let invalid = |kind: &str, index: usize| {
        CodegenError(format!(
            "{owner} references invalid {kind} {index} in @{}",
            function.symbol()
        ))
    };
    match value {
        Value::Local(id) => {
            let index = arena_index(id);
            if index >= function.locals.len() {
                return Err(invalid("local", index));
            }
            Ok(function.locals[id].ty().clone())
        }
        Value::Param(index) => function
            .signature
            .arguments()
            .get(index as usize)
            .map(|argument| argument.logical_storage_type().clone())
            .ok_or_else(|| invalid("parameter", index as usize)),
        Value::Temp(id) => checked_temp_type(function, id, owner).cloned(),
        Value::IntegerConst(value) => Ok(value.scalar_type()),
        Value::MachineScalar(value) => Ok(LirType::MachineScalar(value.kind())),
        Value::BoolConst(_) => Ok(LirType::I1),
        Value::NullPointer(kind) => Ok(LirType::Ptr(kind)),
        Value::TypeDescriptor(reference) => {
            let (kind, index, len) = match reference {
                scoop_lir::TypeDescriptorRef::Local(id) => (
                    "local type descriptor",
                    arena_index(id),
                    module.meta.type_descriptors.len(),
                ),
                scoop_lir::TypeDescriptorRef::CoreExternal(id) => (
                    "external type descriptor",
                    arena_index(id),
                    module.meta.core_external_type_descriptors.len(),
                ),
                scoop_lir::TypeDescriptorRef::DependencyExternal(id) => (
                    "dependency type descriptor",
                    arena_index(id),
                    module.meta.dependency_external_type_descriptors.len(),
                ),
            };
            if index >= len {
                return Err(invalid(kind, index));
            }
            Ok(scoop_lir::METADATA_PTR)
        }
        Value::RootScan(id) => {
            let index = arena_index(id);
            if index >= function.call_targets.root_scans.len() {
                return Err(invalid("root scan", index));
            }
            Ok(scoop_lir::METADATA_PTR)
        }
        Value::Global(id) => {
            let index = arena_index(id);
            if index >= module.globals.len() {
                return Err(invalid("global", index));
            }
            Ok(LirType::Ptr(module.globals[id].address_kind))
        }
        Value::InitializationUnit(id) => {
            let index = arena_index(id);
            if index >= module.initialization_units.len() {
                return Err(invalid("initialization unit", index));
            }
            Ok(scoop_lir::METADATA_PTR)
        }
        Value::CArgumentStorage(storage) => {
            let index = arena_index(storage.local());
            if index >= function.locals.len() {
                return Err(invalid("C argument local", index));
            }
            Ok(scoop_lir::RAW_PTR)
        }
    }
}

fn validate_niche_representations(module: &Module) -> Result<(), CodegenError> {
    for (_, definition) in module.enums.iter() {
        let EnumRepr::Niche {
            kind,
            payload_variant,
        } = &definition.repr
        else {
            continue;
        };
        if *payload_variant > 1 {
            return Err(CodegenError(format!(
                "niche enum `{}` has invalid payload variant {payload_variant}",
                definition.name
            )));
        }
        let valid_scan = match kind {
            scoop_lir::NichePointerKind::Managed => definition.scan == RefScan::References(vec![0]),
            scoop_lir::NichePointerKind::Raw | scoop_lir::NichePointerKind::Code => {
                definition.scan == RefScan::None
            }
        };
        if !valid_scan {
            return Err(CodegenError(format!(
                "niche enum `{}` has {} pointer provenance but incompatible scan {}",
                definition.name,
                kind.pointer_kind().dump(),
                definition.scan.dump(),
            )));
        }
    }
    Ok(())
}

fn validate_dispatch_callable_tables(module: &Module) -> Result<(), CodegenError> {
    let validate_entry = |descriptor: &TypeDescriptor,
                          entry: &DispatchEntry|
     -> Result<(), CodegenError> {
        let id = match entry.callable {
            CallableRef::Local(id) => id,
            CallableRef::CoreExternal(id) => {
                if arena_index(id) >= module.meta.core_external_callables.len() {
                    return Err(CodegenError(format!(
                        "type descriptor `{}` has invalid core dispatch callable id {}",
                        descriptor.diagnostic_name,
                        arena_index(id)
                    )));
                }
                return Ok(());
            }
            CallableRef::DependencyExternal(id) => {
                if arena_index(id) >= module.meta.dependency_external_callables.len() {
                    return Err(CodegenError(format!(
                        "type descriptor `{}` has invalid dependency dispatch callable id {}",
                        descriptor.diagnostic_name,
                        arena_index(id)
                    )));
                }
                return Ok(());
            }
            CallableRef::Runtime(runtime) => {
                if runtime.requires_dedicated_operation() {
                    return Err(CodegenError(format!(
                        "type descriptor `{}` dispatches to a dedicated boxing operation",
                        descriptor.diagnostic_name
                    )));
                }
                if matches!(
                    runtime,
                    scoop_lir::RuntimeFunction::Managed(
                        scoop_lir::ManagedRuntimeFunction::Alloc
                            | scoop_lir::ManagedRuntimeFunction::InitializationEnter
                    )
                ) {
                    return Err(CodegenError(format!(
                        "type descriptor `{}` dispatches to a runtime function whose closed ABI contains an internal machine scalar",
                        descriptor.diagnostic_name
                    )));
                }
                return Ok(());
            }
        };
        let function = module
            .functions
            .get(id.into_u32() as usize)
            .ok_or_else(|| {
                CodegenError(format!(
                    "type descriptor `{}` has invalid local dispatch callable id {}",
                    descriptor.diagnostic_name,
                    id.into_u32()
                ))
            })?;
        if function.signature.arguments().iter().any(|argument| {
            contains_machine_scalar(
                &module.structs,
                &module.enums,
                argument.logical_storage_type(),
            )
        }) || function
            .signature
            .result()
            .logical_storage_type()
            .is_some_and(|ty| contains_machine_scalar(&module.structs, &module.enums, ty))
        {
            return Err(CodegenError(format!(
                "type descriptor `{}` dispatches to local function @{} whose signature exposes an internal machine scalar",
                descriptor.diagnostic_name,
                function.symbol()
            )));
        }
        Ok(())
    };

    for (_, descriptor) in module.meta.type_descriptors.iter() {
        for entry in descriptor.vtable.slots() {
            validate_entry(descriptor, entry)?;
        }
        for record in &descriptor.itables {
            for entry in record.slots() {
                validate_entry(descriptor, entry)?;
            }
        }
    }
    Ok(())
}

fn validate_machine_containers(module: &Module) -> Result<(), CodegenError> {
    for (_, definition) in module.structs.iter() {
        for index in 0..definition.field_count() {
            let field_type = definition
                .field_storage_type(index)
                .expect("index is below the field count");
            if contains_machine_scalar(&module.structs, &module.enums, &field_type) {
                return Err(CodegenError(format!(
                    "struct `{}` field {} embeds an internal machine scalar in a source value container",
                    definition.name, index
                )));
            }
        }
    }
    let validate_value_type =
        |function: &Function, owner: &str, ty: &LirType| -> Result<(), CodegenError> {
            if !matches!(ty, LirType::MachineScalar(_))
                && contains_machine_scalar(&module.structs, &module.enums, ty)
            {
                return Err(CodegenError(format!(
                    "function @{} {owner} embeds an internal machine scalar in {}",
                    function.symbol(),
                    ty.dump()
                )));
            }
            Ok(())
        };
    for function in &module.functions {
        for (index, argument) in function.signature.arguments().iter().enumerate() {
            validate_value_type(
                function,
                &format!("parameter {index}"),
                argument.logical_storage_type(),
            )?;
        }
        if let Some(result) = function.signature.result().logical_storage_type() {
            validate_value_type(function, "result", result)?;
        }
        for (id, local) in function.locals.iter() {
            validate_value_type(
                function,
                &format!("local {}", id.into_raw().into_u32()),
                local.ty(),
            )?;
        }
        for (id, temp) in function.temps.iter() {
            validate_value_type(
                function,
                &format!("temporary {}", id.into_raw().into_u32()),
                &temp.ty,
            )?;
        }
    }
    for (_, global) in module.globals.iter() {
        let GlobalInit::Storage { ty, .. } = &global.init else {
            continue;
        };
        if contains_machine_scalar(&module.structs, &module.enums, ty) {
            return Err(CodegenError(format!(
                "storage global `{}` has internal machine-scalar type {}",
                global.symbol(),
                ty.dump()
            )));
        }
    }
    for (_, definition) in module.enums.iter() {
        let EnumRepr::Tagged { variants, .. } = &definition.repr else {
            continue;
        };
        for variant in variants {
            for field in &variant.fields {
                if contains_machine_scalar(&module.structs, &module.enums, &field.ty) {
                    return Err(CodegenError(format!(
                        "enum `{}` payload type {} contains an internal machine scalar",
                        definition.name,
                        field.ty.dump()
                    )));
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn contains_machine_scalar(
    structs: &StructDefs,
    enums: &EnumDefs,
    ty: &LirType,
) -> bool {
    fn visit(
        structs: &StructDefs,
        enums: &EnumDefs,
        ty: &LirType,
        seen_structs: &mut HashSet<StructDefId>,
        seen_enums: &mut HashSet<EnumDefId>,
    ) -> bool {
        match ty {
            LirType::MachineScalar(_) => true,
            LirType::Aggregate(elements) => elements
                .iter()
                .any(|element| visit(structs, enums, element, seen_structs, seen_enums)),
            LirType::Struct(id) if seen_structs.insert(*id) => {
                let definition = &structs[*id];
                (0..definition.field_count()).any(|index| {
                    let field_type = definition
                        .field_storage_type(index)
                        .expect("index is below the field count");
                    visit(structs, enums, &field_type, seen_structs, seen_enums)
                })
            }
            LirType::Enum(id) if seen_enums.insert(*id) => match &enums[*id].repr {
                EnumRepr::Niche { .. } => false,
                EnumRepr::Tagged { variants, .. } => variants.iter().any(|variant| {
                    variant
                        .fields
                        .iter()
                        .any(|field| visit(structs, enums, &field.ty, seen_structs, seen_enums))
                }),
            },
            _ => false,
        }
    }

    visit(structs, enums, ty, &mut HashSet::new(), &mut HashSet::new())
}

fn validate_c_abi(module: &Module) -> Result<(), CodegenError> {
    for (_, function) in module.extern_functions.iter() {
        match &function.kind {
            ExternFunctionKind::C { signature, .. } => {
                for (index, parameter) in signature.params.iter().enumerate() {
                    validate_c_type(module, parameter, false, &mut HashSet::new()).map_err(
                        |error| {
                            CodegenError(format!(
                                "C extern `{}` parameter {}: {}",
                                function.source_name, index, error.0
                            ))
                        },
                    )?;
                }
                validate_c_return_type(module, &signature.return_type).map_err(|error| {
                    CodegenError(format!(
                        "C extern `{}` result: {}",
                        function.source_name, error.0
                    ))
                })?;
            }
            ExternFunctionKind::Scoop { signature, .. } => {
                if signature.arguments().iter().any(|argument| {
                    contains_machine_scalar(
                        &module.structs,
                        &module.enums,
                        argument.logical_storage_type(),
                    )
                }) || signature
                    .result()
                    .logical_storage_type()
                    .is_some_and(|ty| contains_machine_scalar(&module.structs, &module.enums, ty))
                {
                    return Err(CodegenError(format!(
                        "Scoop extern `{}` exposes an internal machine scalar across an artifact boundary",
                        function.source_name
                    )));
                }
            }
        }
    }

    for (_, global) in module.native_globals.iter() {
        validate_c_type(module, &global.c_type, false, &mut HashSet::new()).map_err(|error| {
            CodegenError(format!(
                "native global `{}`: {}",
                global.source_name, error.0
            ))
        })?;
    }
    for (_, callback) in module.callback_bridges.iter() {
        let bridge_index = callback.bridge.declaration().into_u32() as usize;
        let Some(bridge) = module.functions.get(bridge_index) else {
            return Err(CodegenError(format!(
                "static callback `{}` has invalid NoGC bridge id {bridge_index}",
                callback.source_name
            )));
        };
        if bridge.gc_effect != GcEffect::NoGc {
            return Err(CodegenError(format!(
                "static callback `{}` bridge @{} must be NoGC",
                callback.source_name,
                bridge.symbol()
            )));
        }
        for parameter in &callback.params {
            validate_c_type(module, parameter, false, &mut HashSet::new())?;
        }
        validate_c_return_type(module, &callback.return_type)?;
    }
    for (_, callback) in module.foreign_callback_bridges.iter() {
        for parameter in &callback.params {
            validate_c_type(module, parameter, false, &mut HashSet::new())?;
        }
        validate_c_return_type(module, &callback.return_type)?;
    }
    validate_c_layouts(module)?;
    Ok(())
}

pub(crate) fn validate_c_layouts(module: &Module) -> Result<(), CodegenError> {
    for (id, definition) in module.structs.iter() {
        if definition.is_c_layout() {
            validate_c_struct(module, id, &mut HashSet::new())?;
        }
    }
    Ok(())
}

fn validate_dispatch_signatures(module: &Module) -> Result<(), CodegenError> {
    fn check(
        module: &Module,
        function: &Function,
        params: &[scoop_lir::AbiArgument],
        result: Option<&LirType>,
    ) -> Result<(), CodegenError> {
        let has_machine = params.iter().any(|argument| {
            contains_machine_scalar(
                &module.structs,
                &module.enums,
                argument.logical_storage_type(),
            )
        }) || result
            .is_some_and(|ty| contains_machine_scalar(&module.structs, &module.enums, ty));
        if has_machine {
            return Err(CodegenError(format!(
                "dispatch signature in @{} exposes an internal machine scalar without an authoritative dynamic-slot declaration",
                function.symbol()
            )));
        }
        Ok(())
    }

    for function in &module.functions {
        let targets = &function.call_targets;
        for (_, target) in targets.managed_targets.void.iter() {
            if matches!(
                target.destination,
                scoop_lir::ManagedCallDestination::Dispatch { .. }
            ) {
                check(
                    module,
                    function,
                    targets.void_signatures[target.signature].arguments(),
                    None,
                )?;
            }
        }
        for (_, target) in targets.managed_targets.elided_zst.iter() {
            if matches!(
                target.destination,
                scoop_lir::ManagedCallDestination::Dispatch { .. }
            ) {
                let signature = &targets.elided_zst_signatures[target.signature];
                check(
                    module,
                    function,
                    signature.arguments(),
                    Some(signature.result().storage_type()),
                )?;
            }
        }
        for (_, target) in targets.managed_targets.direct.iter() {
            if matches!(
                target.destination,
                scoop_lir::ManagedCallDestination::Dispatch { .. }
            ) {
                let signature = &targets.direct_signatures[target.signature];
                check(
                    module,
                    function,
                    signature.arguments(),
                    Some(signature.result().storage_type()),
                )?;
            }
        }
        for (_, target) in targets.managed_targets.indirect_result.iter() {
            if matches!(
                target.destination,
                scoop_lir::ManagedCallDestination::Dispatch { .. }
            ) {
                let signature = &targets.indirect_result_signatures[target.signature];
                check(
                    module,
                    function,
                    signature.arguments(),
                    Some(signature.result().storage_type()),
                )?;
            }
        }
        for (_, target) in targets.no_gc_targets.void.iter() {
            if matches!(
                target.destination,
                scoop_lir::NoGcCallDestination::Dispatch { .. }
            ) {
                check(
                    module,
                    function,
                    targets.void_signatures[target.signature].arguments(),
                    None,
                )?;
            }
        }
        for (_, target) in targets.no_gc_targets.elided_zst.iter() {
            if matches!(
                target.destination,
                scoop_lir::NoGcCallDestination::Dispatch { .. }
            ) {
                let signature = &targets.elided_zst_signatures[target.signature];
                check(
                    module,
                    function,
                    signature.arguments(),
                    Some(signature.result().storage_type()),
                )?;
            }
        }
        for (_, target) in targets.no_gc_targets.direct.iter() {
            if matches!(
                target.destination,
                scoop_lir::NoGcCallDestination::Dispatch { .. }
            ) {
                let signature = &targets.direct_signatures[target.signature];
                check(
                    module,
                    function,
                    signature.arguments(),
                    Some(signature.result().storage_type()),
                )?;
            }
        }
        for (_, target) in targets.no_gc_targets.indirect_result.iter() {
            if matches!(
                target.destination,
                scoop_lir::NoGcCallDestination::Dispatch { .. }
            ) {
                let signature = &targets.indirect_result_signatures[target.signature];
                check(
                    module,
                    function,
                    signature.arguments(),
                    Some(signature.result().storage_type()),
                )?;
            }
        }
    }
    Ok(())
}

fn validate_c_return_type(
    module: &Module,
    ty: &scoop_lir::CReturnType,
) -> Result<(), CodegenError> {
    match ty {
        scoop_lir::CReturnType::Void => Ok(()),
        scoop_lir::CReturnType::Value(ty) => {
            validate_c_type(module, ty, false, &mut HashSet::new())
        }
    }
}

fn niche_pointer_kind(module: &Module, lir: &LirType) -> Option<scoop_lir::NichePointerKind> {
    let LirType::Enum(id) = lir else {
        return None;
    };
    match &module.enums[*id].repr {
        EnumRepr::Niche { kind, .. } => Some(*kind),
        EnumRepr::Tagged { .. } => None,
    }
}

fn validate_c_struct(
    module: &Module,
    id: StructDefId,
    visiting: &mut HashSet<StructDefId>,
) -> Result<(), CodegenError> {
    let definition = &module.structs[id];
    let Some(fields) = definition.c_fields() else {
        return Err(CodegenError(format!(
            "struct `{}` is not a C-layout struct",
            definition.name
        )));
    };
    if !visiting.insert(id) {
        return Err(CodegenError(format!(
            "C-layout struct `{}` is recursively embedded by value",
            definition.name
        )));
    }
    for (index, field) in fields.iter().enumerate() {
        validate_c_type(module, &field.ty, true, visiting).map_err(|error| {
            CodegenError(format!(
                "C-layout struct `{}` field {}: {}",
                definition.name, index, error.0
            ))
        })?;
    }
    visiting.remove(&id);
    Ok(())
}

fn validate_c_type(
    module: &Module,
    ty: &scoop_lir::CType,
    struct_by_value: bool,
    visiting: &mut HashSet<StructDefId>,
) -> Result<(), CodegenError> {
    match ty {
        scoop_lir::CType::Integer(_) | scoop_lir::CType::Boolean => Ok(()),
        scoop_lir::CType::DataPointer { pointee, storage } => {
            if let scoop_lir::CDataPointerStorage::Nullable(reference) = storage {
                let id = reference.definition();
                let Some(binding) = module.enums.nullable_data_pointer_binding(id) else {
                    return Err(CodegenError(format!(
                        "nullable C data pointer references enum {} without an exact data-pointer binding owned by this module",
                        id.into_raw()
                    )));
                };
                if binding != reference.pointee() {
                    return Err(CodegenError(format!(
                        "nullable C data pointer enum `{}` reference disagrees with its owned exact pointee binding",
                        module.enums[id].name
                    )));
                }
                if pointee != reference.pointee() {
                    return Err(CodegenError(format!(
                        "nullable C data pointer enum `{}` binds a different exact pointee",
                        module.enums[id].name
                    )));
                }
                if niche_pointer_kind(module, &LirType::Enum(id))
                    != Some(scoop_lir::NichePointerKind::Raw)
                {
                    return Err(CodegenError(format!(
                        "nullable C data pointer references enum `{}` without raw-pointer provenance",
                        module.enums[id].name
                    )));
                }
            }
            if let scoop_lir::CDataPointee::Object(pointee) = pointee {
                validate_c_type(module, pointee, false, visiting)?;
            }
            Ok(())
        }
        scoop_lir::CType::CodePointer { signature, storage } => {
            if let scoop_lir::CCodePointerStorage::Nullable(reference) = storage {
                let id = reference.definition();
                let Some(binding) = module.enums.nullable_code_pointer_binding(id) else {
                    return Err(CodegenError(format!(
                        "nullable C code pointer references enum {} without an exact code-pointer binding owned by this module",
                        id.into_raw()
                    )));
                };
                if binding != reference.signature() {
                    return Err(CodegenError(format!(
                        "nullable C code pointer enum `{}` reference disagrees with its owned exact signature binding",
                        module.enums[id].name
                    )));
                }
                if signature.as_ref() != reference.signature() {
                    return Err(CodegenError(format!(
                        "nullable C code pointer enum `{}` binds a different exact signature",
                        module.enums[id].name
                    )));
                }
                if niche_pointer_kind(module, &LirType::Enum(id))
                    != Some(scoop_lir::NichePointerKind::Code)
                {
                    return Err(CodegenError(format!(
                        "nullable C code pointer references enum `{}` without code-pointer provenance",
                        module.enums[id].name
                    )));
                }
            }
            for parameter in &signature.params {
                validate_c_type(module, parameter, false, visiting)?;
            }
            if let scoop_lir::CReturnType::Value(result) = &signature.return_type {
                validate_c_type(module, result, false, visiting)?;
            }
            Ok(())
        }
        scoop_lir::CType::Struct(reference) => {
            let id = reference.definition();
            if struct_by_value {
                validate_c_struct(module, id, visiting)
            } else if module.structs[id].is_c_layout() {
                Ok(())
            } else {
                Err(CodegenError(format!(
                    "struct `{}` is not a C-layout struct",
                    module.structs[id].name
                )))
            }
        }
    }
}

fn foreign_callback_family<'a>(
    module: &'a Module,
    id: scoop_lir::ForeignCallbackFamilyId,
    owner: &str,
) -> Result<&'a scoop_lir::ForeignCallbackFamily, CodegenError> {
    if id.into_raw().into_u32() as usize >= module.foreign_callback_families.len() {
        return Err(CodegenError(format!(
            "{owner} references invalid foreign callback family {}",
            id.into_raw()
        )));
    }
    Ok(&module.foreign_callback_families[id])
}

fn validate_foreign_callback_family(
    module: &Module,
    id: scoop_lir::ForeignCallbackFamilyId,
    family: &scoop_lir::ForeignCallbackFamily,
) -> Result<(), CodegenError> {
    let owner = format!("foreign callback family {}", id.into_raw());
    if family.callback.into_raw().into_u32() as usize >= module.structs.len() {
        return Err(CodegenError(format!(
            "{owner} references invalid callback struct {}",
            family.callback.into_raw()
        )));
    }
    let callback = &module.structs[family.callback];
    let callback_shape = callback.scoop_fields().is_some_and(|fields| {
        matches!(
            fields,
            [function, context]
                if function.ty == scoop_lir::CODE_PTR
                    && context.ty == scoop_lir::RAW_PTR
                    && function.layout.offset == 0
                    && context.layout.offset == 8
                    && function.layout.access_align == 8
                    && context.layout.access_align == 8
        )
    }) && !callback.interior_mutable
        && callback.size == 16
        && callback.align == 8;
    if !callback_shape {
        return Err(CodegenError(format!(
            "{owner} callback struct `{}` does not have the closed {{ ptr<code>, ptr<raw> }} representation",
            callback.name
        )));
    }

    let modes = scoop_lir::ForeignCallbackModes::checked(
        &module.enums,
        family.modes.reusable(),
        family.modes.one_shot(),
    );
    if modes != Some(family.modes) {
        return Err(CodegenError(format!(
            "{owner} carries invalid callback mode variant metadata"
        )));
    }
    let states = scoop_lir::ForeignCallbackStates::checked(
        &module.enums,
        family.states.registered(),
        family.states.active(),
        family.states.completed(),
        family.states.failed(),
    );
    if states != Some(family.states) {
        return Err(CodegenError(format!(
            "{owner} carries invalid callback state variant metadata"
        )));
    }
    let state = &module.enums[family.states.definition()];
    if !matches!(
        &state.repr,
        EnumRepr::Tagged {
            size: 8,
            align: 8,
            ..
        }
    ) || state.scan.contains_reference()
    {
        return Err(CodegenError(format!(
            "{owner} state enum `{}` does not have the closed four-state unit representation",
            state.name
        )));
    }
    let failure_result = scoop_lir::ForeignCallbackFailureResult::checked(
        &module.enums,
        family.failure_result.some_payload(),
        family.failure_result.none(),
    );
    if failure_result != Some(family.failure_result) {
        return Err(CodegenError(format!(
            "{owner} carries invalid managed-reference callback failure metadata"
        )));
    }
    Ok(())
}

fn validate_foreign_callbacks(module: &Module) -> Result<(), CodegenError> {
    let mut family_by_callback = HashMap::new();
    let mut protocol = None;
    for (id, family) in module.foreign_callback_families.iter() {
        validate_foreign_callback_family(module, id, family)?;
        if let Some(previous) = family_by_callback.insert(family.callback, id) {
            return Err(CodegenError(format!(
                "foreign callback struct {} belongs to both family {} and family {}",
                family.callback.into_raw(),
                previous.into_raw(),
                id.into_raw()
            )));
        }
        let identity = (family.modes, family.states, family.failure_result);
        if let Some(expected) = protocol {
            if expected != identity {
                return Err(CodegenError(format!(
                    "foreign callback family {} conflicts with the module's nominal state/failure protocol",
                    id.into_raw()
                )));
            }
        } else {
            protocol = Some(identity);
        }
    }

    let expected_params = [
        scoop_lir::MANAGED_PTR,
        scoop_lir::RAW_PTR,
        scoop_lir::RAW_PTR,
        scoop_lir::RAW_PTR,
    ];
    let expected_result = LirType::MachineScalar(MachineScalarKind::ForeignCallbackStatus);
    let mut trampolines = HashMap::new();
    let mut signatures = HashMap::new();
    for (id, bridge) in module.foreign_callback_bridges.iter() {
        foreign_callback_family(
            module,
            bridge.family,
            &format!("foreign callback bridge {}", id.into_raw()),
        )?;
        let family = &module.foreign_callback_families[bridge.family];
        if family.modes.runtime_code(bridge.mode).is_none() {
            return Err(CodegenError(format!(
                "foreign callback bridge {} carries a mode outside family {}",
                id.into_raw(),
                bridge.family.into_raw()
            )));
        }
        let Some(context_type) = bridge.params.get(bridge.context_index as usize) else {
            return Err(CodegenError(format!(
                "foreign callback bridge {} context index {} is outside its {} C parameters",
                id.into_raw(),
                bridge.context_index,
                bridge.params.len()
            )));
        };
        if !matches!(
            context_type,
            scoop_lir::CType::DataPointer {
                pointee: scoop_lir::CDataPointee::OpaqueVoid,
                storage: scoop_lir::CDataPointerStorage::Direct,
            }
        ) {
            return Err(CodegenError(format!(
                "foreign callback bridge {} context parameter {} must be a direct opaque C data pointer, found {context_type:?}",
                id.into_raw(),
                bridge.context_index
            )));
        }

        let abi = (
            bridge.trampoline.signature_descriptor_symbol(),
            bridge.params.as_slice(),
            &bridge.return_type,
            bridge.context_index,
        );
        if let Some(previous) = trampolines.insert(bridge.trampoline.entry().symbol(), abi) {
            if previous != abi {
                return Err(CodegenError(format!(
                    "foreign callback trampoline symbol @{} has conflicting ABI metadata",
                    bridge.trampoline.entry().symbol()
                )));
            }
        }
        let signature_abi = (
            bridge.trampoline.entry().symbol(),
            bridge.params.as_slice(),
            &bridge.return_type,
            bridge.context_index,
        );
        if let Some(previous) = signatures.insert(
            bridge.trampoline.signature_descriptor_symbol(),
            signature_abi,
        ) {
            if previous != signature_abi {
                return Err(CodegenError(format!(
                    "foreign callback signature symbol @{} has conflicting ABI metadata",
                    bridge.trampoline.signature_descriptor_symbol()
                )));
            }
        }

        let adapter_index = bridge.adapter.declaration().into_u32() as usize;
        let Some(adapter) = module.functions.get(adapter_index) else {
            return Err(CodegenError(format!(
                "foreign callback bridge {} has invalid managed adapter id {adapter_index}",
                id.into_raw()
            )));
        };
        let adapter_params_match = adapter.signature.arguments().len() == expected_params.len()
            && adapter
                .signature
                .arguments()
                .iter()
                .zip(expected_params.iter())
                .all(|(argument, expected)| {
                    matches!(argument, scoop_lir::AbiArgument::Direct(value)
                        if value.storage_type() == expected)
                });
        let adapter_result_matches = matches!(
            adapter.signature.result(),
            scoop_lir::AbiReturn::Direct(value) if value.storage_type() == &expected_result
        );
        if adapter.gc_effect != GcEffect::Managed
            || !adapter_params_match
            || !adapter_result_matches
        {
            return Err(CodegenError(format!(
                "foreign callback adapter @{} must be managed (ptr<managed>, ptr<raw>, ptr<raw>, ptr<raw>) -> machine<foreign-callback-status>",
                adapter.symbol()
            )));
        }
    }

    for function in &module.functions {
        for (_, block) in function.blocks.iter() {
            for instruction in &block.instructions {
                match instruction {
                    Instruction::FunctionAddress { out, target } => {
                        if checked_temp_type(function, *out, "function address result")?
                            != &scoop_lir::CODE_PTR
                        {
                            return Err(CodegenError(format!(
                                "function address @{} must produce ptr<code>",
                                function.symbol()
                            )));
                        }
                        match target {
                            scoop_lir::FunctionAddressTarget::Local(reference) => {
                                let index = reference.declaration().into_u32() as usize;
                                let Some(target) = module.functions.get(index) else {
                                    return Err(CodegenError(format!(
                                        "function address @{} references invalid local function id {index}",
                                        function.symbol()
                                    )));
                                };
                                let expected = match reference {
                                    scoop_lir::LocalFunctionRef::Managed(_) => GcEffect::Managed,
                                    scoop_lir::LocalFunctionRef::NoGc(_) => GcEffect::NoGc,
                                };
                                if target.gc_effect != expected {
                                    return Err(CodegenError(format!(
                                        "function address @{} has an effect-mismatched local target @{}",
                                        function.symbol(),
                                        target.symbol()
                                    )));
                                }
                            }
                            scoop_lir::FunctionAddressTarget::CallbackTrampoline(bridge)
                                if bridge.into_raw().into_u32() as usize
                                    >= module.callback_bridges.len() =>
                            {
                                return Err(CodegenError(format!(
                                    "function address @{} references invalid callback trampoline {}",
                                    function.symbol(),
                                    bridge.into_raw()
                                )));
                            }
                            scoop_lir::FunctionAddressTarget::CallbackTrampoline(_) => {}
                        }
                    }
                    Instruction::ForeignCallbackRegister {
                        out,
                        bridge,
                        closure,
                    } => {
                        if bridge.into_raw().into_u32() as usize
                            >= module.foreign_callback_bridges.len()
                        {
                            return Err(CodegenError(format!(
                                "foreign callback registration @{} references invalid bridge {}",
                                function.symbol(),
                                bridge.into_raw()
                            )));
                        }
                        let bridge = &module.foreign_callback_bridges[*bridge];
                        let family = foreign_callback_family(
                            module,
                            bridge.family,
                            &format!("foreign callback registration @{}", function.symbol()),
                        )?;
                        let closure_ty = checked_value_type(
                            module,
                            function,
                            *closure,
                            "foreign callback registration closure",
                        )?;
                        let result_ty = checked_temp_type(
                            function,
                            *out,
                            "foreign callback registration result",
                        )?;
                        if closure_ty != scoop_lir::MANAGED_PTR
                            || result_ty != &LirType::Struct(family.callback)
                        {
                            return Err(CodegenError(format!(
                                "foreign callback registration @{} for family {} requires a managed closure and exact callback struct {}, got closure {} and result {}",
                                function.symbol(),
                                bridge.family.into_raw(),
                                family.callback.into_raw(),
                                closure_ty.dump(),
                                result_ty.dump()
                            )));
                        }
                    }
                    Instruction::ForeignCallbackOperation(operation) => {
                        let family_id = operation.family();
                        let family = foreign_callback_family(
                            module,
                            family_id,
                            &format!("foreign callback operation @{}", function.symbol()),
                        )?;
                        let callback_ty = checked_value_type(
                            module,
                            function,
                            operation.callback(),
                            "foreign callback operation callback",
                        )?;
                        if callback_ty != LirType::Struct(family.callback) {
                            return Err(CodegenError(format!(
                                "foreign callback operation @{} for family {} requires exact callback struct {}, got {}",
                                function.symbol(),
                                family_id.into_raw(),
                                family.callback.into_raw(),
                                callback_ty.dump()
                            )));
                        }
                        let result_valid = match *operation {
                            scoop_lir::ForeignCallbackOperation::Retain { out, .. } => {
                                checked_temp_type(function, out, "foreign callback retain result")?
                                    == &LirType::Struct(family.callback)
                            }
                            scoop_lir::ForeignCallbackOperation::Release { .. } => true,
                            scoop_lir::ForeignCallbackOperation::State { out, .. } => {
                                checked_temp_type(function, out, "foreign callback state result")?
                                    == &LirType::Enum(family.states.definition())
                            }
                            scoop_lir::ForeignCallbackOperation::Failure { out, .. } => {
                                checked_temp_type(function, out, "foreign callback failure result")?
                                    == &LirType::Enum(family.failure_result.definition())
                            }
                        };
                        if !result_valid {
                            return Err(CodegenError(format!(
                                "foreign callback operation @{} for family {} has a non-protocol result type",
                                function.symbol(),
                                family_id.into_raw()
                            )));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}
