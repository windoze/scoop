use super::*;

#[derive(Debug)]
pub(super) struct Outcome {
    pub metadata: bool,
    pub provider_calls: usize,
    pub origin_calls: usize,
    pub body_calls: usize,
    pub metadata_calls: usize,
    pub expression_calls: usize,
    pub concrete_calls: usize,
    pub nested_calls: usize,
}
pub(super) fn run(case: Case) -> Result<Outcome, ProtectedDefaultTableSemanticError<&'static str>> {
    run_with_limits(case)
}
pub(super) fn run_with_limits(
    case: Case,
) -> Result<Outcome, ProtectedDefaultTableSemanticError<&'static str>> {
    let fixture = Fixture::new(case);
    let graph_source = fixture.source.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate_with_source_roots(
        graph_source.records.values(),
        graph_source.keys.keys().copied(),
        &graph_source,
    )
    .unwrap();
    let mut source = fixture.source.clone();
    let representation = CanonicalNominalRepresentationSupportV1::default();
    let protected = fixture
        .protected
        .validate_sources(&graph, &representation, &mut source)
        .unwrap();
    let inheritance = fixture
        .inheritance
        .validate_interfaces(&graph, protected, &mut source)
        .unwrap();
    let template = template::build(&fixture, case, 1);
    let complete = CanonicalProtectedDefaultTemplatesV1::try_new(vec![template.clone()]).unwrap();
    let sources = fixture
        .protocols
        .validate_protocols(
            protected,
            inheritance,
            &graph,
            complete.keys(),
            &mut source,
            &mut protocol::Authority::new(&fixture),
        )
        .unwrap();
    let defaults = match case {
        Case::MissingDefault => CanonicalProtectedDefaultTemplatesV1::try_new(vec![]).unwrap(),
        Case::ExtraDefault => CanonicalProtectedDefaultTemplatesV1::try_new(vec![
            template.clone(),
            template::build(&fixture, Case::Valid, 2),
        ])
        .unwrap(),
        _ => complete,
    };
    let path = WirePath::root();

    let actual = defaults.get(fixture.key).unwrap_or(&template);
    let mut authority = Authority::new(&fixture, actual, &path, case);
    let checked = defaults.validate_semantics(
        &sources,
        &graph,
        inheritance,
        &mut source,
        &mut authority,
        &path,
    )?;
    assert!(std::ptr::eq(checked.table(), &defaults));
    assert_eq!(checked.records().len(), 1);
    let record = checked.get(fixture.key).unwrap();
    assert_eq!(record.key(), fixture.key);
    assert!(std::ptr::eq(record.template(), actual));
    assert_eq!(record.source().owner(), fixture.key.owner());
    assert_eq!(record.source().payload().owner(), fixture.owner);
    assert!(std::ptr::eq(
        record.source().protocol().record(),
        fixture.protocols.get(fixture.key.owner()).unwrap()
    ));
    assert!(
        checked
            .get(ProtectedDefaultTemplateKeyV1::try_new(fixture.key.owner(), 2).unwrap())
            .is_none()
    );
    let metadata = match record {
        CheckedProtectedDefaultTemplateV1::ParamFree(record) => {
            assert!(std::ptr::eq(record.template(), actual));
            assert_eq!(record.source().owner(), fixture.key.owner());
            false
        }
        CheckedProtectedDefaultTemplateV1::GenericSourceMetadata(record) => {
            assert!(std::ptr::eq(record.template(), actual));
            assert_eq!(record.source().owner(), fixture.key.owner());
            true
        }
    };
    Ok(Outcome {
        metadata,
        provider_calls: authority.provider_calls,
        origin_calls: authority.origin_calls,
        body_calls: authority.body_calls,
        metadata_calls: authority.metadata_calls,
        expression_calls: authority.expression_calls,
        concrete_calls: authority.concrete_calls,
        nested_calls: authority.nested_calls,
    })
}
