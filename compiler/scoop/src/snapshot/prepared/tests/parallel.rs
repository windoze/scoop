use std::num::NonZeroUsize;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use super::*;

fn write_parallel_graph(workspace: &Path) {
    write_core(&workspace.join("sysroot"));
    write_fake_compiler(&workspace.join("bin/scoopc"));
    for name in ["first", "second", "third", "root"] {
        let root = workspace.join(name);
        std::fs::create_dir_all(root.join("src")).unwrap();
        let mut manifest = format!(
            "schema = 1\n[cone]\ngroup = \"test\"\nname = \"{name}\"\nversion = \"1.0.0\"\nkind = \"library\"\n"
        );
        if name == "root" {
            for dependency in ["first", "second", "third"] {
                manifest.push_str(&format!("[dependencies.\"test:{dependency}\"]\nversion = \"1.0.0\"\npath = \"../{dependency}\"\n"));
            }
        }
        std::fs::write(root.join("Cone.toml"), manifest).unwrap();
        std::fs::write(root.join("src/main.scoop"), "fun value(): Int = 42\n").unwrap();
    }
}

fn prepare_jobs(workspace: &Path, jobs: usize) -> PreparedBuildGraph {
    request(&workspace.join("root"), workspace)
        .with_jobs(NonZeroUsize::new(jobs).unwrap())
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap()
}

fn source_identity(graph: &PreparedBuildGraph, name: &str) -> ConeIdentity {
    graph
        .dependency_first()
        .iter()
        .copied()
        .find(|id| {
            graph
                .source_input_path(*id)
                .is_some_and(|path| path.ends_with(name))
        })
        .unwrap()
}

struct OverlappingFailures {
    requests: Mutex<Vec<(RequestCorrelationId, NonZeroUsize)>>,
    expected: usize,
    entered: Condvar,
    slow: RequestCorrelationId,
}

impl SingleConeCompilerRunner for OverlappingFailures {
    fn invoke(
        &self,
        tool: &ResolvedPairedScoopc,
        request: &ScoopcRequestEnvelopeV1,
        io: &ChildIoPlan,
    ) -> Result<ScoopcResponseEnvelopeV1, ChildTransportError> {
        let mut requests = self.requests.lock().unwrap();
        requests.push((request.request_id(), io.jobs));
        self.entered.notify_all();
        let (requests, timeout) = self
            .entered
            .wait_timeout_while(requests, Duration::from_secs(5), |requests| {
                requests.len() < self.expected
            })
            .unwrap();
        assert!(
            !timeout.timed_out(),
            "independent compiler requests must overlap"
        );
        drop(requests);
        if request.request_id() == self.slow {
            std::thread::sleep(Duration::from_millis(30));
        }
        FailureRunner.invoke(tool, request, io)
    }
}

#[test]
fn independent_children_share_the_worker_limit_and_fail_in_canonical_order() {
    for jobs in [2, 8] {
        let temp = tempfile::tempdir().unwrap();
        write_parallel_graph(temp.path());
        let graph = prepare_jobs(temp.path(), jobs);
        let first = source_identity(&graph, "first");
        let ordinal = graph
            .dependency_first()
            .iter()
            .position(|id| *id == first)
            .unwrap() as u128
            + 1;
        let runner = OverlappingFailures {
            requests: Mutex::new(Vec::new()),
            entered: Condvar::new(),
            expected: jobs.min(3),
            slow: RequestCorrelationId::from_array(ordinal.to_be_bytes()),
        };
        let error = graph.execute_with_runner(&runner).unwrap_err();
        assert!(
            matches!(error.cause(), BuildGraphExecutionError::Ordinary(id, source)
            if *id == first && matches!(source.as_ref(), OrdinarySourceExecutionError::ChildFailure(_)))
        );
        // Failures prevent the dependent root and any unstarted sibling from running.
        let requests = runner.requests.into_inner().unwrap();
        assert_eq!(requests.len(), jobs.min(3));
        assert_eq!(
            requests.iter().map(|(_, jobs)| jobs.get()).sum::<usize>(),
            jobs
        );
    }
}

