use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use scoop_identity::ConeIdentity;
use scoop_protocol::{EmittedDumpDescriptorV1, EmittedDumpDestinationV1, StageDumpSet};

use super::model::{PreparedBuildGraph, PreparedGraphNode};
use crate::ImmutableInputSnapshot;

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
    /// Observe selected source nodes through the same production invocation.
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

    pub(crate) fn is_observed(&self, identity: ConeIdentity) -> bool {
        self.dumps.contains_key(&identity)
    }

    pub(super) fn publish_dumps(
        &self,
        identity: ConeIdentity,
        descriptors: &[EmittedDumpDescriptorV1],
    ) -> io::Result<()> {
        let Some(dump) = self.dumps.get(&identity) else {
            return if descriptors.is_empty() {
                Ok(())
            } else {
                Err(invalid("unrequested stage dumps"))
            };
        };
        let captures = capture_dumps(dump, descriptors)?;
        std::fs::create_dir_all(&dump.destination)?;
        for (stage, snapshot) in dump.stages.iter().zip(captures) {
            let mut file = tempfile::NamedTempFile::new_in(&dump.destination)?;
            file.write_all(snapshot.as_bytes())?;
            file.as_file().sync_all()?;
            file.persist(dump.destination.join(stage.file_name()))
                .map_err(|error| error.error)?;
        }
        Ok(())
    }
}

fn capture_dumps(
    dump: &PreparedDump,
    descriptors: &[EmittedDumpDescriptorV1],
) -> io::Result<Vec<ImmutableInputSnapshot>> {
    let stages = dump.stages.iter().collect::<Vec<_>>();
    if descriptors
        .iter()
        .map(|descriptor| descriptor.stage())
        .collect::<Vec<_>>()
        != stages
    {
        return Err(invalid(
            "compiler dump descriptors do not match the requested stage set",
        ));
    }
    validate_directory(&dump.private_directory)?;
    let names = std::fs::read_dir(&dump.private_directory)?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let expected = stages
        .iter()
        .map(|stage| stage.file_name().into())
        .collect();
    if names != expected {
        return Err(invalid(
            "compiler dump directory does not contain exactly the requested files",
        ));
    }
    descriptors
        .iter()
        .map(|descriptor| {
            let EmittedDumpDestinationV1::File(carrier) = descriptor.destination() else {
                return Err(invalid("machine stage dumps cannot use stdout"));
            };
            let path = carrier.to_path_buf().map_err(invalid)?;
            let expected = dump.private_directory.join(descriptor.stage().file_name());
            if path != expected {
                return Err(invalid(format!(
                    "compiler dump path differs from {}",
                    expected.display()
                )));
            }
            let snapshot = ImmutableInputSnapshot::capture_no_follow(&path).map_err(invalid)?;
            if snapshot.digest().as_array() != descriptor.content_digest().as_array() {
                return Err(invalid(format!(
                    "compiler dump digest differs for {}",
                    path.display()
                )));
            }
            Ok(snapshot)
        })
        .collect()
}

fn validate_directory(path: &Path) -> io::Result<()> {
    if !std::fs::symlink_metadata(path)?.is_dir() {
        return Err(invalid("compiler dump directory is not a directory"));
    }
    Ok(())
}

fn invalid(message: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

#[cfg(test)]
mod tests;
