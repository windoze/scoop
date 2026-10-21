//! Map initialization registrations to module globals by unit identity.

use super::*;

pub(super) fn initialization_unit_globals<'ctx>(
    module: &Module,
    emitted: &EmittedStrongInitializationUnitRegistrationSetV1<'ctx>,
) -> Result<BTreeMap<scoop_lir::InitializationUnitId, GlobalValue<'ctx>>, CodegenError> {
    initialization_unit_globals_from_pairs(
        module,
        emitted
            .registrations()
            .iter()
            .map(|registration| (registration.unit(), registration.registration_descriptor())),
    )
}

pub(super) fn initialization_unit_requests<D, C, I>(
    module: &Module,
    production: &scoop_lir::ConeProductionSection<D, C, I>,
) -> Result<
    BTreeMap<scoop_lir::InitializationUnitId, scoop_lir::PersistentSymbolRequest>,
    CodegenError,
> {
    let plans = production
        .registration_production()
        .initialization_units()
        .registrations()
        .iter()
        .map(|plan| (plan.semantic().unit(), plan.registration_symbol()))
        .collect::<BTreeMap<_, _>>();
    let requests = module
        .initialization_units
        .iter()
        .map(|(id, unit)| {
            plans
                .get(&unit.identity.id())
                .copied()
                .map(|request| (id, request))
                .ok_or_else(|| {
                    CodegenError(format!(
                        "initialization unit {} has no strong production plan",
                        unit.identity.id()
                    ))
                })
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    if requests.len() != plans.len() {
        return Err(CodegenError(format!(
            "strong initialization declaration coverage mismatch: LIR {}, planned {}",
            requests.len(),
            plans.len(),
        )));
    }
    Ok(requests)
}

pub(super) fn declare_initialization_unit_globals<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    requests: &BTreeMap<scoop_lir::InitializationUnitId, scoop_lir::PersistentSymbolRequest>,
    selected: &std::collections::BTreeSet<scoop_lir::InitializationUnitId>,
) -> Result<BTreeMap<scoop_lir::InitializationUnitId, GlobalValue<'ctx>>, CodegenError> {
    let ty =
        runtime_metadata_v1::RuntimeMetadataV1Types::new(context).initialization_unit_descriptor();
    selected
        .iter()
        .map(|&id| {
            let request = requests[&id];
            let symbol = request.symbol();
            if llvm.get_global(symbol.as_str()).is_some()
                || llvm.get_function(symbol.as_str()).is_some()
            {
                return Err(CodegenError(format!(
                    "initialization registration declaration `{symbol}` collides with an LLVM value"
                )));
            }
            let global = llvm.add_global(ty, None, symbol.as_str());
            global.set_constant(true);
            apply_persistent_linkage(&global, request, false)?;
            Ok((id, global))
        })
        .collect()
}

pub(crate) fn initialization_unit_globals_from_pairs<'ctx>(
    module: &Module,
    emitted: impl IntoIterator<Item = (scoop_lir::PersistentInitializationUnitId, GlobalValue<'ctx>)>,
) -> Result<BTreeMap<scoop_lir::InitializationUnitId, GlobalValue<'ctx>>, CodegenError> {
    let mut by_unit = std::collections::BTreeMap::new();
    for (unit, descriptor) in emitted {
        if by_unit.insert(unit, descriptor).is_some() {
            return Err(CodegenError(
                "strong initialization emission contains duplicate unit ids".to_string(),
            ));
        }
    }
    let globals = module
        .initialization_units
        .iter()
        .map(|(id, unit)| {
            by_unit
                .get(&unit.identity.id())
                .copied()
                .map(|global| (id, global))
                .ok_or_else(|| {
                    CodegenError(format!(
                        "initialization unit {} has no emitted initialization registration",
                        unit.identity.id()
                    ))
                })
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    if globals.len() != by_unit.len() {
        return Err(CodegenError(format!(
            "strong initialization coverage mismatch: LIR {}, emitted {}",
            globals.len(),
            by_unit.len()
        )));
    }
    Ok(globals)
}
