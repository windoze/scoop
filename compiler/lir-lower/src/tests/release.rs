use super::*;

fn module() -> lir::Module {
    let mut builder = Builder::new();
    let owner = builder.class("Owner", None, &[("handle", INT)], Vec::new(), Vec::new());
    let main = builder.main(Arena::new(), Vec::new());
    let mut module = builder.finish(main);
    let exact = module
        .meta
        .source_exact_types
        .get(&mir::Type::Class(owner))
        .unwrap()
        .identity_record()
        .id();
    let hook = module.release_hooks.alloc(mir::ReleaseHook {
        owner,
        materialization: CallableMaterialization::new(
            CallableTemplateOwner::ReleaseHook(exact),
            CallableMaterializationContext::NoSubstitution,
        ),
        code: mir::Function {
            gc_effect: mir::GcEffect::NoGc,
            name: "Owner::release".into(),
            params: Vec::new(),
            return_ty: mir::Type::Unit,
            body: body_with_terminator(
                Arena::new(),
                vec![stmt(mir::StatementKind::Expr(expr(
                    INT,
                    mir::ExprKind::ReleaseFieldLoad {
                        class: owner,
                        index: 0,
                    },
                )))],
                mir::Terminator::Return { value: None },
            ),
        },
    });
    module.classes[owner].release_policy = mir::ReleasePolicy::SynchronousGcFree {
        hook: mir::ReleaseHookTarget::Local(hook),
    };
    lower(module)
}

fn hook(module: &mut lir::Module) -> &mut lir::Function {
    &mut module.release_hooks.iter_mut().next().unwrap().1.code
}

#[test]
fn release_body_has_private_raw_abi_and_no_safepoints() {
    let module = module();
    let (_, hook) = module.release_hooks.iter().next().unwrap();
    assert_eq!(
        hook.code.signature.arguments()[0].logical_storage_type(),
        &lir::RAW_PTR
    );
    assert_eq!(hook.code.gc_effect, lir::GcEffect::NoGc);
    assert!(hook.code.safepoints.is_empty());
    assert!(matches!(
        hook.code.blocks[hook.code.entry].instructions[0],
        lir::Instruction::ReleaseFieldLoad { offset: 16, .. }
    ));
    let lir::TypeDescriptorRef::Local(owner) = hook.owner else {
        panic!("the source owner is local");
    };
    assert_eq!(
        module.meta.type_descriptors[owner].release_policy,
        lir::ReleasePolicy::SynchronousGcFree {
            hook: hook.code.callable_body.id()
        }
    );
}

#[test]
fn release_output_rejects_receiver_escape_and_managed_temporaries() {
    let mut escaped = module();
    let code = hook(&mut escaped);
    let out = code.temps.alloc(lir::Temp {
        ty: lir::LirType::I64,
    });
    code.blocks[code.entry]
        .instructions
        .push(lir::Instruction::PtrToULong {
            out,
            value: lir::Value::Param(0),
        });
    assert!(safepoints::validate_release_bodies(&escaped).is_err());

    let mut managed = module();
    hook(&mut managed).temps.alloc(lir::Temp {
        ty: lir::MANAGED_PTR,
    });
    assert!(safepoints::validate_release_bodies(&managed).is_err());
}

#[test]
fn release_output_rejects_unwind_and_managed_storage_operations() {
    let mut unwinding = module();
    let code = hook(&mut unwinding);
    code.blocks[code.entry].terminator = lir::Terminator::Resume {
        exception: lir::Value::Param(0),
    };
    assert!(safepoints::validate_release_bodies(&unwinding).is_err());

    let mut publishing = module();
    let code = hook(&mut publishing);
    code.blocks[code.entry]
        .instructions
        .push(lir::Instruction::PublishReleaseReady {
            object: lir::Value::Param(0),
        });
    assert!(safepoints::validate_release_bodies(&publishing).is_err());
}
