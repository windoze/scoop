use super::*;

mod expressions;
mod publish;
pub(super) use expressions::validate_expression;

fn invalid(reason: &'static str) -> MirValidationErrorKind {
    MirValidationErrorKind::InvalidRelease { reason }
}

fn arena_get<T>(arena: &Arena<T>, id: Idx<T>) -> Option<&T> {
    ((id.into_raw().into_u32() as usize) < arena.len()).then(|| &arena[id])
}

pub(super) fn block_location(owner: LocalValueOwner, block: BlockId) -> MirValidationLocation {
    match owner {
        LocalValueOwner::Function(function) => {
            MirValidationLocation::FunctionBlock { function, block }
        }
        LocalValueOwner::ReleaseHook(hook) => {
            MirValidationLocation::ReleaseHookBlock { hook, block }
        }
    }
}

pub(super) fn validate_policies(module: &Module) -> Result<(), MirValidationError> {
    for (id, class) in module.classes.iter() {
        let ReleasePolicy::SynchronousGcFree { hook } = class.release_policy else {
            continue;
        };
        let valid = class.modifier == ClassModifier::Final
            && matches!(class.representation, ClassRepresentation::Declared { .. })
            && match hook {
                ReleaseHookTarget::Local(hook) => {
                    arena_get(&module.release_hooks, hook).is_some_and(|hook| hook.owner == id)
                }
                ReleaseHookTarget::External { owner } => owner == id,
            };
        if !valid {
            return Err(MirValidationError {
                location: MirValidationLocation::ReleasePolicy { class: id },
                kind: invalid("release policy requires a final declared class and its own hook"),
            });
        }
    }
    Ok(())
}

pub(super) fn validate_signature(
    module: &Module,
    id: ReleaseHookId,
    hook: &ReleaseHook,
) -> Result<(), MirValidationError> {
    let expected = ReleasePolicy::SynchronousGcFree {
        hook: ReleaseHookTarget::Local(id),
    };
    if !arena_get(&module.classes, hook.owner).is_some_and(|class| class.release_policy == expected)
        || hook.code.gc_effect != GcEffect::NoGc
        || !hook.code.params.is_empty()
        || hook.code.return_ty != Type::Unit
        || hook
            .code
            .body
            .locals
            .iter()
            .any(|(_, local)| !gc_free(module, &local.ty))
    {
        return Err(MirValidationError {
            location: MirValidationLocation::ReleaseHook { hook: id },
            kind: invalid(
                "release body requires its exact owner, a NoGc Unit signature and GC-free locals",
            ),
        });
    }
    Ok(())
}

pub(super) fn validate_statement(
    module: &Module,
    owner: LocalValueOwner,
    block: &BasicBlock,
    index: usize,
) -> Result<(), MirValidationErrorKind> {
    let statement = &block.statements[index].kind;
    if let StatementKind::PublishReleaseReady { class, receiver } = statement {
        return publish::validate(module, owner, block, index, *class, receiver);
    }
    if !matches!(owner, LocalValueOwner::ReleaseHook(_)) {
        return Ok(());
    }
    match statement {
        StatementKind::Expr(_) | StatementKind::ValDecl { .. } | StatementKind::Assign { .. } => {
            Ok(())
        }
        StatementKind::GlobalAssign { global, .. } => check_global(module, *global),
        StatementKind::Call(CallEffect::Unit(call))
        | StatementKind::Call(CallEffect::Value { call, .. }) => {
            let allowed = call.target.kind == CallKind::Direct
                && match call.target.callee {
                    Callee::User(id) => arena_get(&module.functions, id)
                        .is_some_and(|f| f.gc_effect == GcEffect::NoGc),
                    Callee::Monomorphized(id) => arena_get(&module.meta.instances, id)
                        .and_then(|instance| arena_get(&module.functions, instance.function))
                        .is_some_and(|f| f.gc_effect == GcEffect::NoGc),
                    Callee::External(id) => arena_get(&module.meta.external_callables, id)
                        .is_some_and(|f| f.gc_effect() == GcEffect::NoGc),
                    Callee::Extern(id) => {
                        arena_get(&module.extern_functions, id).is_some_and(|f| f.abi.is_c())
                    }
                    Callee::CoroutineSuspend { .. }
                    | Callee::Closure(_)
                    | Callee::FunctionBridge(_)
                    | Callee::Runtime(_) => false,
                };
            if allowed {
                Ok(())
            } else {
                Err(invalid(
                    "release call requires a static NoGc Scoop or direct C target",
                ))
            }
        }
        StatementKind::ArraySet { .. }
        | StatementKind::FieldSet { .. }
        | StatementKind::AtomicFieldStore { .. }
        | StatementKind::Eh(_)
        | StatementKind::PublishReleaseReady { .. } => Err(invalid(
            "managed storage and EH operations are forbidden in release bodies",
        )),
    }
}

pub(super) fn validate_control_flow(
    owner: LocalValueOwner,
    block: &BasicBlock,
) -> Result<(), MirValidationErrorKind> {
    if !matches!(owner, LocalValueOwner::ReleaseHook(_)) {
        return Ok(());
    }
    if block.unwind.is_some()
        || !matches!(
            block.terminator,
            Terminator::Goto(_)
                | Terminator::Branch { .. }
                | Terminator::Return { value: None }
                | Terminator::Unreachable
        )
    {
        return Err(invalid(
            "release control flow cannot unwind, throw or return a value",
        ));
    }
    Ok(())
}

fn gc_free(module: &Module, ty: &Type) -> bool {
    match ty {
        Type::Unit | Type::Integer(_) | Type::MachineScalar(_) | Type::Boolean | Type::Ptr(_) => {
            true
        }
        Type::Struct(id) => arena_get(&module.structs, *id).is_some_and(|ty| ty.gc_free),
        Type::Enum(id, _) => arena_get(&module.enums, *id).is_some_and(|ty| ty.gc_free),
        Type::Tuple(elements) => elements.iter().all(|ty| gc_free(module, ty)),
        Type::Context(_)
        | Type::String
        | Type::Class(_)
        | Type::Interface(_)
        | Type::Any
        | Type::Function(_)
        | Type::FunPtr(_) => false,
    }
}

fn check_global(module: &Module, id: GlobalId) -> Result<(), MirValidationErrorKind> {
    if !arena_get(&module.globals, id).is_some_and(|global| {
        gc_free(module, &global.ty)
            && !matches!(
                global.storage,
                GlobalStorage::Local {
                    thread_local: true,
                    ..
                } | GlobalStorage::Extern {
                    thread_local: true,
                    ..
                }
            )
    }) {
        return Err(invalid(
            "release global access requires GC-free non-TLS storage",
        ));
    }
    Ok(())
}
