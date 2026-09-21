use super::*;

#[test]
fn ordinary_source_demands_use_the_same_complete_lir_materialization_checks() {
    let provider = ConeIdentity::SINGLE_FILE;
    let value = source_declaration(provider, "Value", SourceNominalKind::Struct, 0);
    let reference = source_declaration(provider, "Reference", SourceNominalKind::Class, 0);
    let mut module = fixture_module(provider);
    add_shape_support(&mut module, &value, true);
    add_shape_support(&mut module, &reference, false);
    let mut sources = vec![value.clone(), reference];
    sources.sort_by_key(source_nominal);
    let plan = StrongLirShapeSupportPlan::from_module(&module, sources.clone()).unwrap();
    assert_eq!(plan.roots().len(), 2);
    for root in plan.roots() {
        assert_eq!(root.declaration().origin(), provider);
        assert_eq!(root.source().exact(), source_exact(root.declaration()));
    }
    assert!(matches!(
        StrongLirShapeSupportPlan::from_module(&module, vec![value.clone(), value.clone()],),
        Err(StrongLirShapeSupportError::NonCanonicalSources { index: 1, .. })
    ));
    let missing = source_exact(&value);
    let descriptors = std::mem::take(&mut module.meta.type_descriptors);
    for (_, descriptor) in descriptors {
        if descriptor.identity.exact_type() != missing {
            module.meta.type_descriptors.alloc(descriptor);
        }
    }
    assert!(
        matches!(StrongLirShapeSupportPlan::from_module(&module, sources),
            Err(StrongLirShapeSupportError::TypeDescriptorSet { exact, actual: 0 }) if exact == missing
        )
    );
}
