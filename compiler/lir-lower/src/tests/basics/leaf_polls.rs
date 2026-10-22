use super::*;

fn identity(ty: mir::Type, copies: usize) -> lir::Module {
    let mut builder = Builder::new();
    let mut locals = Arena::new();
    let value = locals.alloc(local("value", ty.clone()));
    let parameters = vec![mir::Param {
        name: "value".into(),
        ty: ty.clone(),
        local: value,
    }];
    let statements = (0..copies)
        .map(|i| {
            let copy = locals.alloc(local(&format!("copy{i}"), ty.clone()));
            val_decl(copy, local_expr(value, ty.clone()))
        })
        .collect();
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let returned = cfg_block(&mut blocks, "inline.cont");
    let inlined = cfg_block(&mut blocks, "inline.entry");
    set_cfg_block(
        &mut blocks,
        entry,
        Vec::new(),
        mir::Terminator::Goto(inlined),
        None,
    );
    set_cfg_block(
        &mut blocks,
        inlined,
        Vec::new(),
        mir::Terminator::Goto(returned),
        None,
    );
    set_cfg_block(
        &mut blocks,
        returned,
        statements,
        mir::Terminator::Return {
            value: local_expr(value, ty.clone()),
        },
        None,
    );
    let function = builder.user_fn_body(
        "identity",
        parameters,
        ty,
        mir::Body {
            locals,
            blocks,
            entry,
            loop_header_polls: Vec::new(),
        },
    );
    lower(builder.finish(function))
}

fn entry_poll(function: &lir::Function) -> &lir::ManagedPollSite {
    let lir::Instruction::ManagedPoll { site } = &function.blocks[function.entry].instructions[0]
    else {
        panic!("Managed functions retain their entry handshake");
    };
    site
}

#[test]
fn bounded_leaf_retains_activation_and_live_reference_roots() {
    let module = identity(mir::Type::String, 0);
    let site = entry_poll(&module.functions[0]);
    assert_eq!(site.kind, lir::ManagedPollKind::EntryActivation);
    assert_eq!(site.live.as_slice().len(), 1);
    assert_eq!(
        site.live.as_slice()[0].source,
        lir::CallerRootSource::Param(0)
    );
    lir::CanonicalLirFoundation::from_module(&module).unwrap();
}

#[test]
fn large_copies_and_long_bodies_retain_full_polls() {
    for module in [
        identity(mir::Type::Tuple(vec![mir::Type::String; 9]), 1),
        identity(mir::Type::String, 65),
        lower(hello_world()),
    ] {
        assert_eq!(
            entry_poll(&module.functions[0]).kind,
            lir::ManagedPollKind::Full
        );
    }
}
