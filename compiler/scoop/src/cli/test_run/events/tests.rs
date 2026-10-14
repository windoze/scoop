use super::*;

fn run(records: &[serde_json::Value], expected: &[&str]) -> RunResult {
    let input = records
        .iter()
        .map(|record| format!("{record}\n"))
        .collect::<String>();
    read(input.as_bytes(), expected)
}

fn begin(case: &str) -> serde_json::Value {
    serde_json::json!({"event": "begin", "case": case})
}

fn end(case: &str) -> serde_json::Value {
    serde_json::json!({"event": "end", "case": case, "duration_ns": 7, "failures": []})
}

fn complete() -> serde_json::Value {
    serde_json::json!({"event": "complete"})
}

#[test]
fn completed_selection_retains_original_catalogue_keys_and_durations() {
    let result = run(
        &[begin("2"), end("2"), begin("7"), end("7"), complete()],
        &["2", "7"],
    );
    assert!(result.complete);
    assert!(!result.failed());
    assert!(result.cases.iter().all(|case| case.duration_ns == Some(7)));
}

#[test]
fn premature_exit_preserves_passed_interrupted_and_unstarted_cases() {
    let result = run(&[begin("0"), end("0"), begin("1")], &["0", "1", "2"]);
    assert!(!result.complete);
    assert!(result.failed());
    assert_eq!(result.cases[0].status, Status::Passed);
    assert_eq!(result.cases[1].status, Status::Failed);
    assert!(result.cases[1].interruption.is_some());
    assert_eq!(result.cases[2].status, Status::NotRun);
    assert!(result.target_failure.unwrap().contains("completion"));
}

#[test]
fn missing_completion_and_invalid_sequences_cannot_be_successful() {
    for records in [
        vec![],
        vec![begin("0"), end("0")],
        vec![end("0"), complete()],
        vec![begin("9")],
        vec![begin("0"), begin("0")],
        vec![begin("0"), end("1")],
        vec![begin("0"), complete()],
        vec![begin("0"), end("0"), complete(), complete()],
    ] {
        let result = run(&records, &["0"]);
        assert!(result.failed(), "{records:?}");
        assert!(result.target_failure.is_some(), "{records:?}");
    }
}

#[test]
fn truncated_json_keeps_the_started_case_and_original_failure_details() {
    let mut input = format!("{}\n", begin("0"));
    input.push_str("{\"event\":\"end\"");
    let result = read(input.as_bytes(), &["0"]);
    assert_eq!(result.cases[0].status, Status::Failed);
    assert!(
        result
            .target_failure
            .unwrap()
            .contains("invalid test event")
    );

    let mut finished = end("0");
    let failure = serde_json::json!({"phase": "body", "location": {
        "file": "tests/雪.scoop", "line": 8, "column": 3, "function": "check", "type": "Cases"
    }, "kind": "assertion", "assertion": "assertEquals", "message": "雪\n\"", "expected": {
        "kind": "rendering-failed", "message": "render failed"
    }, "actual": {"kind": "text", "text": "2"}, "cause": null});
    let mut cleanup = failure.clone();
    cleanup["phase"] = serde_json::json!("after-test");
    finished["failures"] = serde_json::json!([failure, cleanup]);
    let result = run(&[begin("0"), finished, complete()], &["0"]);
    assert!(result.complete);
    assert!(result.target_failure.is_none());
    assert_eq!(result.cases[0].status, Status::Failed);
    assert_eq!(result.cases[0].failures.len(), 2);
    assert_eq!(
        serde_json::to_value(&result.cases[0].failures[0]).unwrap(),
        failure
    );
}
