use super::*;

#[test]
fn template_and_local_origin_relations_are_independent_and_metered() {
    let case = Case::method(false, false, false);
    let template = case.template();
    let mut authority = Authority::new(&case, &template);
    template
        .validate_origin_semantics(&mut authority, &mut meter())
        .unwrap();
    authority.reject_origin = true;
    assert!(matches!(
        template.validate_origin_semantics(&mut authority, &mut meter()),
        Err(
            ProtectedDefaultTemplateOriginSemanticError::DefinitionRelation("wrong default origin")
        )
    ));
    authority.reject_origin = false;
    authority.reject_local = true;
    assert!(matches!(
        template.validate_origin_semantics(&mut authority, &mut meter()),
        Err(ProtectedDefaultTemplateOriginSemanticError::LocalRelation { .. })
    ));
    authority.reject_local = false;
    assert!(matches!(
        template.validate_origin_semantics(
            &mut authority,
            &mut BudgetMeter::new(DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            })
        ),
        Err(ProtectedDefaultTemplateOriginSemanticError::Resource(_))
    ));
    let source = case.origin.origin().source().clone();
    let changed = ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(
            source.clone(),
            SourceSpan::new(100, 102).unwrap(),
            &SourceContextKey::File { source },
        )
        .unwrap(),
    );
    let mut wrong = template.clone();
    wrong.definition_origin = changed.clone();
    assert!(matches!(
        wrong.validate_origin_semantics(&mut authority, &mut meter()),
        Err(ProtectedDefaultTemplateOriginSemanticError::DefinitionSource(_))
    ));
    wrong = template;
    wrong.locals = CanonicalTemplateLocalTableV1::try_new(
        wrong
            .locals()
            .records()
            .iter()
            .map(|record| {
                local(
                    record.selector().clone(),
                    record.value_type().clone(),
                    &changed,
                )
            })
            .collect(),
    )
    .unwrap();
    assert!(matches!(
        wrong.validate_origin_semantics(&mut authority, &mut meter()),
        Err(ProtectedDefaultTemplateOriginSemanticError::LocalSource { .. })
    ));
}
