use std::fs::{File, OpenOptions};

use scoop_protocol::StageDumpSet;

use super::*;

struct LockCheckingRunner {
    file: File,
    observed: bool,
}

impl SingleConeCompilerRunner for LockCheckingRunner {
    fn invoke(
        &mut self,
        tool: &ResolvedPairedScoopc,
        request: &ScoopcRequestEnvelopeV1,
        io: &ChildIoPlan,
    ) -> Result<ScoopcResponseEnvelopeV1, ChildTransportError> {
        let result = fs4::FileExt::try_lock(&self.file);
        if self.observed {
            result.expect("an observed child must not monopolize the compile cache key");
            fs4::FileExt::unlock(&self.file).unwrap();
        } else {
            assert!(matches!(result, Err(fs4::TryLockError::WouldBlock)));
        }
        FailureRunner.invoke(tool, request, io)
    }
}

#[test]
fn observations_leave_the_cache_key_available_while_ordinary_misses_deduplicate() {
    for observed in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path();
        let root = workspace.join("root");
        write_core(&workspace.join("sysroot"));
        write_manifest(&root, "root", "");
        write_fake_compiler(&workspace.join("bin/scoopc"));
        let mut prepared = prepare(&root, workspace).unwrap();
        if observed {
            prepared
                .observe_dumps(BuildDumpRequest {
                    stages: StageDumpSet::all(),
                    directory: workspace.join("dumps"),
                    scope: DumpScope::Sources,
                })
                .unwrap();
        }
        let key = prepared.compile_cache_key(ConeIdentity::CORE, &[]).unwrap();
        let store =
            CompileCacheStoreV1::new(&ArtifactCacheRoot::new(workspace.join("cache")).unwrap());
        let lock = store.acquire_shared(key).unwrap();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(lock.path())
            .unwrap();
        drop(lock);
        let mut runner = LockCheckingRunner { file, observed };
        assert!(matches!(
            prepared.execute_ordinary_source(
                ConeIdentity::CORE,
                &[],
                &mut runner,
                RequestCorrelationId::from_array([34; 16]),
            ),
            Err(OrdinarySourceExecutionError::ChildFailure(_))
        ));
        assert!(!store.entry_path(key).exists());
    }
}
