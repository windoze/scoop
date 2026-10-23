use super::*;

mod projection;
mod storage;

fn fixture() -> (mir::Module, mir::FunctionId) {
    fixture_type(false)
}

fn fixture_type(child_source: bool) -> (mir::Module, mir::FunctionId) {
    let mut b = Builder::new();
    let parent = b.interface("Parent", &["read"]);
    let other = b.interface("Other", &["read"]);
    let child = b.interface("Child", &["own"]);
    b.interfaces[child].parents = vec![parent, other];
    let mut declarations = Vec::new();
    for (name, interface) in [("Parent.read", parent), ("Other.read", other)] {
        let ty = mir::Type::Interface(interface);
        let mut locals = Arena::new();
        let this = locals.alloc(local("this", ty.clone()));
        declarations.push(b.user_fn_body(
            name,
            vec![param("this", ty, this)],
            mir::Type::Unit,
            returning_body(locals, mir::Expr::unit()),
        ));
    }
    let ty = if child_source {
        mir::Type::Interface(child)
    } else {
        mir::Type::Any
    };
    let mut locals = Arena::new();
    let input = locals.alloc(local("input", ty.clone()));
    b.user_fn_body(
        "replace",
        vec![param("input", ty.clone(), input)],
        ty.clone(),
        returning_body(locals, local_expr(input, ty.clone())),
    );
    let mut locals = Arena::new();
    let source = locals.alloc(local("source", ty.clone()));
    let replacement = locals.alloc(local("replacement", ty.clone()));
    let current = locals.alloc(local("current", ty.clone()));
    locals[current].mutable = true;
    let call = mir::Call {
        target: mir::CallTarget::Interface {
            interface: parent,
            slot: 0,
            callee: mir::DispatchCallee::User(declarations[0]),
            receiver: Box::new(retype(
                local_expr(current, ty.clone()),
                mir::Type::Interface(parent),
            )),
        },
        args: Vec::new(),
    };
    let statements = vec![
        val_decl(current, local_expr(source, ty.clone())),
        stmt(mir::StatementKind::Call(mir::CallEffect::Unit(
            call.clone(),
        ))),
        stmt(mir::StatementKind::Call(mir::CallEffect::Unit(call))),
    ];
    let scope = b.user_fn_body(
        "scope",
        vec![
            param("source", ty.clone(), source),
            param("replacement", ty, replacement),
        ],
        mir::Type::Unit,
        body_with_terminator(
            locals,
            statements,
            mir::Terminator::Return {
                value: mir::Expr::unit(),
            },
        ),
    );
    let main = b.main(Arena::new(), Vec::new());
    (b.finish(main), scope)
}

fn retype(value: mir::Expr, ty: mir::Type) -> mir::Expr {
    mir::Expr::new(
        ty.clone(),
        mir::ExprKind::Retype {
            operand: Box::new(value),
            ty: Box::new(ty),
        },
    )
}

fn physical(module: mir::Module, optimization: lir::OptimizationMode) -> lir::Module {
    let input = seal_strong_input(module);
    lower_test_input_with_optimization(&input, optimization)
        .unwrap()
        .into_module()
}

fn function(module: &lir::Module, id: mir::FunctionId) -> &lir::Function {
    &module.functions[id.into_raw().into_u32() as usize]
}

fn lookup_count(function: &lir::Function) -> usize {
    function
        .blocks
        .iter()
        .flat_map(|(_, block)| &block.instructions)
        .filter(|instruction| {
            matches!(instruction, lir::Instruction::Call { site }
            if site.destination(&function.call_targets) == lir::CallDestination::Runtime(
                lir::RuntimeFunction::NoGc(lir::NoGcRuntimeFunction::ITableLookup)))
        })
        .count()
}

fn current(function: &mir::Function) -> mir::LocalId {
    function
        .body
        .locals
        .iter()
        .find(|(_, local)| local.name == "current")
        .unwrap()
        .0
}

#[test]
fn cached_metadata_removes_a_lookup_without_an_extra_managed_root() {
    for mode in [lir::OptimizationMode::Debug, lir::OptimizationMode::Release] {
        let (module, scope) = fixture();
        let lowered = physical(module, mode);
        let function = function(&lowered, scope);
        assert_eq!(
            lookup_count(function),
            if mode == lir::OptimizationMode::Release {
                1
            } else {
                2
            }
        );
        assert_eq!(function.locals.len(), 1);
        let roots = function
            .blocks
            .iter()
            .flat_map(|(_, block)| &block.instructions)
            .filter_map(|instruction| match instruction {
                lir::Instruction::Call {
                    site: lir::CallSite::Managed(site),
                } => Some(site.live.as_slice().len()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(roots, vec![2, 1]);
    }
}

#[test]
fn different_exact_interfaces_keep_distinct_tables() {
    let (mut module, scope) = fixture();
    let other = module
        .interfaces
        .iter()
        .find(|(_, interface)| interface.name == "Other")
        .unwrap()
        .0;
    let method = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "Other.read")
        .unwrap()
        .0;
    let function = &mut module.functions[scope];
    let source = current(function);
    let call = mir::Call {
        target: mir::CallTarget::Interface {
            interface: other,
            slot: 0,
            callee: mir::DispatchCallee::User(method),
            receiver: Box::new(retype(
                local_expr(source, function.body.locals[source].ty.clone()),
                mir::Type::Interface(other),
            )),
        },
        args: Vec::new(),
    };
    function.body.blocks[function.body.entry]
        .statements
        .push(stmt(mir::StatementKind::Call(mir::CallEffect::Unit(call))));
    let lowered = physical(module, lir::OptimizationMode::Release);
    assert_eq!(lookup_count(self::function(&lowered, scope)), 2);
}