#[test]
fn one_job_stops_before_starting_other_ready_nodes() {
    let temp = tempfile::tempdir().unwrap();
    write_parallel_graph(temp.path());
    let graph = prepare_jobs(temp.path(), 1);
    let runner = OverlappingFailures {
        requests: Mutex::new(Vec::new()),
        entered: Condvar::new(),
        expected: 1,
        slow: RequestCorrelationId::from_array([0; 16]),
    };
    assert!(graph.execute_with_runner(&runner).is_err());
    let requests = runner.requests.into_inner().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].1.get(), 1);
}

#[test]
fn a_single_ready_cone_receives_all_available_workers() {
    let temp = tempfile::tempdir().unwrap();
    write_parallel_graph(temp.path());
    let graph = request(&temp.path().join("first"), temp.path())
        .with_jobs(NonZeroUsize::new(7).unwrap())
        .load_root()
        .unwrap()
        .discover()
        .unwrap()
        .resolve()
        .unwrap()
        .prepare()
        .unwrap();
    let runner = OverlappingFailures {
        requests: Mutex::new(Vec::new()),
        entered: Condvar::new(),
        expected: 1,
        slow: RequestCorrelationId::from_array([0; 16]),
    };
    assert!(graph.execute_with_runner(&runner).is_err());
    let requests = runner.requests.into_inner().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].1.get(), 7);
}

#[test]
fn independent_roots_share_the_job_limit_without_blocking_on_their_own_key_lock() {
    let temp = tempfile::tempdir().unwrap();
    write_parallel_graph(temp.path());
    // The repeated first root represents a shared source ready in two closures.
    // A second distinct root must still start while the first holds its key lock.
    let graphs = ["first", "first", "second", "third"].map(|name| {
        request(&temp.path().join(name), temp.path())
            .with_jobs(NonZeroUsize::new(2).unwrap())
            .load_root()
            .unwrap()
            .discover()
            .unwrap()
            .resolve()
            .unwrap()
            .prepare()
            .unwrap()
    });
    let first = graphs[0].root_identity();
    let ordinal = graphs[0]
        .dependency_first()
        .iter()
        .position(|id| *id == first)
        .unwrap() as u128
        + 1;
    let runner = OverlappingFailures {
        requests: Mutex::new(Vec::new()),
        entered: Condvar::new(),
        expected: 2,
        slow: RequestCorrelationId::from_array(ordinal.to_be_bytes()),
    };
    let error =
        PreparedBuildGraph::execute_all_with_runner(Vec::from(graphs), &runner).unwrap_err();
    assert!(matches!(error.cause(), BuildGraphExecutionError::Ordinary(id, _) if *id == first));
    let requests = runner.requests.into_inner().unwrap();
    assert_eq!(requests.len(), 2);
    assert_ne!(requests[0].0, requests[1].0);
    assert_eq!(
        requests.iter().map(|(_, jobs)| jobs.get()).sum::<usize>(),
        2
    );
}

#[test]
fn jobs_do_not_change_compilation_keys() {
    let temp = tempfile::tempdir().unwrap();
    write_parallel_graph(temp.path());
    let first = prepare_jobs(temp.path(), 1);
    let second = prepare_jobs(temp.path(), 4);
    let identity = source_identity(&first, "first");
    let core = first
        .complete_prebuilt_node(ConeIdentity::CORE, &[])
        .unwrap();
    assert_eq!(
        first.compile_cache_key(identity, &[&core]).unwrap(),
        second.compile_cache_key(identity, &[&core]).unwrap()
    );
}

#[test]
fn busy_cache_key_does_not_block_an_active_child_from_releasing_its_lock() {
    let temp = tempfile::tempdir().unwrap();
    write_parallel_graph(temp.path());
    let graph = prepare_jobs(temp.path(), 2);
    let second = source_identity(&graph, "second");
    let core = graph
        .complete_prebuilt_node(ConeIdentity::CORE, &[])
        .unwrap();
    let key = graph.compile_cache_key(second, &[&core]).unwrap();
    let store =
        CompileCacheStoreV1::new(&ArtifactCacheRoot::new(temp.path().join("cache")).unwrap());
    let lock = store.acquire_exclusive(key).unwrap();
    let (finished, release) = std::sync::mpsc::channel();
    let holder = std::thread::spawn(move || {
        let completed = release.recv_timeout(Duration::from_secs(5)).is_ok();
        drop(lock);
        completed
    });
    assert!(graph.execute_with_runner(&FailureRunner).is_err());
    let _ = finished.send(());
    assert!(
        holder.join().unwrap(),
        "waiting for another key must not block active child completion"
    );
}
