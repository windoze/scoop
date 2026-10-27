use super::*;
use scoop_protocol::HostPathCarrier;
use scoop_wire::{Digest256, encode, sha256};

pub(super) fn of(request: &Request<'_>) -> BuildResult<Digest256> {
    let artifacts = request
        .graph
        .artifacts()
        .iter()
        .map(|node| {
            let summary = node.artifact().summary();
            Ok((
                encode(&summary.cone().identity()).map_err(failure)?,
                encode(&summary.code_fingerprint_availability()).map_err(failure)?,
            ))
        })
        .collect::<BuildResult<Vec<_>>>()?;
    let path = |path: &Path| HostPathCarrier::from_path(path).map_err(failure);
    let options = request.options;
    let mode = match options.mode {
        None => "default",
        Some(scoop_toolchain::LinkMode::Static) => "static",
        Some(scoop_toolchain::LinkMode::Dynamic) => "dynamic",
    };
    let tool =
        NativeInputMetadata::read(&std::env::current_exe().map_err(failure)?).map_err(failure)?;
    let bytes = serde_json::to_vec(&(
        artifacts,
        encode(&request.graph.root().artifact().summary().cone().identity()).map_err(failure)?,
        request.runtime.fingerprint().digest().as_array(),
        encode(&request.target.child_request().map_err(failure)?).map_err(failure)?,
        encode(&request.target.lir_target().wire_id()).map_err(failure)?,
        request.profile.as_str(),
        request.keep_symbols(),
        request.emit_symbol_map(),
        mode,
        options.sysroot.as_deref().map(path).transpose()?,
        options.unwind_prefix.as_deref().map(path).transpose()?,
        options.linker.as_deref().map(path).transpose()?,
        request
            .libraries
            .iter()
            .map(|p| path(p))
            .collect::<BuildResult<Vec<_>>>()?,
        tool,
    ))
    .map_err(failure)?;
    Ok(sha256(&bytes))
}
