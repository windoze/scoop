use std::collections::HashSet;

use la_arena::{Arena, Idx};
use scoop_identity::{
    CallableMaterializationContext, CborIdentityRecord, Effect, ExactCallableSignature,
    GeneratedNominalKey, OdrGroupId, PersistentTypeId,
};

use super::*;

type GeneratedTypeRecord = CborIdentityRecord<PersistentTypeId, GeneratedNominalKey>;

pub(super) fn validate_function_bridge_metadata(module: &Module) -> Result<(), MirValidationError> {
    let mut physical_entries = HashSet::new();
    let mut functions = HashSet::new();
    let mut callables = HashSet::new();

    for (index, bridge) in module.meta.function_bridges.iter().enumerate() {
        let location = MirValidationLocation::FunctionBridge {
            bridge: index as u32,
        };
        let Some(class) = arena_get(&module.closure_classes, bridge.class()) else {
            return invalid(location, "the bridge closure class does not exist");
        };
        if FunctionBridgeMaterialization::checked(
            bridge.class(),
            class,
            bridge.target(),
            bridge.function(),
            bridge.identity().clone(),
        )
        .is_none()
        {
            return invalid(
                location,
                "the generated bridge does not have one physical dispatch entry",
            );
        }
        if arena_get(&module.functions, bridge.function()).is_none() {
            return invalid(location, "the generated bridge function does not exist");
        }
        if !physical_entries.insert((bridge.class(), bridge.target())) {
            return invalid(
                location,
                "the same closure and target have more than one generated bridge",
            );
        }
        if !functions.insert(bridge.function()) {
            return invalid(
                location,
                "one generated bridge function is used by more than one dispatch entry",
            );
        }
        if !callables.insert(bridge.identity().callable_record().id()) {
            return invalid(
                location,
                "one generated callable identity is used by more than one bridge",
            );
        }

        let expected_environment = environment_record(module, bridge.class()).ok_or_else(|| {
            function_bridge_error(location, "the bridge class has no persistent environment")
        })?;
        if bridge.identity().environment_record() != expected_environment {
            return invalid(
                location,
                "the generated bridge identifies a different closure environment",
            );
        }
        let target = exact_signature(module, bridge.target()).ok_or_else(|| {
            function_bridge_error(
                location,
                "the bridge target has no complete exact signature identity",
            )
        })?;
        let root =
            expected_root(module, bridge.class(), expected_environment).ok_or_else(|| {
                function_bridge_error(location, "the bridge environment root is inconsistent")
            })?;
        let odr_group = match root {
            ExpectedRoot::Strong => {
                if bridge.identity().odr_member_record().is_some() {
                    return invalid(
                        location,
                        "a parameter-free source bridge must be a strong callable",
                    );
                }
                None
            }
            ExpectedRoot::Known(group) => {
                if bridge
                    .identity()
                    .odr_member_record()
                    .is_none_or(|member| member.key().group() != group)
                {
                    return invalid(
                        location,
                        "the bridge ODR member belongs to a different environment root",
                    );
                }
                Some(group)
            }
            ExpectedRoot::Materialized => Some(
                bridge
                    .identity()
                    .odr_member_record()
                    .ok_or_else(|| {
                        function_bridge_error(
                            location,
                            "a materialized source bridge requires an ODR member",
                        )
                    })?
                    .key()
                    .group(),
            ),
        };
        let rebuilt = FunctionBridgeIdentity::new(expected_environment, target, odr_group)
            .map_err(|_| {
                function_bridge_error(location, "the function bridge identity is inconsistent")
            })?;
        if &rebuilt != bridge.identity() {
            return invalid(location, "the function bridge identity is not canonical");
        }
    }

    for (class_id, class) in module.closure_classes.iter() {
        let location = MirValidationLocation::FunctionBridge {
            bridge: module.meta.function_bridges.len() as u32,
        };
        if class.bridges.len() != 1 {
            return invalid(
                location,
                "a closure requires exactly one fixed dynamic invoke",
            );
        }
        let Some(invoke) = arena_get(&module.closure_invoke_functions, class.invoke) else {
            continue;
        };
        let Some(source) = arena_get(&module.function_types, class.function_type) else {
            continue;
        };
        for entry in &class.bridges {
            let Some(target) = arena_get(&module.function_types, entry.target) else {
                return invalid(location, "a closure dispatch entry has an invalid target");
            };
            if target.is_suspend != source.is_suspend
                || target.parameter_types.len() != source.parameter_types.len()
                || target.parameter_types.iter().any(|ty| *ty != Type::Any)
                || target.return_type != Type::Any
            {
                return invalid(
                    location,
                    "a dynamic invoke preserves arity and effect with Any parameters and result",
                );
            }
            if arena_get(&module.functions, entry.function).is_none() {
                return invalid(location, "a closure dispatch entry has an invalid target");
            }
            if entry.target == class.function_type {
                if entry.function != invoke.function {
                    return invalid(
                        location,
                        "the source-signature dispatch entry must reuse the closure invoke",
                    );
                }
            } else if !physical_entries.contains(&(class_id, entry.target)) {
                return invalid(
                    location,
                    "a signature-changing dispatch entry has no persistent bridge identity",
                );
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum ExpectedRoot {
    Strong,
    Known(OdrGroupId),
    Materialized,
}

fn environment_record(module: &Module, class: ClosureClassId) -> Option<&GeneratedTypeRecord> {
    if let Some(environment) = module
        .meta
        .closure_environments
        .iter()
        .find(|environment| environment.class() == class)
    {
        return Some(environment.identity().generated_type_record());
    }
    if let Some((_, adapter)) = module
        .meta
        .closure_adapters
        .iter()
        .find(|(_, adapter)| adapter.class() == class)
    {
        return Some(adapter.identity().environment_record());
    }
    module
        .meta
        .dynamic_closure_adapters
        .iter()
        .find(|(_, adapter)| adapter.class() == class)
        .map(|(_, adapter)| adapter.identity().environment_record())
}

fn expected_root(
    module: &Module,
    class: ClosureClassId,
    environment: &GeneratedTypeRecord,
) -> Option<ExpectedRoot> {
    if module
        .meta
        .closure_environments
        .iter()
        .any(|candidate| candidate.class() == class)
    {
        let GeneratedNominalKey::ClosureEnvironment { callable, .. } = environment.key() else {
            return None;
        };
        return Some(match callable.context() {
            CallableMaterializationContext::NoSubstitution => ExpectedRoot::Strong,
            CallableMaterializationContext::Application(_)
            | CallableMaterializationContext::InitializationApplication(_) => {
                ExpectedRoot::Materialized
            }
        });
    }
    let group = module
        .meta
        .closure_adapters
        .iter()
        .find(|(_, adapter)| adapter.class() == class)
        .map(|(_, adapter)| adapter.identity().odr_group_record().id())
        .or_else(|| {
            module
                .meta
                .dynamic_closure_adapters
                .iter()
                .find(|(_, adapter)| adapter.class() == class)
                .map(|(_, adapter)| adapter.identity().odr_group_record().id())
        })?;
    Some(ExpectedRoot::Known(group))
}

fn exact_signature(module: &Module, target: FunctionTypeId) -> Option<ExactCallableSignature> {
    let function_type = arena_get(&module.function_types, target)?;
    let parameters = function_type
        .parameter_types
        .iter()
        .map(|ty| {
            module
                .meta
                .source_exact_types
                .get(ty)
                .map(|exact| exact.identity_record().id())
        })
        .collect::<Option<Vec<_>>>()?;
    let result = module
        .meta
        .source_exact_types
        .get(&function_type.return_type)?
        .identity_record()
        .id();
    Some(ExactCallableSignature::new(
        if function_type.is_suspend {
            Effect::Suspend
        } else {
            Effect::Ordinary
        },
        None,
        parameters,
        result,
    ))
}

fn invalid<T>(
    location: MirValidationLocation,
    reason: &'static str,
) -> Result<T, MirValidationError> {
    Err(function_bridge_error(location, reason))
}

fn function_bridge_error(
    location: MirValidationLocation,
    reason: &'static str,
) -> MirValidationError {
    MirValidationError {
        location,
        kind: MirValidationErrorKind::InvalidFunctionBridge { reason },
    }
}

fn arena_get<T>(arena: &Arena<T>, id: Idx<T>) -> Option<&T> {
    ((id.into_raw().into_u32() as usize) < arena.len()).then(|| &arena[id])
}
