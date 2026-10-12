//! Lowering tests exercise the production backend planner without LLVM.

use super::*;

pub(crate) fn classify_signature(
    context: &LoweringContext,
    parameters: impl IntoIterator<Item = lir::LirType>,
    result: Option<lir::LirType>,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> StorageResult<lir::ScoopAbiSignature> {
    let mut builder = prepare::SignatureBuilder::new(context, structs, enums);
    let request = builder.scoop(parameters, result)?;
    Ok(scoop_abi_plan::plan(&builder.input)?.scoop(request).clone())
}

pub(crate) fn classify_argument(
    context: &LoweringContext,
    ty: lir::LirType,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> StorageResult<lir::AbiArgument> {
    Ok(classify_signature(context, [ty], None, structs, enums)?.arguments()[0].clone())
}

#[test]
fn large_tagged_payload_remains_one_shared_storage_node() {
    let context = LoweringContext::new(lir::LirTargetProfile::DARWIN_AARCH64);
    let mut enums = lir::EnumDefs::default();
    let id = enums.alloc(lir::EnumDef {
        exact_type: crate::tests::test_physical_exact(
            "LargePayload",
            scoop_identity::SourceNominalKind::Enum,
        ),
        name: "LargePayload".to_string(),
        repr: lir::EnumRepr::Tagged {
            variants: vec![],
            size: 1 << 30,
            align: 8,
        },
        scan: lir::RefScan::None,
    });
    let structs = lir::StructDefs::default();
    let mut builder = prepare::SignatureBuilder::new(&context, &structs, &enums);
    let ty = lir::LirType::Enum(id);
    let request = builder.scoop([ty.clone(), ty.clone()], Some(ty)).unwrap();
    assert_eq!(builder.input.values.len(), 1);
    assert!(matches!(
        builder.input.values.iter().next().unwrap().1,
        lir::AbiPlanningValue::IntegerStorage(_)
    ));
    let plans = scoop_abi_plan::plan(&builder.input).unwrap();
    assert!(
        plans
            .scoop(request)
            .arguments()
            .iter()
            .all(|arg| matches!(arg, lir::AbiArgument::Indirect(_)))
    );
    assert!(matches!(
        plans.scoop(request).result(),
        lir::AbiReturn::Indirect(_)
    ));
}
