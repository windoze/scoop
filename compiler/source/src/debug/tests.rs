use super::*;

fn source_file() -> Arc<DebugFile> {
    Arc::new(DebugFile {
        identity: Arc::new(SourceIdentity::single_file()),
        path: "/workspace/main.scoop".into(),
    })
}

fn location(origin: &DebugOrigin) -> &Arc<DebugLocation> {
    let DebugOrigin::Source(location) = origin else {
        panic!("the test requested a source location")
    };
    location
}

#[test]
fn records_share_source_facts_and_survive_the_builder() {
    let source = source_file();
    let mut records = DebugRecords::default();
    let first = records.source(source.clone(), 4, 7);
    let repeated = records.source(Arc::new((*source).clone()), 4, 7);
    let different_line = records.source(source, 5, 3);
    assert!(Arc::ptr_eq(location(&first), location(&repeated)));
    assert!(Arc::ptr_eq(
        &location(&first).file,
        &location(&different_line).file,
    ));
    drop(records);
    assert_eq!((location(&first).line, location(&first).column), (4, 7));
    assert_eq!(location(&different_line).line, 5);
}

#[test]
fn nested_inlining_keeps_each_call_site_and_shares_repeated_locations() {
    let source = source_file();
    let mut records = DebugRecords::default();
    let definition = records.source(source.clone(), 2, 1);
    let operation = records.source(source.clone(), 4, 7);
    let inner_call = records.source(source.clone(), 10, 3);
    let outer_call = records.source(source, 20, 5);
    let inner = records.inline_site("inner", definition.clone(), inner_call);
    let outer = records.inline_site("outer", definition, outer_call);
    let expanded = records.inlined(&operation, &inner);
    let nested = records.inlined(&expanded, &outer);
    let repeated = records.inlined(&expanded, &outer);
    assert!(Arc::ptr_eq(location(&nested), location(&repeated)));
    let inner = location(&nested).inlined_at.as_ref().unwrap();
    assert_eq!(inner.function.as_ref(), "inner");
    assert_eq!(location(&inner.call_site).line, 10);
    let outer = location(&inner.call_site).inlined_at.as_ref().unwrap();
    assert_eq!(outer.function.as_ref(), "outer");
    assert_eq!(location(&outer.call_site).line, 20);
    assert!(location(&outer.call_site).inlined_at.is_none());
    assert!(matches!(
        records.inlined(&DebugOrigin::Generated, outer),
        DebugOrigin::Generated
    ));
}

#[test]
fn lexical_scopes_share_parents_and_survive_inline_locations() {
    let mut records = DebugRecords::default();
    let outer = records.source(source_file(), 2, 5);
    let inner = records.source(source_file(), 4, 9);
    let parent = records.scope(None, &outer).unwrap();
    let child = records.scope(Some(parent.clone()), &inner).unwrap();
    let repeated = records.scope(Some(parent.clone()), &inner).unwrap();
    assert!(Arc::ptr_eq(&child, &repeated));
    assert!(Arc::ptr_eq(child.parent.as_ref().unwrap(), &parent));
    let scoped = records.scoped(&inner, Some(child.clone()));
    let site = records.inline_site("callee", outer.clone(), outer);
    let inline = records.inlined(&scoped, &site);
    assert!(Arc::ptr_eq(
        location(&inline).scope.as_ref().unwrap(),
        &child
    ));
    assert!(location(&inline).inlined_at.is_some());
}
