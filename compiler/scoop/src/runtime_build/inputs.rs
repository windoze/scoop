use super::*;
use scoop_protocol::HostPathCarrier;
use scoop_wire::sha256;

pub(super) struct Inputs {
    pub root: PathBuf,
    pub sources: Vec<String>,
    pub flags: Vec<String>,
    pub includes: Vec<PathBuf>,
    pub compiler_inputs: Digest256,
    pub base_key: Digest256,
}

impl Inputs {
    pub fn read(request: &RuntimeBuildRequest<'_>) -> Result<Self, RuntimeBuildError> {
        let root = request.runtime_root.canonicalize().map_err(error)?;
        let sources = request
            .target
            .runtime_build()
            .runtime_sources()
            .into_iter()
            .map(|path| {
                path.strip_prefix("runtime/")
                    .map(str::to_owned)
                    .ok_or_else(|| error(format!("invalid runtime source rule {path}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut includes: Vec<_> = request
            .target
            .runtime_build()
            .include_directories()
            .iter()
            .map(|p| root.join(p))
            .collect();
        if let Some(include) = scoop_toolchain::runtime_unwind_include(
            request.target.lir_target(),
            request.unwind_prefix,
        )
        .map_err(error)?
        {
            includes.push(include);
        }
        let flags: Vec<_> = request
            .target
            .runtime_build()
            .runtime_c_flags()
            .into_iter()
            .chain(request.optimization.c_flags().iter().copied())
            .chain([
                "-funwind-tables",
                "-fasynchronous-unwind-tables",
                "-fno-lto",
            ])
            .map(str::to_owned)
            .collect();
        let compiler_inputs = request.target.native_tool_inputs();
        let path = |p: &Path| HostPathCarrier::from_path(p).map_err(error);
        let key = serde_json::to_vec(&(
            path(&root)?,
            &sources,
            &flags,
            includes
                .iter()
                .map(|p| path(p))
                .collect::<Result<Vec<_>, _>>()?,
            compiler_inputs.as_array(),
            encode(request.target.c_bridge_toolchain().profile().contract()).map_err(error)?,
            encode(&scoop_lir::RuntimeAbiContract).map_err(error)?,
        ))
        .map_err(error)?;
        Ok(Self {
            root,
            sources,
            flags,
            includes,
            compiler_inputs,
            base_key: sha256(&key),
        })
    }
}
