use std::collections::HashSet;

use super::*;

pub(super) fn validate_callable_functions(module: &Module) -> Result<(), MirValidationError> {
    let mut emitted = HashSet::new();
    for &function in &module.top_level {
        if !emitted.insert(function) {
            return invalid(function, "the function is emitted more than once");
        }
        if !module
            .functions
            .iter()
            .any(|(candidate, _)| candidate == function)
        {
            return invalid(function, "the emitted function does not exist");
        }
        match module.meta.callable_signature_subject(function) {
            None => {
                return invalid(
                    function,
                    "the function is not claimed by exactly one callable materialization",
                );
            }
            Some(CallableSignatureSubject::Strong(
                scoop_identity::CallableOwner::GenericTemplate(_)
                | scoop_identity::CallableOwner::Application(_),
            )) => {
                return invalid(
                    function,
                    "the strong callable subject does not name a concrete definition",
                );
            }
            Some(CallableSignatureSubject::Strong(_) | CallableSignatureSubject::Odr(_)) => {}
        }
    }

    if let MirOutput::Executable { entry, arguments } = module.output {
        if !emitted.contains(&entry) {
            return invalid(entry, "the entry function is not emitted");
        }
        let parameters = &module.functions[entry].params;
        let valid = match arguments {
            ProgramArguments::Unused => parameters.is_empty(),
            ProgramArguments::Local(builder) => {
                emitted.contains(&builder)
                    && parameters.len() == 1
                    && module.functions[builder].params.is_empty()
                    && module.functions[builder].return_ty == parameters[0].ty
                    && module.functions[builder].gc_effect == GcEffect::Managed
            }
            ProgramArguments::External(builder) => {
                parameters.len() == 1
                    && module.meta.external_callables.iter().any(|(id, callable)| {
                        id == builder && callable.gc_effect() == GcEffect::Managed
                    })
            }
        };
        if !valid {
            return invalid(
                entry,
                "the argument builder does not match the entry parameter list",
            );
        }
    }

    Ok(())
}

fn invalid(function: FunctionId, reason: &'static str) -> Result<(), MirValidationError> {
    Err(MirValidationError {
        location: MirValidationLocation::CallableFunction { function },
        kind: MirValidationErrorKind::InvalidCallableFunction { reason },
    })
}
