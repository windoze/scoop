use super::*;

#[test]
fn ordinary_dependency_calls_emit_only_typed_external_uses() {
    assert_dependency_calls(
        scoop_identity::ConeCoordinate::new("tests", "dependency", "1.0.0")
            .unwrap()
            .identity()
            .unwrap(),
    );
}

#[test]
fn core_dependency_calls_share_managed_and_no_gc_emission() {
    assert_dependency_calls(scoop_identity::ConeIdentity::CORE);
}

#[test]
fn external_callables_reject_shared_bodies_and_local_ownership() {
    let mut module = values_module();
    let callable = dependency_external(ConeIdentity::CORE, "shared", scoop_lir::GcEffect::Managed);
    module.meta.external_callables.alloc(callable.clone());
    module.meta.external_callables.alloc(callable.clone());
    assert_eq!(
        validation::validate_module(&module).unwrap_err().0,
        format!("duplicate external callable body {}", callable.body())
    );

    module.meta.external_callables.clear();
    module.meta.external_callables.alloc(callable.clone());
    let scoop_identity::StrongCallableDefinitionOwner::Function(function) = callable.target()
    else {
        panic!("the fixture imports a source function")
    };
    module.functions[0].callable_body =
        scoop_lir::CallableBodyIdentity::for_function(function).unwrap();
    assert_eq!(
        validation::validate_module(&module).unwrap_err().0,
        format!(
            "external callable body {} is also defined locally",
            callable.body()
        )
    );

    module.meta.external_callables.clear();
    let local = dependency_external(module.cone, "localProvider", scoop_lir::GcEffect::NoGc);
    module.meta.external_callables.alloc(local.clone());
    assert_eq!(
        validation::validate_module(&module).unwrap_err().0,
        format!(
            "external callable {:?} names the current Cone as provider",
            local.target()
        )
    );
}

fn assert_dependency_calls(provider: scoop_identity::ConeIdentity) {
    let mut module = values_module();
    let managed = dependency_external(provider, "managedDependency", scoop_lir::GcEffect::Managed);
    let managed_symbol = managed.expected_symbol().symbol().to_string();
    let managed = module.meta.external_callables.alloc(managed);
    let no_gc = dependency_external(provider, "noGcDependency", scoop_lir::GcEffect::NoGc);
    let no_gc_symbol = no_gc.expected_symbol().symbol().to_string();
    let no_gc = module.meta.external_callables.alloc(no_gc);

    let function = &mut module.functions[0];
    let managed_call = void_site(
        &mut function.call_targets,
        TestCallProtocol::Managed {
            safepoint: 900,
            destination: scoop_lir::ManagedCallDestination::external(managed),
        },
        Vec::new(),
        Vec::new(),
    );
    let no_gc_call = void_site(
        &mut function.call_targets,
        TestCallProtocol::NoGc {
            destination: scoop_lir::NoGcCallDestination::external(no_gc),
        },
        Vec::new(),
        Vec::new(),
    );
    function.blocks[function.entry].instructions.extend([
        Instruction::Call { site: managed_call },
        Instruction::Call { site: no_gc_call },
    ]);
    refresh_module_safepoints(&mut module);

    let ir = rewritten_ir_of(&module);
    let managed_marker = format!("@\"{managed_symbol}\"");
    let no_gc_marker = format!("@\"{no_gc_symbol}\"");
    assert!(
        ir.lines()
            .any(|line| line.starts_with("declare void ") && line.contains(&managed_marker)),
        "managed dependency must remain an external declaration:\n{ir}"
    );
    assert!(
        ir.lines()
            .any(|line| line.starts_with("declare void ") && line.contains(&no_gc_marker)),
        "NoGc dependency must remain an external declaration:\n{ir}"
    );
    assert!(
        ir.lines().any(|line| {
            line.contains("llvm.experimental.gc.statepoint") && line.contains(&managed_marker)
        }),
        "managed dependency must use the statepoint path:\n{ir}"
    );
    assert!(
        ir.lines()
            .any(|line| line.contains("call void") && line.contains(&no_gc_marker)),
        "NoGc dependency must remain an ordinary typed call:\n{ir}"
    );
}

fn dependency_external(
    provider: scoop_identity::ConeIdentity,
    name: &str,
    effect: scoop_lir::GcEffect,
) -> scoop_lir::ExternalCallable {
    let site = scoop_identity::SourceDeclarationSite::new(
        provider,
        scoop_identity::PackagePath::root(),
        scoop_identity::DefinitionOwnerChain::top_level(),
        scoop_identity::DeclarationScope::ConeWide,
    )
    .unwrap();
    let source = scoop_identity::SourceDeclarationKey::function(
        site,
        scoop_identity::CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function = scoop_identity::PersistentFunctionId::from_source_declaration(&source).unwrap();
    let unit =
        scoop_identity::PersistentExactTypeId::from_key(&scoop_identity::ExactTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ))
        .unwrap();
    let exact = scoop_identity::ExactCallableSignature::new(
        scoop_identity::Effect::Ordinary,
        None,
        Vec::new(),
        unit,
    );
    let canonical = scoop_identity::CanonicalScoopAbiFunctionSignature::new(
        exact,
        Vec::new(),
        scoop_identity::ScoopAbiReturn::unit_void(),
        match effect {
            scoop_lir::GcEffect::Managed => scoop_identity::GcEffect::Managed,
            scoop_lir::GcEffect::NoGc => scoop_identity::GcEffect::NoGc,
        },
    )
    .unwrap();
    let root_plan = match effect {
        scoop_lir::GcEffect::Managed => {
            scoop_lir::DependencyExternalCallableRootPlanV1::ManagedStatepoint
        }
        scoop_lir::GcEffect::NoGc => scoop_lir::DependencyExternalCallableRootPlanV1::NoGc,
    };
    scoop_lir::SelectedDependencyLirCallableV1::new(
        provider,
        scoop_identity::DependencyCallableDeclarationId::Function(function),
        scoop_identity::StrongCallableDefinitionOwner::Function(function),
        canonical,
        scoop_lir::CallingConvention::Cdecl,
        root_plan,
    )
    .unwrap()
    .materialize(plain_scoop_signature(Vec::new(), LirType::Void))
    .unwrap()
}
