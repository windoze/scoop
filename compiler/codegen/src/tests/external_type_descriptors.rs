use super::*;
use scoop_identity::ConeCoordinate;
use scoop_lir::ExternalTypeDescriptor;

#[test]
fn emits_runtime_string_and_ordinary_descriptors_as_shared_external_declarations() {
    let mut module = values_module();
    module.meta.type_descriptors.clear();
    let string = module
        .meta
        .external_type_descriptors
        .alloc(descriptor(ConeIdentity::CORE, "String"));
    module.meta.well_known_type_descriptors.string = TypeDescriptorRef::External(string);
    module
        .meta
        .external_type_descriptors
        .alloc(descriptor(ConeIdentity::CORE, "CoreExtension"));
    module
        .meta
        .external_type_descriptors
        .alloc(descriptor(ordinary_provider(), "LibraryClass"));

    let machine = host_target_machine().unwrap();
    let context = Context::create();
    let llvm = emit_llvm_module(&context, &module, &machine, host_profile()).unwrap();
    llvm.verify().unwrap();
    for (_, descriptor) in module.meta.external_type_descriptors.iter() {
        let symbol = descriptor.expected_symbol().symbol();
        let global = llvm
            .get_global(symbol.as_str())
            .expect("external descriptor is declared");
        assert!(
            global.get_initializer().is_none(),
            "{symbol} must stay external"
        );
        assert_eq!(global.get_linkage(), inkwell::module::Linkage::External);
    }
    let string_symbol = module.meta.external_type_descriptors[string]
        .expected_symbol()
        .symbol();
    let ir = llvm.print_to_string().to_string();
    assert!(
        ir.matches(string_symbol.as_str()).count() >= 3,
        "immortal strings must use the selected descriptor: {ir}"
    );
}

fn ordinary_provider() -> ConeIdentity {
    ConeCoordinate::new("test", "descriptor-provider", "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}

fn descriptor(provider: ConeIdentity, name: &str) -> ExternalTypeDescriptor {
    let source = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    );
    let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        PersistentTypeId::from_source_declaration(&source).unwrap(),
    ))
    .unwrap();
    ExternalTypeDescriptor::new(provider, exact).unwrap()
}
