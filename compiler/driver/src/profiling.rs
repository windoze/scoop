//! One LLVM reader boundary for summaries and MIR feedback.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub struct LoadedProfile {
    pub target: String,
    pub feedback: scoop_profile::Feedback,
    pub maps: Vec<scoop_profile::ProfileMap>,
    pub input_paths: Vec<PathBuf>,
    pub compiler_inputs: Vec<scoop_protocol::CompilerInputV1>,
}

#[derive(Debug)]
pub struct ProfileInputError(pub String);
impl std::fmt::Display for ProfileInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ProfileInputError {}

pub fn read_profile(
    index: &Path,
    target: Option<&str>,
) -> Result<LoadedProfile, ProfileInputError> {
    let mut compiler_inputs = Vec::new();
    let mut read = |path: &Path| {
        compiler_inputs.push(scoop_protocol::CompilerInputV1::BuildFile(
            scoop_toolchain::build_file_input(path).map_err(ProfileInputError)?,
        ));
        std::fs::read(path)
            .map_err(|e| ProfileInputError(format!("profile input {}: {e}", path.display())))
    };
    let path = std::path::absolute(index).map_err(|e| ProfileInputError(e.to_string()))?;
    let index: scoop_profile::ProfileIndex = serde_json::from_slice(&read(&path)?)
        .map_err(|e| ProfileInputError(format!("profile index {}: {e}", path.display())))?;
    if index.schema != scoop_profile::SCHEMA {
        return Err(ProfileInputError(format!(
            "unsupported Scoop profile index schema {}",
            index.schema
        )));
    }
    if target.is_some_and(|target| target != index.target) {
        return Err(ProfileInputError(format!(
            "profile target {} does not match {}",
            index.target,
            target.unwrap()
        )));
    }
    let directory = path.parent().expect("absolute profile path has parent");
    let counts = directory.join(&index.counts);
    let records = scoop_codegen::profiling::decode_indexed_profile(&read(&counts)?)
        .map_err(|e| ProfileInputError(e.to_string()))?;
    let paths = index
        .maps
        .iter()
        .map(|map| directory.join(map))
        .collect::<BTreeSet<_>>();
    let maps = paths
        .iter()
        .map(|map| {
            serde_json::from_slice::<scoop_profile::ProfileMap>(&read(map)?)
                .map_err(|e| ProfileInputError(format!("profile map {}: {e}", map.display())))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let feedback = scoop_profile::Feedback::from_records(&index.target, &maps, &records)
        .map_err(ProfileInputError)?;
    let input_paths = std::iter::once(path)
        .chain(std::iter::once(counts))
        .chain(paths)
        .collect();
    Ok(LoadedProfile {
        target: index.target,
        feedback,
        maps,
        input_paths,
        compiler_inputs,
    })
}
