use std::{fs, path::PathBuf};

use super::*;
use crate::cache::{FileStamp, PackageCache, atomic_write};

#[derive(serde::Serialize, serde::Deserialize)]
struct Input {
    path: HostPathCarrier,
    stamp: Option<FileStamp>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct Inputs(Vec<Input>);

impl Inputs {
    pub fn capture(resolved: &ResolvedWorkspace) -> PmResult<Self> {
        let mut inputs: BTreeMap<_, _> = resolved
            .missing_inputs
            .iter()
            .map(|path| (path.clone(), None))
            .collect();
        inputs.extend(
            resolved
                .inputs
                .iter()
                .map(|(path, stamp)| (path.clone(), Some(stamp.clone()))),
        );
        Ok(Self(
            inputs
                .into_iter()
                .map(|(path, stamp)| {
                    Ok(Input {
                        path: host(&path)?,
                        stamp,
                    })
                })
                .collect::<PmResult<_>>()?,
        ))
    }

    fn unchanged(&self) -> PmResult<bool> {
        for input in &self.0 {
            if FileStamp::optional(&path(&input.path)?)? != input.stamp {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

pub(super) struct PreparedRecord {
    key: Vec<u8>,
    inputs: PathBuf,
    response: PathBuf,
}

impl PreparedRecord {
    pub fn new(cache: &PackageCache, request: &PrepareRequestV1) -> PmResult<Self> {
        let mut request = request.clone();
        request.package_cache = Some(host(cache.root())?);
        // These policies affect acquisition, not a valid, already prepared graph.
        request.locked = false;
        request.offline = false;
        let configuration = crate::resolve::config::path()
            .as_deref()
            .map(host)
            .transpose()?;
        let key = serde_json::to_vec(&(scoop_toolchain::VERSION, request, configuration))
            .map_err(|error| PmError::new(error.to_string()))?;
        let stem = cache
            .root()
            .join("records/prepared-v1")
            .join(scoop_wire::sha256(&key).to_string());
        Ok(Self {
            key,
            inputs: stem.with_extension("inputs.json"),
            response: stem.with_extension("response.json"),
        })
    }

    pub fn key(&self) -> &[u8] {
        &self.key
    }

    pub fn load(&self) -> PmResult<Option<PreparedWorkspaceV1>> {
        let Some(bytes) = read(&self.inputs)? else {
            return Ok(None);
        };
        let inputs: Inputs =
            serde_json::from_slice(&bytes).map_err(|error| PmError::new(error.to_string()))?;
        if !inputs.unchanged()? {
            return Ok(None);
        }
        read(&self.response)?
            .map(|bytes| {
                serde_json::from_slice(&bytes).map_err(|error| PmError::new(error.to_string()))
            })
            .transpose()
    }

    pub fn publish(&self, response: &PreparedWorkspaceV1, inputs: &Inputs) -> PmResult<()> {
        if !inputs.unchanged()? {
            return Err(PmError::new(
                "input changed while preparing packages; retry the command",
            ));
        }
        let response =
            serde_json::to_vec(response).map_err(|error| PmError::new(error.to_string()))?;
        let inputs = serde_json::to_vec(inputs).map_err(|error| PmError::new(error.to_string()))?;
        atomic_write(&self.response, &response)?;
        atomic_write(&self.inputs, &inputs)
    }
}

fn read(path: &Path) -> PmResult<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(PmError::io(path, error)),
    }
}
