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

#[test]
fn output_validation_rejects_wrong_release_owner_and_field() {
    let mut module = lower(
        "class Owner(val handle: Int) { release { val saved = handle } } fun main() { val owner = Owner(7) }",
    );
    let (id, _) = module.release_hooks.iter().next().unwrap();
    let owner = module.release_hooks[id].owner;
    module.classes[owner].release_policy = mir::ReleasePolicy::None;
    assert!(matches!(
        module.validate().unwrap_err().kind,
        mir::MirValidationErrorKind::InvalidRelease { .. }
    ));
    module.classes[owner].release_policy = mir::ReleasePolicy::SynchronousGcFree {
        hook: mir::ReleaseHookTarget::Local(id),
    };
    let hook = &mut module.release_hooks[id];
    let field = hook
        .code
        .body
        .blocks
        .iter_mut()
        .flat_map(|(_, block)| &mut block.statements)
        .find_map(|statement| {
            let mir::StatementKind::ValDecl { init, .. } = &mut statement.kind else {
                return None;
            };
            let mir::ExprKind::ReleaseFieldLoad { index, .. } = &mut init.kind else {
                return None;
            };
            Some(index)
        })
        .unwrap();
    *field = u32::MAX;
    assert!(matches!(
        module.validate().unwrap_err().kind,
        mir::MirValidationErrorKind::InvalidRelease { .. }
    ));
}

#[test]
fn output_validation_rejects_early_publication_and_hook_unwind() {
    let mut module = lower(
        "class Owner(val handle: Int) { release { val saved = handle } } fun main() { val owner = Owner(7) }",
    );
    let (id, _) = module.release_hooks.iter().next().unwrap();
    let hook = &mut module.release_hooks[id];
    let entry = hook.code.body.entry;
    hook.code.body.blocks[entry].unwind = Some(entry);
    assert!(module.validate().is_err());
    module.release_hooks[id].code.body.blocks[entry].unwind = None;
    let block = module
        .functions
        .iter_mut()
        .flat_map(|(_, function)| function.body.blocks.iter_mut())
        .find_map(|(_, block)| {
            let index = block.statements.iter().position(|statement| {
                matches!(
                    statement.kind,
                    mir::StatementKind::PublishReleaseReady { .. }
                )
            })?;
            block.statements.swap(index, index - 1);
            Some(block)
        })
        .unwrap();
    assert!(block.statements.len() >= 3);
    assert!(matches!(
        module.validate().unwrap_err().kind,
        mir::MirValidationErrorKind::InvalidRelease { .. }
    ));
}
