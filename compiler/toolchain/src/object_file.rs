//! Stable native-tool inputs shared by object production and artifact linking.
use std::io::Write;
use std::path::{Path, PathBuf};

use scoop_wire::Digest256;

use crate::ToolchainError;

pub fn store_object(
    cache: &Path,
    digest: Digest256,
    bytes: &[u8],
) -> Result<PathBuf, ToolchainError> {
    let directory = std::path::absolute(cache)
        .map_err(error)?
        .join("objects-v1");
    let path = directory.join(format!("{digest}.o"));
    if path.try_exists().map_err(error)? {
        return Ok(path);
    }
    std::fs::create_dir_all(&directory).map_err(error)?;
    let mut temporary = tempfile::NamedTempFile::new_in(directory).map_err(error)?;
    temporary.write_all(bytes).map_err(error)?;
    match temporary.persist_noclobber(&path) {
        Ok(_) => {}
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(error(e.error)),
    }
    Ok(path)
}

fn error(error: impl std::fmt::Display) -> ToolchainError {
    ToolchainError(format!("object file: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrent_outputs_publish_once_and_existing_files_are_reused() {
        let directory = tempfile::tempdir().unwrap();
        let bytes = vec![42; 65536];
        let digest = scoop_wire::sha256(&bytes);
        let barrier = std::sync::Barrier::new(4);
        let paths = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        store_object(directory.path(), digest, &bytes).unwrap()
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert!(paths.iter().all(|path| path == &paths[0]));
        assert_eq!(std::fs::read(&paths[0]).unwrap(), bytes);
        let modified = paths[0].metadata().unwrap().modified().unwrap();
        assert_eq!(
            store_object(directory.path(), digest, &bytes).unwrap(),
            paths[0]
        );
        assert_eq!(paths[0].metadata().unwrap().modified().unwrap(), modified);
        let changed =
            store_object(directory.path(), scoop_wire::sha256(b"changed"), b"changed").unwrap();
        assert_ne!(changed, paths[0]);
        assert_eq!(std::fs::read(&paths[0]).unwrap(), bytes);
    }
}
