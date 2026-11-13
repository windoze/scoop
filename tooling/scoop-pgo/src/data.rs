use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use fs4::FileExt;
use scoop::options::{BuildInvocation, CommandArgument};
use scoop_protocol::HostPathCarrier;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::Result;

pub const SCHEMA: u32 = 1;

pub struct Dataset {
    pub root: PathBuf,
    _lock: File,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedBuild {
    pub schema: u32,
    pub driver: HostPathCarrier,
    pub invocation: BuildInvocation,
    pub build_id: String,
    pub maps: Vec<PathBuf>,
    pub output: HostPathCarrier,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Run {
    pub schema: u32,
    pub build_id: String,
    pub maps: Vec<PathBuf>,
    pub cwd: HostPathCarrier,
    pub arguments: Vec<CommandArgument>,
    pub status: Option<RunStatus>,
    /// Paths relative to this run, including files already seen from external runs.
    pub raw: Vec<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunStatus {
    pub success: bool,
    pub code: Option<i32>,
    pub signal: Option<i32>,
}

impl RunStatus {
    pub fn from_status(status: std::process::ExitStatus) -> Self {
        #[cfg(unix)]
        let signal = {
            use std::os::unix::process::ExitStatusExt;
            status.signal()
        };
        #[cfg(not(unix))]
        let signal = None;
        Self {
            success: status.success(),
            code: status.code(),
            signal,
        }
    }
}

impl Dataset {
    pub fn open(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join(".lock"))?;
        FileExt::lock(&lock)?;
        Ok(Self { root, _lock: lock })
    }

    pub fn saved(&self) -> Result<Option<SavedBuild>> {
        let path = self.root.join("build.json");
        if !path.try_exists()? {
            return Ok(None);
        }
        let saved: SavedBuild = read_json(&path)?;
        check_schema(saved.schema)?;
        Ok(Some(saved))
    }

    pub fn new_run(&self) -> Result<PathBuf> {
        let directory = self.root.join("runs");
        fs::create_dir_all(&directory)?;
        Ok(tempfile::Builder::new()
            .prefix("run-")
            .tempdir_in(directory)?
            .keep())
    }

    pub fn save_maps(&self, id: &str, sources: &[PathBuf]) -> Result<Vec<PathBuf>> {
        if id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("build result has an invalid link fingerprint".into());
        }
        let relative = PathBuf::from("builds").join(id);
        let destination = self.root.join(&relative);
        let maps = (0..sources.len())
            .map(|index| relative.join(format!("{index}.profile.json")))
            .collect::<Vec<_>>();
        if destination.try_exists()? {
            return Ok(maps);
        }
        fs::create_dir_all(self.root.join("builds"))?;
        let stage = tempfile::tempdir_in(self.root.join("builds"))?;
        for (index, source) in sources.iter().enumerate() {
            fs::copy(source, stage.path().join(format!("{index}.profile.json")))?;
        }
        fs::rename(stage.path(), destination)?;
        Ok(maps)
    }
}

pub fn check_schema(schema: u32) -> Result<()> {
    if schema != SCHEMA {
        return Err(format!("unsupported PGO data schema {schema}").into());
    }
    Ok(())
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_reader(File::open(path).map_err(|e| format!("{}: {e}", path.display()))?)
        .map_err(|e| format!("{}: {e}", path.display()).into())
}

pub fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path
        .parent()
        .ok_or("JSON output needs a parent directory")?;
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut temp, value)?;
    temp.write_all(b"\n")?;
    temp.flush()?;
    // tempfile publishes through native APIs, which require an extended Windows path.
    temp.persist(
        fs::canonicalize(parent)?.join(path.file_name().ok_or("JSON output needs a filename")?),
    )?;
    Ok(())
}

pub fn path(path: &Path) -> Result<HostPathCarrier> {
    Ok(HostPathCarrier::from_path(path)?)
}

pub fn raw_files(run: &Path) -> Result<Vec<PathBuf>> {
    fn visit(root: &Path, directory: &Path, output: &mut BTreeSet<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                visit(root, &entry.path(), output)?;
            } else if entry
                .path()
                .extension()
                .is_some_and(|value| value == "profraw")
            {
                output.insert(entry.path().strip_prefix(root)?.to_owned());
            }
        }
        Ok(())
    }
    let mut output = BTreeSet::new();
    visit(run, run, &mut output)?;
    Ok(output.into_iter().collect())
}

/// Replace an output directory while retaining the old result on publication failure.
pub fn publish_directory(stage: tempfile::TempDir, destination: &Path) -> Result<()> {
    let backup = if destination.try_exists()? {
        let backup = tempfile::tempdir_in(destination.parent().ok_or("output needs a parent")?)?;
        fs::remove_dir(backup.path())?;
        fs::rename(destination, backup.path())?;
        Some(backup)
    } else {
        None
    };
    if let Err(error) = fs::rename(stage.path(), destination) {
        if let Some(backup) = backup {
            let backup = backup.keep();
            fs::rename(&backup, destination).map_err(|restore| {
                format!(
                    "publish failed: {error}; restore failed: {restore}; previous result is at {}",
                    backup.display()
                )
            })?;
        }
        return Err(error.into());
    }
    Ok(())
}
