use scoop_mir as mir;

fn lower(source: &str) -> mir::Module {
    let output = super::lower(&[
        super::complete_core_file(),
        scoop_parser::parse(source).unwrap(),
    ])
    .unwrap();
    scoop_mir_lower::lower(&output.local).unwrap()
}

#[test]
fn construction_publishes_once_after_outer_initializer_and_keeps_hook_owner() {
    let module = lower(
        r#"
        class Owner(private val handle: Int) {
            constructor(): this(7) {}
            release { val saved = handle }
        }
        fun make(): Owner = Owner()
        fun identity(value: Owner): Owner = value
        fun main() { identity(make()) }
        "#,
    );
    assert_eq!(module.release_hooks.len(), 1);
    let (hook_id, hook) = module.release_hooks.iter().next().unwrap();
    assert_eq!(hook.code.gc_effect, mir::GcEffect::NoGc);
    assert_eq!(hook.code.return_ty, mir::Type::Unit);
    assert!(hook.code.params.is_empty());
    assert_eq!(
        module.classes[hook.owner].release_policy,
        mir::ReleasePolicy::SynchronousGcFree {
            hook: mir::ReleaseHookTarget::Local(hook_id),
        }
    );
    assert!(
        module
            .meta
            .local_values
            .iter()
            .any(|value| { value.owner() == mir::LocalValueOwner::ReleaseHook(hook_id) })
    );
    let mut publications = 0;
    for (_, function) in module.functions.iter() {
        for (_, block) in function.body.blocks.iter() {
            for (index, statement) in block.statements.iter().enumerate() {
                if let mir::StatementKind::PublishReleaseReady { class, receiver } = &statement.kind
                {
                    publications += 1;
                    assert_eq!(function.name, "make");
                    assert_eq!(*class, hook.owner);
                    assert_eq!(receiver.ty, mir::Type::Class(hook.owner));
                    let mir::ExprKind::Local(receiver) = receiver.kind else {
                        panic!("publication reloads the allocation's live local");
                    };
                    let mir::StatementKind::Call(mir::CallEffect::Unit(initializer)) =
                        &block.statements[index - 1].kind
                    else {
                        panic!("the selected initializer immediately precedes publication");
                    };
                    assert!(
                        matches!(initializer.args[0].kind, mir::ExprKind::Local(local) if local == receiver)
                    );
                }
            }
        }
    }
    assert_eq!(publications, 1);
}

#[test]
fn exceptional_constructor_edge_never_publishes_ready() {
    let module = lower(
        r#"
        class Owner(private val handle: Int) {
            release { val saved = handle }
        }
        fun make(): Owner {
            try { return Owner(7) }
            catch (error: Throwable) { throw error }
        }
        fun main() { make() }
        "#,
    );
    let function = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "make")
        .unwrap()
        .1;
    let (_, block) = function
        .body
        .blocks
        .iter()
        .find(|(_, block)| {
            block.statements.iter().any(|statement| {
                matches!(
                    statement.kind,
                    mir::StatementKind::PublishReleaseReady { .. }
                )
            })
        })
        .unwrap();
    let unwind = block.unwind.expect("constructor failure has a catch edge");
    assert!(
        function.body.blocks[unwind]
            .statements
            .iter()
            .all(|statement| {
                !matches!(
                    statement.kind,
                    mir::StatementKind::PublishReleaseReady { .. }
                )
            })
    );
    assert!(matches!(block.terminator, mir::Terminator::Return { .. }));
}
