use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use scoop_process::CommandExt;

use crate::{Result, args::Cli, data::SavedBuild};

pub struct Tools {
    pub driver: PathBuf,
    pub sysroot: PathBuf,
    pub package_manager: PathBuf,
    llvm_profdata: Option<PathBuf>,
}

impl Tools {
    pub fn resolve(cli: &Cli, saved: Option<&SavedBuild>, cwd: &Path) -> Result<Self> {
        let exe = std::env::current_exe()?;
        let install = exe
            .parent()
            .and_then(Path::parent)
            .ok_or("cannot locate command installation")?;
        let driver = cli
            .scoop
            .clone()
            .or_else(|| std::env::var_os("SCOOP_DRIVER").map(PathBuf::from))
            .or(saved.map(|saved| saved.driver.to_path_buf()).transpose()?)
            .unwrap_or_else(|| {
                install
                    .join("bin")
                    .join(if cfg!(windows) { "sco.exe" } else { "scoop" })
            });
        let driver = cwd.join(driver);
        let sysroot = cli
            .sysroot
            .clone()
            .or_else(|| std::env::var_os("SCOOP_SYSROOT").map(PathBuf::from))
            .or(saved
                .map(|saved| saved.invocation.sysroot.to_path_buf())
                .transpose()?)
            .unwrap_or_else(|| install.to_owned());
        let same_driver =
            saved.is_some_and(|saved| saved.driver.to_path_buf().is_ok_and(|path| path == driver));
        let package_manager = cli
            .package_manager
            .clone()
            .or(if same_driver {
                saved
                    .map(|saved| saved.invocation.package_manager.to_path_buf())
                    .transpose()?
            } else {
                None
            })
            .unwrap_or_else(|| {
                driver.with_file_name(scoop_toolchain::host_executable_name("scoop-pm"))
            });
        Ok(Self {
            driver,
            sysroot: cwd.join(sysroot),
            package_manager: cwd.join(package_manager),
            llvm_profdata: cli.llvm_profdata.as_ref().map(|path| cwd.join(path)),
        })
    }

    pub fn profdata(&self) -> Result<PathBuf> {
        if let Some(explicit) = &self.llvm_profdata {
            return Ok(explicit.clone());
        }
        for candidate in [
            self.sysroot
                .join("libexec/llvm/bin")
                .join(scoop_toolchain::host_executable_name("llvm-profdata")),
            self.sysroot
                .join("bin")
                .join(scoop_toolchain::host_executable_name("scoop-llvm-profdata")),
        ] {
            if candidate.try_exists()? {
                return Ok(candidate);
            }
        }
        Err(format!("paired llvm-profdata is missing from {}; install the matching host tools, or use --llvm-profdata for a development build", self.sysroot.display()).into())
    }
}

pub fn merge(tools: &Tools, raw: &[PathBuf], output: &Path) -> Result<()> {
    let status = Command::new(tools.profdata()?)
        .args(["merge", "--failure-mode=any", "--sparse=false", "-o"])
        .arg(output)
        .args(raw)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .scoop_spawn()?
        .wait()?;
    if !status.success() {
        return Err(
            format!("profile merge failed ({status}); previous merged data was retained").into(),
        );
    }
    Ok(())
}

pub fn summary(compiler: &Path, index: &Path, output: &Path) -> Result<()> {
    let result = Command::new(compiler)
        .arg("profile-summary")
        .arg(index)
        .args(["--message-format", "json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .scoop_spawn()?
        .wait_with_output()?;
    if !result.status.success() {
        use std::io::Write;
        std::io::stderr().write_all(&result.stderr)?;
        return Err(format!("profile summary failed ({})", result.status).into());
    }
    let _: serde_json::Value = serde_json::from_slice(&result.stdout)?;
    fs::write(output, &result.stdout)?;
    Ok(())
}
