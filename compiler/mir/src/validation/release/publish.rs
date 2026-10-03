use super::*;

pub(super) fn validate(
    module: &Module,
    owner: LocalValueOwner,
    block: &BasicBlock,
    index: usize,
    class: ClassId,
    receiver: &Expr,
) -> Result<(), MirValidationErrorKind> {
    let failure = || {
        invalid(
            "release publication requires a fresh exact object immediately after its outer initializer",
        )
    };
    if !matches!(owner, LocalValueOwner::Function(_))
        || receiver.ty != Type::Class(class)
        || !arena_get(&module.classes, class).is_some_and(|class| {
            matches!(
                class.release_policy,
                ReleasePolicy::SynchronousGcFree { .. }
            )
        })
        || index < 2
    {
        return Err(failure());
    }
    let ExprKind::Local(local) = receiver.kind else {
        return Err(failure());
    };
    let StatementKind::Call(CallEffect::Unit(call)) = &block.statements[index - 1].kind else {
        return Err(failure());
    };
    if call.target.kind != CallKind::Direct
        || !call.args.first().is_some_and(|arg| {
            arg.ty == receiver.ty && matches!(arg.kind, ExprKind::Local(id) if id == local)
        })
        || !matches!(&block.statements[index - 2].kind,
            StatementKind::Assign { local: allocated, value: Expr { kind: ExprKind::ClassAlloc { class_id }, .. } }
            if *allocated == local && *class_id == class)
    {
        return Err(failure());
    }
    let constructor = match call.target.callee {
        Callee::User(function) => local_constructor(module, function),
        Callee::Monomorphized(instance) => arena_get(&module.meta.instances, instance)
            .is_some_and(|instance| local_constructor(module, instance.function)),
        Callee::External(external) => arena_get(&module.meta.external_callables, external)
            .is_some_and(|external| {
                matches!(
                    external.reference().implementation(),
                    scoop_identity::StrongCallableDefinitionOwner::Constructor(_)
                )
            }),
        _ => false,
    };
    if constructor { Ok(()) } else { Err(failure()) }
}

fn local_constructor(module: &Module, function: FunctionId) -> bool {
    module
        .meta
        .source_callable_materializations
        .get(function)
        .is_some_and(|source| {
            matches!(
                source.materialization().template(),
                scoop_identity::CallableTemplateOwner::Constructor(_)
            )
        })
}
