use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::ValueEnum;
use scoop::options::{BuildInvocation, BuildOptions};
use scoop_process::CommandExt;
use serde_json::Value;

use crate::{
    Result,
    args::Common,
    data::{self, Dataset},
    process::Tools,
};

pub struct Success {
    pub record: Value,
    pub invocation: BuildInvocation,
    pub build_id: String,
    pub maps: Vec<PathBuf>,
}

pub struct Output {
    pub destination: PathBuf,
    pub staging: PathBuf,
    directory: Option<PathBuf>,
    stage: tempfile::TempDir,
}

impl Output {
    pub fn prepare(
        common: &Common,
        dataset: &Dataset,
        phase: &str,
        target: &str,
        cwd: &Path,
    ) -> Result<Self> {
        let (destination, directory) = if let Some(output) = &common.output {
            (cwd.join(output), None)
        } else {
            let directory = dataset.root.join(phase);
            (
                directory.join(if target.contains("windows") {
                    "app.exe"
                } else {
                    "app"
                }),
                Some(directory),
            )
        };
        let parent = match &directory {
            Some(_) => &dataset.root,
            None => destination
                .parent()
                .ok_or("binary output needs a parent directory")?,
        };
        fs::create_dir_all(parent)?;
        let stage = tempfile::Builder::new()
            .prefix(".pgo-build-")
            .tempdir_in(parent)?;
        let staging = stage.path().join(
            destination
                .file_name()
                .ok_or("binary output needs a file name")?,
        );
        Ok(Self {
            destination,
            staging,
            directory,
            stage,
        })
    }

    pub fn publish(self) -> Result<PathBuf> {
        if let Some(directory) = &self.directory {
            data::publish_directory(self.stage, directory)?;
        } else {
            // Executable companions are produced by the normal driver with the same basename.
            let basename = self
                .destination
                .file_name()
                .ok_or("binary output needs a filename")?
                .as_encoded_bytes();
            for entry in fs::read_dir(self.stage.path())? {
                let entry = entry?;
                let name = entry.file_name();
                let bytes = name.as_encoded_bytes();
                if bytes == basename
                    || (bytes.starts_with(basename) && bytes.get(basename.len()) == Some(&b'.'))
                {
                    let destination = self.destination.with_file_name(name);
                    replace(&entry.path(), &destination)?;
                }
            }
        }
        Ok(self.destination)
    }
}

fn replace(source: &Path, destination: &Path) -> Result<()> {
    let backup = if destination.try_exists()? {
        let backup = tempfile::tempdir_in(destination.parent().ok_or("output needs a parent")?)?;
        fs::rename(destination, backup.path().join("previous"))?;
        Some(backup)
    } else {
        None
    };
    if let Err(error) = fs::rename(source, destination) {
        if let Some(backup) = backup
            && let Err(restore) = fs::rename(backup.path().join("previous"), destination)
        {
            let path = backup.keep();
            return Err(format!(
                "publish failed: {error}; restore failed: {restore}; previous output is in {}",
                path.display()
            )
            .into());
        }
        return Err(error.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    tools: &Tools,
    options: &BuildOptions,
    common: &Common,
    output: &Output,
    cwd: &Path,
    enable_gc_stress: bool,
    enable_startup_stats: bool,
) -> Result<Success> {
    let mut command = Command::new(&tools.driver);
    command
        .current_dir(cwd)
        .arg("--sysroot")
        .arg(&tools.sysroot)
        .arg("--package-manager")
        .arg(&tools.package_manager);
    if enable_gc_stress {
        command.arg("--enable-gc-stress");
    }
    if enable_startup_stats {
        command.arg("--enable-startup-stats");
    }
    command
        .arg("build")
        .args(options.arguments())
        .arg("-o")
        .arg(&output.staging);
    if let Some(emit) = common.emit {
        command.arg("--emit").arg(
            emit.to_possible_value()
                .expect("all emit modes have CLI values")
                .get_name(),
        );
    }
    if let Some(directory) = &common.dump_dir {
        command.arg("--dump-dir").arg(directory);
    }
    if let Some(scope) = common.dump_scope {
        command.arg("--dump-scope").arg(
            scope
                .to_possible_value()
                .expect("all dump scopes have CLI values")
                .get_name(),
        );
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::piped())
        .scoop_spawn()?;
    let mut stderr = BufReader::new(child.stderr.take().ok_or("driver stderr pipe is missing")?);
    let mut records = Vec::new();
    let mut line = Vec::new();
    while stderr.read_until(b'\n', &mut line)? != 0 {
        match serde_json::from_slice::<Value>(&line) {
            Ok(value) if value.get("invocation").is_some() => records.push(value),
            _ => std::io::stderr().write_all(&line)?,
        }
        line.clear();
    }
    let status = child.wait()?;
    if !status.success() {
        return Err(format!(
            "build failed ({status}); no training program was started or output replaced"
        )
        .into());
    }
    if records.len() > 1 {
        return Err("PGO requires exactly one executable root; use -p group:name".into());
    }
    let record = records
        .pop()
        .ok_or("paired driver did not return a successful build record")?;
    if record["schema"] != 1 || record["kind"] != "executable" {
        return Err("PGO requires an executable source root; library and native-app roots cannot be trained by this command".into());
    }
    let invocation = serde_json::from_value(record["invocation"].clone())?;
    let build_id = record["link_plan_fingerprint"]
        .as_str()
        .ok_or("successful build is missing its link fingerprint")?
        .to_owned();
    let maps = record["profiling"]["maps"]
        .as_array()
        .map(|maps| maps.iter().map(read_path).collect())
        .transpose()?
        .unwrap_or_default();
    Ok(Success {
        record,
        invocation,
        build_id,
        maps,
    })
}

fn read_path(value: &Value) -> Result<PathBuf> {
    if let Some(path) = value.as_str() {
        return Ok(PathBuf::from(path));
    }
    #[cfg(unix)]
    if value["encoding"] == "unix-bytes" {
        use std::os::unix::ffi::OsStringExt;
        let hex = value["hex"]
            .as_str()
            .ok_or("invalid native path in build result")?;
        if !hex.len().is_multiple_of(2) {
            return Err("invalid native path byte count".into());
        }
        let bytes = hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|chunk| Ok(u8::from_str_radix(std::str::from_utf8(chunk)?, 16)?))
            .collect::<Result<Vec<_>>>()?;
        return Ok(std::ffi::OsString::from_vec(bytes).into());
    }
    #[cfg(windows)]
    if value["encoding"] == "windows-wide" {
        use std::os::windows::ffi::OsStringExt;
        let units: Vec<u16> = serde_json::from_value(value["units"].clone())?;
        return Ok(std::ffi::OsString::from_wide(&units).into());
    }
    Err("build result contains an unsupported host path encoding".into())
}
