use std::fs;
use std::io;

use scoop_protocol::{
    EmittedDumpDescriptorV1, EmittedDumpDestinationV1, HostPathCarrier, StageDumpPolicyV1,
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
    fs::create_dir_all(&directory)?;
    dumps
        .iter()
        .map(|dump| {
            let path = directory.join(dump.kind().file_name());
            fs::write(&path, dump.text().as_bytes())?;
            Ok(EmittedDumpDescriptorV1::new(
                dump.kind(),
                EmittedDumpDestinationV1::File(
                    HostPathCarrier::from_path(&path).map_err(io::Error::other)?,
                ),
            ))
        })
        .collect()
}
