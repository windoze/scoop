use super::*;

pub(super) fn validate_support_callables(
    module: &Module,
) -> Result<HashSet<FunctionId>, MirValidationError> {
    let mut functions = HashSet::new();
    let mut start_results = HashSet::new();
    for (index, start) in module.meta.coroutine_starts.iter().enumerate() {
        let location = MirValidationLocation::CoroutineStart {
            start: u32::try_from(index).expect("MIR metadata index fits u32"),
        };
        if CoroutineStart::checked(
            &module.functions,
            start.result().clone(),
            start.function(),
            start.identity().clone(),
        )
        .is_none()
        {
            return Err(error(
                location,
                "coroutine start helper no longer has its exact erased signature",
            ));
        }
        let exact = start.identity().result_record().id();
        if !start_results.insert(exact) {
            return Err(error(
                location,
                "an exact result can have only one coroutine start helper",
            ));
        }
        if step_exact_result(module, start.result()) != Some(exact) {
            return Err(error(
                location,
                "coroutine start helper and CoroutineStep disagree on the exact result",
            ));
        }
        if source_exact_type(module, start.result()) != Some(exact) {
            return Err(error(
                location,
                "coroutine start result does not match the source exact-type relation",
            ));
        }
        let Some(signature) = exact_support_signature(module, start.function()) else {
            return Err(error(
                location,
                "coroutine start signature has no complete exact-type relation",
            ));
        };
        if start.identity().signature_record().signature() != &signature {
            return Err(error(
                location,
                "coroutine start identity does not retain its exact logical signature",
            ));
        }
        if !functions.insert(start.function()) {
            return Err(error(
                location,
                "each coroutine support function must be uniquely owned",
            ));
        }
    }
    Ok(functions)
}

fn exact_support_signature(
    module: &Module,
    function: FunctionId,
) -> Option<scoop_identity::ExactCallableSignature> {
    let function = arena_get(&module.functions, function)?;
    let parameters = function
        .params
        .iter()
        .map(|parameter| source_exact_type(module, &parameter.ty))
        .collect::<Option<Vec<_>>>()?;
    let result = source_exact_type(module, &function.return_ty)?;
    Some(scoop_identity::ExactCallableSignature::new(
        scoop_identity::Effect::Ordinary,
        None,
        parameters,
        result,
    ))
}

fn step_exact_result(
    module: &Module,
    result: &Type,
) -> Option<scoop_identity::PersistentExactTypeId> {
    let mut matching = module.meta.coroutine_steps.iter().filter_map(|(_, step)| {
        (step.result() == result).then_some(step.identity().result_record().id())
    });
    let exact = matching.next()?;
    matching.next().is_none().then_some(exact)
}
