use super::*;

#[cfg(windows)]
mod windows;

#[test]
fn actual_host_discovery_is_reused_and_transported_to_the_child() {
    let cache = tempfile::tempdir().unwrap();
    let triple = crate::host_target_triple().unwrap();
    let request = TargetSelectionRequestV1::new(triple.to_owned()).unwrap();
    let cold = ResolvedTargetProfile::resolve_request_cached(&request, cache.path()).unwrap();
    let records: Vec<_> = std::fs::read_dir(cache.path().join("native-tools-v1"))
        .unwrap()
        .map(|p| p.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    assert_eq!(records.len(), 1);
    let modified = records[0].metadata().unwrap().modified().unwrap();
    let warm = ResolvedTargetProfile::resolve_request_cached(&request, cache.path()).unwrap();
    assert_eq!(cold, warm);
    assert_eq!(records[0].metadata().unwrap().modified().unwrap(), modified);
    let child = cold.child_request().unwrap();
    assert!(child.resolved_toolchain().is_some());
    assert_eq!(
        ResolvedTargetProfile::resolve_request(&child).unwrap(),
        cold
    );
}

#[test]
fn observations_detect_real_input_changes_and_new_lookup_candidates() {
    let directory = tempfile::tempdir().unwrap();
    let selected = directory.path().join("selected");
    let earlier = directory.path().join("earlier");
    std::fs::write(&selected, "first").unwrap();
    let input = Input::read(&selected).unwrap();
    let absent = Input::read(&earlier).unwrap();
    assert!(input.is_current().unwrap());
    assert!(absent.is_current().unwrap());
    std::fs::write(&selected, "changed input").unwrap();
    std::fs::write(&earlier, "new candidate").unwrap();
    assert!(!input.is_current().unwrap());
    assert!(!absent.is_current().unwrap());
}
