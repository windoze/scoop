use std::sync::Arc;

use scoop_analysis::{AnalysisHost, Cancelled};
use scoop_source::SourceText;

fn text(source: &str) -> SourceText {
    SourceText::from(source.to_owned())
}

#[test]
fn changed_files_replace_their_parse_and_preserve_other_inputs() {
    let mut host = AnalysisHost::default();
    let first = host.add_source(text("fun first() = 1"));
    let second = host.add_source(text("fun second() = 2"));
    let previous = host.snapshot();
    let previous_first = previous.source(first).unwrap().unwrap().clone();
    let previous_second = previous.source(second).unwrap().unwrap().clone();
    let second_parse = previous_second.parse().unwrap();
    assert!(host.set_text(first, text("fun changed() { value. }")));
    let current = host.snapshot();
    assert_eq!(previous.check_cancelled(), Err(Cancelled));
    assert!(current.check_cancelled().is_ok());
    let current_first = current.source(first).unwrap().unwrap();
    assert!(!Arc::ptr_eq(&previous_first, current_first));
    assert_eq!(previous_first.text.as_str(), "fun first() = 1");
    assert!(current_first.parse().unwrap().ast().is_none());
    let current_second = current.source(second).unwrap().unwrap();
    assert!(Arc::ptr_eq(&previous_second, current_second));
    assert!(std::ptr::eq(second_parse, current_second.parse().unwrap()));
}

#[test]
fn identical_text_reuses_the_revision_and_released_sources_do_not_reappear() {
    let mut host = AnalysisHost::default();
    let source = host.add_source(text("fun f() = 1"));
    let previous = host.snapshot();
    assert!(host.set_text(source, text("fun f() = 1")));
    assert_eq!(host.revision(), previous.revision);
    assert!(previous.check_cancelled().is_ok());
    host.remove_source(source);
    assert_eq!(previous.check_cancelled(), Err(Cancelled));
    assert!(!host.set_text(source, text("fun f() = 2")));
    assert!(host.snapshot().source(source).unwrap().is_none());
    let replacement = host.add_source(text("fun f() = 3"));
    assert_ne!(replacement, source);
}
