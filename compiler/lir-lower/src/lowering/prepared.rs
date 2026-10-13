//! Owned preparation state, distinct from complete LIR output.

use super::*;

pub struct PreparedLir<'a, 'layout> {
    pub(crate) input: &'a mir::ConeMirInput,
    pub(super) selected_layout: Option<&'a lir::StrongProductionDependencySelectionV2<'layout>>,
    pub(super) context: LoweringContext,
    pub(super) identity_roots: IdentityRoots<'a>,
    pub(super) external_type_descriptors: Arena<lir::ExternalTypeDescriptor>,
    pub(super) dependency_types: dependency_types::DependencyTypeDescriptors,
    pub(super) imported_runtime_string: Option<lir::TypeDescriptorRef>,
    pub(super) globals: Arena<lir::Global>,
    pub(super) string_global_map: HashMap<mir::StringConstId, lir::GlobalId>,
    pub(super) enums: lir::EnumDefs,
    pub(super) structs: lir::StructDefs,
    pub(super) external_callables: Arena<lir::ExternalCallable>,
    pub(super) external_callable_map: HashMap<mir::ExternalCallableUseId, lir::ExternalCallableId>,
    pub(super) planning: lir::AbiPlanningInput,
    pub(super) native_abi: native_abi::PreparedNativeAbi,
    pub(super) requests: abi::plans::SignatureRequests,
}

impl PreparedLir<'_, '_> {
    pub fn abi_input(&self) -> &lir::AbiPlanningInput {
        &self.planning
    }
}

pub fn prepare<'a, 'layout>(
    input: &'a mir::ConeMirInput,
    external_descriptors: &[lir::ExternalTypeDescriptor],
    selected_callables: &lir::SelectedExternalLirSet,
    target_profile: lir::LirTargetProfile,
    selected_layout: Option<&'a lir::StrongProductionDependencySelectionV2<'layout>>,
) -> Result<PreparedLir<'a, 'layout>, LirLoweringError> {
    let module = input.module();
    let context = LoweringContext::new(target_profile);
    let identity_roots = IdentityRoots::new(input);
    let mut external_type_descriptors = external_descriptors.iter().copied().collect();
    let dependency_types = dependency_types::lower(
        input,
        target_profile,
        selected_layout,
        &mut external_type_descriptors,
    )?;
    let imported_runtime_string = dependency_types
        .source
        .iter()
        .find_map(|(ty, reference)| (*ty == mir::Type::String).then_some(*reference));
    validate_strong_materialization(input, &identity_roots, &dependency_types)
        .map_err(LirLoweringError::Capability)?;
    // Every string selected by the sealed materialization plan becomes a
    // global with the same typed identity.
    let mut globals = Arena::new();
    let mut string_global_map: HashMap<mir::StringConstId, lir::GlobalId> = HashMap::new();
    for &id in input.materialization().strings() {
        let string = &module.strings[id];
        let global = globals.alloc(lir::Global {
            address_kind: lir::PointerKind::Managed,
            scan: lir::RefScan::None,
            init: lir::GlobalInit::StringConst {
                identity: lir::ImmortalObjectIdentity::from_key(
                    string.identity.clone(),
                    identity_roots.for_immortal_object(&string.identity),
                )
                .expect("validated MIR string identities have canonical CBOR records"),
                value: string.value.clone(),
            },
        });
        string_global_map.insert(id, global);
    }

    // Enum definitions with fixed representations, in the MIR arena's
    // order: `mir::EnumId` and `lir::EnumDefId` align.
    let enums = lower_enums(&context, module)?;
    // Struct ids also transpose 1:1. Their definitions retain the exact
    // physical layout needed by codegen and C bridge generation.
    let structs = lower_structs(&context, module, &enums)?;
    let (external_callables, external_callable_map) = lower_external_callables(
        &context,
        input,
        selected_callables,
        selected_layout,
        &structs,
        &enums,
    )?;
    let native_abi = native_abi::prepare(
        &context,
        module,
        &structs,
        &enums,
        identity_roots
            .source_nominal_shapes()
            .iter()
            .map(mir::SourceNominalShapeRoot::ty),
    )?;
    let (planning, requests) = abi::plans::prepare(
        &context,
        module,
        &structs,
        &enums,
        &external_callables,
        &external_callable_map,
    )?;
    Ok(PreparedLir {
        input,
        selected_layout,
        context,
        identity_roots,
        external_type_descriptors,
        dependency_types,
        imported_runtime_string,
        globals,
        string_global_map,
        enums,
        structs,
        external_callables,
        external_callable_map,
        planning,
        requests,
        native_abi,
    })
}
