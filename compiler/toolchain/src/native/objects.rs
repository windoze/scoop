use super::*;
use crate::build_record::{Input, Record, cached, fingerprint};
use scoop_protocol::HostPathCarrier;
use serde::{Deserialize, Serialize};

mod inputs;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedNativeObjects {
    key: Digest256,
    objects: Vec<PathBuf>,
    inputs: Vec<crate::NativeInputMetadata>,
}

impl PreparedNativeObjects {
    pub fn key(&self) -> Digest256 {
        self.key
    }
    pub fn objects(&self) -> &[PathBuf] {
        &self.objects
    }
    pub fn input_metadata(&self) -> &[crate::NativeInputMetadata] {
        &self.inputs
    }
}

#[derive(Serialize, Deserialize)]
struct Outputs {
    objects: Vec<HostPathCarrier>,
}

/// Check native inputs before invoking any preprocessor or compiler.
pub fn prepare_native_objects(
    manifest: &LoadedConeManifest,
    target: &crate::ResolvedTargetProfile,
    optimization: OptimizationMode,
    public_include: &Path,
    cache: &Path,
) -> Result<PreparedNativeObjects, ToolchainError> {
    let compiler = NativeToolchain::resolve_cached(target, manifest, cache)?;
    let key = inputs::configuration(manifest, target, &compiler, optimization, public_include)?;
    let mut inputs = compiler.input_metadata();
    if !manifest
        .parsed()
        .semantic()
        .native()
        .sources()
        .iter()
        .any(|source| source.predicate().matches(target.id()))
    {
        return Ok(PreparedNativeObjects {
            key,
            objects: Vec::new(),
            inputs,
        });
    }
    let directory = cache.join("native-objects-v1");
    let record = cached(
        &directory,
        key,
        |outputs: &Outputs| {
            for path in &outputs.objects {
                if !path
                    .to_path_buf()
                    .map_err(error)?
                    .try_exists()
                    .map_err(error)?
                {
                    return Ok(false);
                }
            }
            Ok(true)
        },
        || {
            let prepared = compiler.prepare(manifest, optimization, public_include)?;
            let temporary = tempfile::Builder::new()
                .prefix("objects-")
                .tempdir_in(&directory)
                .map_err(error)?;
            let mut objects = Vec::new();
            for (index, unit) in prepared.units().iter().enumerate() {
                let object = temporary.path().join(format!("{index:08}.o"));
                compile_native_source(
                    unit,
                    manifest.parsed().semantic().native(),
                    &compiler,
                    optimization,
                    &object,
                )?;
                objects.push(HostPathCarrier::from_path(&object).map_err(error)?);
            }
            let inputs = prepared
                .observed_paths
                .iter()
                .map(|p| Input::read(p))
                .collect::<Result<_, _>>()?;
            let _ = temporary.keep();
            Ok(Record {
                inputs,
                result: Outputs { objects },
            })
        },
    )?;
    inputs.extend(record.inputs.iter().cloned());
    Ok(PreparedNativeObjects {
        key: fingerprint(&(key.as_array(), &record.inputs))?,
        objects: record
            .result
            .objects
            .into_iter()
            .map(|p| p.to_path_buf().map_err(error))
            .collect::<Result<_, _>>()?,
        inputs,
    })
}
