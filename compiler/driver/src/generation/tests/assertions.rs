use super::*;
use serde_json::json;

pub(super) fn positive(queries: &mut dyn SemanticQueries, batches: &[GeneratorBatch]) {
    assert_eq!(
        batches
            .iter()
            .map(|batch| batch.generator.as_str())
            .collect::<Vec<_>>(),
        ["merge", "multi", "partial", "inherited", "generic"]
    );
    let merge = &batches[0].tasks[0];
    assert_eq!(merge.bindings.len(), 2);
    assert_eq!(merge.methods.len(), 1);
    assert_eq!(merge.methods.first().pending.requirements.len(), 2);
    let multi = &batches[1].tasks[0];
    assert_eq!(multi.bindings.len(), 2);
    assert_eq!(multi.methods.len(), 2);
    let partial = &batches[2].tasks[0];
    assert_eq!(partial.bindings.len(), 2);
    assert_eq!(partial.methods.len(), 1);
    assert_eq!(
        queries
            .signature(partial.methods.first().pending.function)
            .name,
        "distinct"
    );
    assert_eq!(batches[3].tasks.len(), 1);
    let generic = batches[4].request(queries).unwrap();
    assert_eq!(
        generic["tasks"][0]["owner"]["arguments"][0],
        json!({"kind": "binder", "depth": 0, "index": 0})
    );
    for batch in batches {
        let request = batch.request(queries).unwrap();
        assert_eq!(request["generator"], batch.generator);
        assert_eq!(request["protocolVersion"], 1);
        assert_eq!(request["currentCone"], ConeIdentity::CORE.to_string());
        for method in request["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|task| task["methods"].as_array().unwrap())
        {
            assert!(!method["requirements"].as_array().unwrap().is_empty());
            assert_eq!(method["method"], method["signature"]["method"]);
            assert_eq!(
                method["origin"]["source"]["path"],
                "src/generator-plans.scoop"
            );
            assert!(method["requirements"][0]["slot"].as_str().unwrap().len() == 64);
        }
    }
}

pub(super) fn errors(queries: &mut dyn SemanticQueries, annotations: &[GenerationAnnotation]) {
    for (names, message, note) in [
        (&["duplicate"][..], "duplicate GenerateWith", true),
        (&["dupComplete"][..], "duplicate GenerateWith", true),
        (&["conflictA", "conflictB"][..], "same missing method", true),
        (&["syntax"][..], "expected", false),
        (&["notInherited"][..], "not inherited", false),
        (&["nonInterface"][..], "must name an interface", false),
        (&["bareGeneric"][..], "type argument", false),
        (&["wrongArguments"][..], "type argument", false),
        (&["unknown"][..], "unknown type", false),
        (&[" "][..], "must not be empty", false),
        (&["onInterface"][..], "implementation type", false),
    ] {
        let diagnostics = plan::plan(queries, select(annotations, names)).unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(message)),
            "{names:?}: {diagnostics:?}"
        );
        let diagnostic = &diagnostics[0];
        assert_eq!(
            diagnostic.source.as_ref().unwrap().logical_path().as_str(),
            "src/generator-plans.scoop"
        );
        let span = diagnostic.span.unwrap();
        assert!(span.start < span.end);
        if note {
            assert_eq!(diagnostic.notes.len(), 1);
            assert_ne!(diagnostic.notes[0].span, span);
        }
    }
    let all = plan_generators(queries).unwrap_err();
    assert!(all.len() >= 11);
}
