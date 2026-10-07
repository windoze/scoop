use super::support::*;
use super::*;
use crate::*;

fn definition(module: &Module, foundation: &ConeLirFoundation) -> CanonicalCallableAbiV1 {
    CanonicalCallableAbisV1::from_module(module, foundation)
        .unwrap()
        .definitions()[0]
}

fn abi(definition: CanonicalCallableAbiV1) -> Digest256 {
    let CanonicalCallableAbiOwnerV1::Odr { abi, .. } = definition.owner() else {
        panic!("expected ODR callable ABI")
    };
    abi
}

#[test]
fn odr_callable_abi_keeps_the_actual_role_and_ignores_body_changes() {
    for role in [OdrMemberRole::CallableBody, OdrMemberRole::DispatchAdapter] {
        let mut module = scalar(false);
        let foundation = odr_foundation(&mut module, role);
        let original = definition(&module, &foundation);
        assert!(
            matches!(original.owner(), CanonicalCallableAbiOwnerV1::Odr { role: actual, .. } if actual == role)
        );

        let mut shuffled = scalar(true);
        let shuffled_foundation = odr_foundation(&mut shuffled, role);
        shuffled.cone = scoop_identity::ConeIdentity::CORE;
        assert_eq!(original, definition(&shuffled, &shuffled_foundation));

        let function = &mut module.functions[0];
        let Instruction::IntegerBinary { rhs, .. } =
            &mut function.blocks[function.entry].instructions[3]
        else {
            panic!("product instruction")
        };
        *rhs = Value::IntegerConst(LirIntegerConstant::Signed64(8));
        let changed = definition(&module, &foundation);
        assert_eq!(original.owner(), changed.owner());
        assert_eq!(original, changed);
    }
}

#[test]
fn odr_callable_abi_retains_gc_argument_return_and_zst_layout() {
    let mut module = scalar(false);
    let foundation = odr_foundation(&mut module, OdrMemberRole::CallableBody);
    let original = abi(definition(&module, &foundation));
    module.functions[0].gc_effect = GcEffect::Managed;
    assert_ne!(original, abi(definition(&module, &foundation)));
    module.functions[0].gc_effect = GcEffect::NoGc;

    let signature = module.functions[0].signature.clone();
    module.functions[0].signature = ScoopAbiSignature::new(
        signature.arguments().to_vec(),
        AbiReturn::Indirect(value(LirType::I64, 8, RefScan::None)),
        signature.calling_convention(),
    );
    assert_ne!(original, abi(definition(&module, &foundation)));

    let mut previous = original;
    for alignment in [1, 16] {
        let mut arguments = signature.arguments().to_vec();
        arguments.push(AbiArgument::ElidedZst(
            AbiZst::new(
                LirType::Aggregate(Vec::new()),
                AbiZeroSizedLayout::new(alignment).unwrap(),
            )
            .unwrap(),
        ));
        module.functions[0].signature = ScoopAbiSignature::new(
            arguments,
            signature.result().clone(),
            signature.calling_convention(),
        );
        let current = abi(definition(&module, &foundation));
        assert_ne!(previous, current);
        previous = current;
    }
}

#[test]
fn odr_callable_abi_retains_nominal_identity_and_reference_scan() {
    let mut module = scalar(false);
    let foundation = odr_foundation(&mut module, OdrMemberRole::CallableBody);
    let signature = module.functions[0].signature.clone();
    let mut fingerprints = std::collections::BTreeSet::new();
    for name in ["Left", "Right"] {
        let exact =
            crate::tests::test_physical_exact(name, scoop_identity::SourceNominalKind::Struct);
        let id =
            module
                .structs
                .alloc_scoop(exact, "same name".to_string(), 16, 8, false, Vec::new());
        for scan in [
            RefScan::None,
            RefScan::References(vec![0]),
            RefScan::References(vec![8]),
        ] {
            let mut arguments = signature.arguments().to_vec();
            arguments.push(AbiArgument::Indirect(
                AbiValue::new(
                    LirType::Struct(id),
                    AbiNonZeroLayout::new(16, 8).unwrap(),
                    scan,
                )
                .unwrap(),
            ));
            module.functions[0].signature = ScoopAbiSignature::new(
                arguments,
                signature.result().clone(),
                signature.calling_convention(),
            );
            assert!(fingerprints.insert(abi(definition(&module, &foundation)).to_string()));
        }
    }
}

#[test]
fn canonical_callable_owner_must_match_the_existing_member_key() {
    let mut module = scalar(false);
    let foundation = odr_foundation(&mut module, OdrMemberRole::CallableBody);
    let original = definition(&module, &foundation);
    let CanonicalCallableAbiOwnerV1::Odr {
        group,
        member,
        role,
        abi,
    } = original.owner()
    else {
        panic!("ODR member")
    };
    for owner in [
        CanonicalCallableAbiOwnerV1::Strong,
        CanonicalCallableAbiOwnerV1::Odr {
            group,
            member,
            role: OdrMemberRole::DispatchAdapter,
            abi,
        },
    ] {
        let changed = CanonicalCallableAbiV1::new(original.body(), owner);
        assert!(
            matches!(CanonicalCallableAbisV1::new(vec![changed], &foundation), Err(CanonicalCallableAbiError::DefinitionOwner { body }) if body == original.body())
        );
    }
    assert_eq!(role, OdrMemberRole::CallableBody);
    // The source member belongs to an earlier IR layer. Its resolved body
    // reference retains the group/role without another local member record.
    assert_eq!(foundation.as_canonical().counts().odr_members, 0);
    assert_eq!(definition(&module, &foundation), original);
}
