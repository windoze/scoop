use std::fs::{self, OpenOptions};
use std::io::{self, Write};

use scoop_protocol::{
    EmittedDumpDescriptorV1, EmittedDumpDestinationV1, HostPathCarrier, ProtocolDumpContentDigest,
    StageDumpPolicyV1,
};

pub(super) fn write_dumps(
    policy: &StageDumpPolicyV1,
    dumps: &[scoopc::EmittedStageDump],
) -> io::Result<Vec<EmittedDumpDescriptorV1>> {
    let (stages, directory) = match policy {
        StageDumpPolicyV1::None if dumps.is_empty() => return Ok(Vec::new()),
        StageDumpPolicyV1::None => return Err(io::Error::other("unexpected stage dumps")),
        StageDumpPolicyV1::Files { stages, directory } => (stages, directory),
    };
    if !stages
        .iter()
        .eq(dumps.iter().map(scoopc::EmittedStageDump::kind))
    {
        return Err(io::Error::other(
            "compiler did not capture the requested stages",
        ));
    }
    let directory = directory.to_path_buf().map_err(io::Error::other)?;
    if !fs::symlink_metadata(&directory)?.is_dir() || fs::read_dir(&directory)?.next().is_some() {
        return Err(io::Error::other(
            "dump destination must be an empty directory",
        ));
    }
    dumps
        .iter()
        .map(|dump| {
            let path = directory.join(dump.kind().file_name());
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            file.write_all(dump.text().as_bytes())?;
            file.sync_all()?;
            Ok(EmittedDumpDescriptorV1::new(
                dump.kind(),
                EmittedDumpDestinationV1::File(
                    HostPathCarrier::from_path(&path).map_err(io::Error::other)?,
                ),
                ProtocolDumpContentDigest::from_array(
                    *scoop_wire::sha256(dump.text().as_bytes()).as_array(),
                ),
            ))
        })
        .collect()
}
