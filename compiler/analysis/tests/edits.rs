use std::sync::Arc;

use scoop_analysis::{AnalysisHost, SnapshotEditError};
use scoop_source::{Revision, SourceEdit, SourceText, Span};

#[test]
fn private_edits_preserve_the_host_and_share_unchanged_inputs() {
    let mut host = AnalysisHost::default();
    let text = "fun 名字() {}\n";
    let first = host.add_source(SourceText::from(text.to_owned()));
    let second = host.add_source(SourceText::from("fun second() {}\n".to_owned()));
    let original = host.snapshot();
    let source = original.source(first).unwrap().unwrap();
    let unchanged = original.source(second).unwrap().unwrap();
    unchanged.parse().unwrap();
    let start = text.find("名字").unwrap() as u32;
    let candidate = original
        .with_edits(&[SourceEdit {
            source: source.version,
            span: Span::new(start, start + "名字".len() as u32),
            replacement: "renamed".into(),
        }])
        .unwrap();
    assert_eq!(
        candidate.source(first).unwrap().unwrap().text.as_str(),
        "fun renamed() {}\n"
    );
    assert_eq!(original.source(first).unwrap().unwrap().text.as_str(), text);
    assert!(Arc::ptr_eq(
        unchanged,
        candidate.source(second).unwrap().unwrap()
    ));
    assert!(Arc::ptr_eq(source, host.source(first).unwrap()));
    assert_ne!(
        candidate.source(first).unwrap().unwrap().version,
        source.version
    );
    original.check_cancelled().unwrap();
    candidate.check_cancelled().unwrap();
    host.set_text(second, SourceText::from("fun updated() {}\n".to_owned()));
    assert!(original.check_cancelled().is_err());
    assert!(candidate.check_cancelled().is_err());
}

#[test]
fn private_edits_reject_stale_overlapping_and_non_character_ranges() {
    let mut host = AnalysisHost::default();
    let text = "fun 名字() {}\n";
    let id = host.add_source(SourceText::from(text.to_owned()));
    let original = host.snapshot();
    let source = original.source(id).unwrap().unwrap();
    let edit = SourceEdit {
        source: source.version,
        span: Span::new(4, 10),
        replacement: "valid".into(),
    };
    let mut stale = edit.clone();
    stale.source.revision = Revision(0);
    assert_eq!(
        original.with_edits(&[stale]).unwrap_err(),
        SnapshotEditError::SourceChanged(id)
    );
    assert_eq!(
        original
            .with_edits(&[edit.clone(), edit.clone()])
            .unwrap_err(),
        SnapshotEditError::Overlap(id)
    );
    for span in [Span::new(5, 10), Span::new(10, 4), Span::new(4, 100)] {
        let invalid = SourceEdit {
            span,
            ..edit.clone()
        };
        assert_eq!(
            original.with_edits(&[invalid]).unwrap_err(),
            SnapshotEditError::InvalidRange(id)
        );
    }
    assert_eq!(host.source(id).unwrap().text.as_str(), text);
    original.check_cancelled().unwrap();
}
