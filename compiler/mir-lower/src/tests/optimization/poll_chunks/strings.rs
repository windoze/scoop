use super::*;

#[test]
fn copied_runtime_read_defines_the_copied_temporary() {
    let (mut module, id) = sample(hir::IntegerKind::SIGNED_64, false);
    let owner = module
        .meta
        .source_callable_materializations
        .get(id)
        .unwrap()
        .materialization();
    let string = module.immortals.alloc(mir::ImmortalConst {
        identity: mir::ImmortalObjectKey::string_constant(
            mir::ImmortalObjectOwner::Callable(owner),
            mir::StructuralDefinitionPath::from_first(
                mir::StructuralPathSegment::new(
                    mir::StructuralDefinitionSiteRole::StringConstant,
                    0,
                ),
                [],
            ),
        ),
        value: mir::ImmortalConstValue::String("text".into()),
        source: mir::ImmortalConstSource::CurrentCone,
    });
    let body = &mut module.functions[id].body;
    let header = body.loop_header_polls[0].header();
    let mir::Terminator::Branch { then_block, .. } = body.blocks[header].terminator else {
        unreachable!()
    };
    let mut read = body.blocks[then_block].statements[0].clone();
    let mir::StatementKind::ValDecl { local, .. } = read.kind else {
        unreachable!()
    };
    // The next iteration overwrites saved before using it for induction.
    read.kind = mir::StatementKind::Call(mir::CallEffect::Value {
        destination: local,
        call: mir::Call {
            target: mir::CallTarget::Direct(mir::Callee::Runtime(mir::RuntimeFn::StringByteLength)),
            args: vec![mir::Expr::new(
                mir::Type::String,
                mir::ExprKind::StringConst(string),
            )],
        },
    });
    body.blocks[then_block].statements.push(read);
    chunk(&mut module);
    let destinations = module.functions[id]
        .body
        .blocks
        .iter()
        .flat_map(|(_, block)| &block.statements)
        .filter_map(|statement| match statement.kind {
            mir::StatementKind::Call(mir::CallEffect::Value { destination, .. }) => {
                Some(destination)
            }
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(destinations.len(), 2);
    assert!(destinations.contains(&local));
}
