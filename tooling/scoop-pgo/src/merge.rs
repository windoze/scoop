use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use scoop_protocol::HostPathCarrier;
use serde::{Deserialize, Serialize};

use crate::{
    Result,
    data::{self, Dataset, Run},
    process::{self, Tools},
};

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Input {
    path: HostPathCarrier,
    length: u64,
    modified: std::time::SystemTime,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
struct State {
    schema: u32,
    target: String,
    inputs: Vec<Input>,
}

pub fn update(dataset: &Dataset, tools: &Tools, compiler: &Path, target: &str) -> Result<PathBuf> {
    let runs = dataset.root.join("runs");
    if !runs.try_exists()? {
        return Err("no training runs; use instrument first".into());
    }
    let mut raw = BTreeSet::new();
    let mut maps = BTreeSet::new();
    for directory in fs::read_dir(runs)? {
        let directory = directory?;
        if !directory.file_type()?.is_dir() {
            continue;
        }
        let record = directory.path().join("run.json");
        if !record.try_exists()? {
            continue;
        }
        let mut run: Run = data::read_json(&record)?;
        data::check_schema(run.schema)?;
        for path in &run.raw {
            if !directory.path().join(path).try_exists()? {
                return Err(format!(
                    "training input is missing: {}",
                    directory.path().join(path).display()
                )
                .into());
            }
        }
        let files = data::raw_files(&directory.path())?;
        if files.is_empty() {
            continue;
        }
        if run.raw != files {
            run.raw = files;
            data::write_json(&record, &run)?;
        }
        raw.extend(run.raw.iter().map(|path| directory.path().join(path)));
        maps.extend(run.maps.iter().map(|path| dataset.root.join(path)));
    }
    if raw.is_empty() {
        return Err("no runtime counters were collected; run the instrumented program using the recorded SCOOP_PROFILE_DIR".into());
    }
    let inputs = raw
        .iter()
        .chain(&maps)
        .map(|path| {
            let metadata =
                fs::metadata(path).map_err(|e| format!("profile input {}: {e}", path.display()))?;
            Ok(Input {
                path: data::path(path)?,
                length: metadata.len(),
                modified: metadata.modified()?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let state = State {
        schema: data::SCHEMA,
        target: target.to_owned(),
        inputs,
    };
    let merged = dataset.root.join("merged");
    let index = merged.join("profile.json");
    if merged.join("state.json").try_exists()? {
        let previous: State = data::read_json(&merged.join("state.json"))?;
        data::check_schema(previous.schema)?;
        // A removed run is still a missing prior merge input, not a request to drop samples.
        let current = state
            .inputs
            .iter()
            .map(|input| &input.path)
            .collect::<BTreeSet<_>>();
        for input in &previous.inputs {
            if !current.contains(&input.path) && !input.path.to_path_buf()?.try_exists()? {
                return Err(format!(
                    "training input is missing: {}",
                    input.path.to_path_buf()?.display()
                )
                .into());
            }
        }
        if previous == state
            && index.try_exists()?
            && merged.join("counts.profdata").try_exists()?
            && merged.join("summary.json").try_exists()?
        {
            return Ok(index);
        }
    }
    let stage = tempfile::Builder::new()
        .prefix(".merge-")
        .tempdir_in(&dataset.root)?;
    process::merge(
        tools,
        &raw.into_iter().collect::<Vec<_>>(),
        &stage.path().join("counts.profdata"),
    )?;
    let profile = scoop_profile::ProfileIndex {
        schema: scoop_profile::SCHEMA,
        target: target.to_owned(),
        counts: "counts.profdata".into(),
        maps: maps
            .into_iter()
            .map(|path| {
                let relative = path
                    .strip_prefix(&dataset.root)?
                    .to_str()
                    .ok_or("profile maps must have portable relative names")?;
                Ok(format!("../{}", relative.replace('\\', "/")))
            })
            .collect::<Result<_>>()?,
    };
    data::write_json(&stage.path().join("profile.json"), &profile)?;
    process::summary(
        compiler,
        &stage.path().join("profile.json"),
        &stage.path().join("summary.json"),
    )?;
    data::write_json(&stage.path().join("state.json"), &state)?;
    data::publish_directory(stage, &merged)?;
    Ok(index)
}
