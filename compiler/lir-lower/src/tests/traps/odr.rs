use super::*;
use scoop_identity::{
    CallableApplicationKey, CallableInstantiationOwner, ConeCoordinate, DefinitionAtomRole,
    LinkageClass, ObjectDefinitionPlanId, OdrGroupId, PersistentCallableApplicationId,
    PersistentGenericFunctionId,
};

#[test]
fn odr_trap_support_keeps_its_body_owner_across_producers() {
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("abstractGenericBody").unwrap(),
        1,
        None,
        Vec::new(),
    );
    let origin = PersistentGenericFunctionId::from_source_declaration(&declaration).unwrap();
    let application = CallableApplicationKey::for_generic_function(
        origin,
        CallableInstantiationOwner::NoOwner,
        NonEmptyVec::from_first(mir::core_unit_exact_type(), []),
    );
    let materialization = CallableMaterialization::new(
        CallableTemplateOwner::GenericFunction(origin),
        CallableMaterializationContext::Application(
            PersistentCallableApplicationId::from_key(&application).unwrap(),
        ),
    );
    let group = OdrGroupId::from_key(&SpecializationKey::Callable { application }).unwrap();
    let mut constants = Vec::new();
    for name in ["first", "second"] {
        let producer = ConeCoordinate::new("test", name, "1.0.0")
            .unwrap()
            .identity()
            .unwrap();
        let mut builder = Builder::new();
        let message = String::from("abstract callable cannot execute");
        let mut body = mir::Body::unreachable(Arena::new());
        body.blocks[body.entry].terminator = mir::Terminator::Trap { message };
        let function = builder.user_fn_body("abstract", Vec::new(), mir::Type::Unit, body);
        let main = builder.main(Arena::new(), Vec::new());
        let mut module = builder.finish_with_types(main, producer, Vec::new());
        module.output = mir::MirOutput::Library;
        let source = mir::SourceCallableMaterialization::new(
            function,
            materialization,
            module
                .meta
                .source_callable_materializations
                .get(function)
                .unwrap()
                .signature_record()
                .signature()
                .clone(),
            Some(group),
        )
        .unwrap();
        module.meta.source_callable_materializations =
            mir::SourceCallableMaterializations::checked(
                module
                    .meta
                    .source_callable_materializations
                    .iter()
                    .map(|record| {
                        if record.function() == function {
                            source.clone()
                        } else {
                            record.clone()
                        }
                    })
                    .collect(),
            )
            .unwrap();
        module.meta.callable_signatures = mir::MirCallableSignatures::checked(
            module
                .meta
                .source_callable_materializations
                .iter()
                .map(|record| record.signature_record().clone())
                .collect(),
        )
        .unwrap();
        let output = try_lower(module).unwrap();
        let module = output.module();
        assert!(
            module
                .globals
                .iter()
                .all(|(_, global)| { !matches!(global.init, lir::GlobalInit::StringConst { .. }) })
        );
        let owner = &module.functions[0].callable_body;
        assert_eq!(owner.symbol_request().linkage(), LinkageClass::OdrWeak);
        let plan = ObjectDefinitionPlanId::from_key(&owner.definition_plan_key(producer)).unwrap();
        let (_, global) = module
            .globals
            .iter()
            .find(|(_, global)| matches!(global.init, lir::GlobalInit::CString { .. }))
            .unwrap();
        let lir::GlobalInit::CString { identity, value } = &global.init else {
            unreachable!()
        };
        assert_eq!(identity.owner(), owner.id());
        assert_eq!(identity.atom_record().key().plan(), plan);
        assert_eq!(
            identity.atom_record().key().role(),
            DefinitionAtomRole::AddressTakenConstant
        );
        assert_eq!(identity.symbol_request().linkage(), LinkageClass::OdrWeak);
        constants.push((
            identity.atom_record().clone(),
            identity.symbol_request(),
            value.clone(),
        ));
    }
    assert_eq!(constants[0], constants[1]);
}
