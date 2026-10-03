use super::*;
use scoop_lir_lower::LirLoweringError as Error;

pub(super) fn check(
    input: &mir::ConeMirInput,
    callables: &lir::SelectedExternalLirSet,
    diagnostics: &ExactTypeDiagnosticCatalog<'_>,
    selected: &lir::StrongProductionDependencySelectionV2<'_>,
    source: &Source,
    provider: Provider<'_, '_>,
) {
    let lower = |selection: &lir::StrongProductionDependencySelectionV2<'_>| {
        scoop_lir_lower::lower_with_layout_dependencies(
            input,
            callables,
            provider.target.lir_target(),
            selection,
            diagnostics,
        )
    };
    let absent = Source {
        roots: vec![],
        physical: vec![],
    };
    for owner in [input.module().cone, provider.layout.provider()] {
        let empty = lir::StrongProductionDependencySelectionV2::try_new(
            owner,
            provider.target.lir_target(),
            &[],
            vec![],
            &absent.roots,
        )
        .unwrap();
        let error = lower(&empty).err().unwrap();
        if owner == input.module().cone {
            assert!(
                matches!(
                    error,
                    Error::DependencyLayout(
                        lir::LayoutExternalMaterializationError::MissingShapeSupport { .. }
                    )
                ),
                "{error}"
            );
        } else {
            assert!(
                matches!(error, Error::DependencyLayoutConsumer { .. }),
                "{error}"
            );
        }
    }
    let root = input
        .materialization()
        .dependency_generated_nominal_shapes()[0];
    let subject = lir::ExternalStrongShapeSubjectV1::TypeDescriptor(root.exact());
    let missing = Source {
        roots: source.roots.clone(),
        physical: source
            .physical
            .iter()
            .copied()
            .filter(|(_, item)| *item != subject)
            .collect(),
    };
    let imports = selected
        .physical_imports()
        .records()
        .iter()
        .filter(|import| import.subject() != subject)
        .cloned()
        .collect();
    let incomplete = lir::StrongProductionDependencySelectionV2::try_new(
        input.module().cone,
        provider.target.lir_target(),
        &[provider.layout],
        imports,
        &missing.roots,
    )
    .unwrap();
    assert!(matches!(lower(&incomplete), Err(Error::DependencyLayout(
        lir::LayoutExternalMaterializationError::MissingPhysicalImport { subject: found, .. }
    )) if found == subject));

    let descriptor_only = Source {
        roots: source
            .physical
            .iter()
            .filter_map(|(owner, subject)| {
                let lir::ExternalStrongShapeSubjectV1::TypeDescriptor(exact) = subject else {
                    return None;
                };
                Some(lir::LayoutAbiDependencyV1::new(
                    *owner,
                    lir::LayoutAbiSemanticTargetV1::Descriptor(*exact),
                ))
            })
            .collect(),
        physical: source.physical.clone(),
    };
    let missing_shape_support = lir::StrongProductionDependencySelectionV2::try_new(
        input.module().cone,
        provider.target.lir_target(),
        &[provider.layout],
        selected.physical_imports().records().to_vec(),
        &descriptor_only.roots,
    )
    .unwrap();
    assert!(matches!(
        lower(&missing_shape_support),
        Err(Error::DependencyLayout(
            lir::LayoutExternalMaterializationError::MissingShapeSupport { .. }
        ))
    ));
    assert!(matches!(
        selected
            .materialize_shape_type_descriptor(root.provider(), root.source(), provider.string,),
        Err(lir::LayoutExternalMaterializationError::UnavailableShapeDescriptor { .. })
    ));
}
