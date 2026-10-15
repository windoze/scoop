use std::io;
use std::path::{Path, PathBuf};

use scoop_identity::ConeIdentity;
use scoop_protocol::{EmittedDumpDescriptorV1, EmittedDumpDestinationV1, StageDumpSet};

use super::model::{PreparedBuildGraph, PreparedGraphNode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DumpScope {
    Root,
    Sources,
}

#[derive(Clone, Debug)]
pub struct BuildDumpRequest {
    pub stages: StageDumpSet,
    pub directory: PathBuf,
    pub scope: DumpScope,
}

#[derive(Debug)]
pub(super) struct PreparedDump {
    pub(super) stages: StageDumpSet,
    pub(super) private_directory: PathBuf,
    destination: PathBuf,
}

impl PreparedBuildGraph {
    /// Request stage outputs for selected source nodes.
    pub fn observe_dumps(&mut self, request: BuildDumpRequest) -> io::Result<()> {
        if !self.dumps.is_empty() {
            return Err(invalid("stage observation is already configured"));
        }
        for (identity, node) in &self.nodes {
            if matches!(node, PreparedGraphNode::PrebuiltArtifact(_))
                || request.scope == DumpScope::Root && *identity != self.root
            {
                continue;
            }
            let private_directory = self.staging.root().join("dumps").join(identity.to_string());
            std::fs::create_dir_all(&private_directory)?;
            self.dumps.insert(
                *identity,
                PreparedDump {
                    stages: request.stages,
                    private_directory,
                    destination: request.directory.join(identity.to_string()),
                },
            );
        }
        Ok(())
    }

    pub(super) fn missing_dumps(
        &self,
        identity: ConeIdentity,
        cache: &Path,
    ) -> io::Result<Option<StageDumpSet>> {
        match self.dumps.get(&identity) {
            Some(dump) => missing_stages(dump.stages, cache),
            None => Ok(None),
        }
    }

    pub(super) fn store_dumps(
        &self,
        identity: ConeIdentity,
        stages: Option<StageDumpSet>,
        descriptors: &[EmittedDumpDescriptorV1],
        cache: &Path,
    ) -> io::Result<()> {
        let requested = stages.or_else(|| self.dumps.get(&identity).map(|dump| dump.stages));
        store_dumps(requested, descriptors, cache)
    }

    pub(super) fn publish_cached_dumps(
        &self,
        identity: ConeIdentity,
        cache: &Path,
    ) -> io::Result<()> {
        let Some(dump) = self.dumps.get(&identity) else {
            return Ok(());
        };
        std::fs::create_dir_all(&dump.destination)?;
        for stage in dump.stages.iter() {
            copy_dump(
                &cache.join(stage.file_name()),
                &dump.destination,
                stage.file_name(),
            )?;
        }
        Ok(())
    }
}

fn missing_stages(stages: StageDumpSet, cache: &Path) -> io::Result<Option<StageDumpSet>> {
    let mut missing = Vec::new();
    for stage in stages.iter() {
        if !cache.join(stage.file_name()).try_exists()? {
            missing.push(stage);
        }
    }
    if missing.is_empty() {
        Ok(None)
    } else {
        StageDumpSet::new(&missing).map(Some).map_err(invalid)
    }
}

fn store_dumps(
    requested: Option<StageDumpSet>,
    descriptors: &[EmittedDumpDescriptorV1],
    cache: &Path,
) -> io::Result<()> {
    let expected = requested.into_iter().flat_map(StageDumpSet::iter);
    if !expected.eq(descriptors.iter().map(EmittedDumpDescriptorV1::stage)) {
        return Err(invalid(
            "compiler dump descriptors do not match the requested stages",
        ));
    }
    if descriptors.is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(cache)?;
    for descriptor in descriptors {
        let EmittedDumpDestinationV1::File(path) = descriptor.destination() else {
            return Err(invalid("machine stage dumps require a file"));
        };
        copy_dump(
            &path.to_path_buf().map_err(invalid)?,
            cache,
            descriptor.stage().file_name(),
        )?;
    }
    Ok(())
}

fn copy_dump(source: &Path, directory: &Path, name: &str) -> io::Result<()> {
    let mut source = std::fs::File::open(source)?;
    let mut output = tempfile::NamedTempFile::new_in(directory)?;
    io::copy(&mut source, &mut output)?;
    output
        .persist(directory.join(name))
        .map_err(|error| error.error)?;
    Ok(())
}

fn invalid(message: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

#[cfg(test)]
mod tests;
