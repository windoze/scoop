use std::ffi::OsString;

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WindowsCEnvironment {
    pub(crate) native_sysroot: Option<PathBuf>,
    pub(crate) includes: Vec<PathBuf>,
    pub(crate) libraries: Vec<PathBuf>,
    pub(crate) search_path: OsString,
    pub(crate) system_root: Option<OsString>,
}

impl WindowsCEnvironment {
    pub(super) fn resolve(
        target: LirTargetProfile,
        compiler: &Path,
        native_sysroot: Option<&Path>,
    ) -> Result<(Self, Vec<PathBuf>), ToolchainError> {
        let mut environment = Self {
            native_sysroot: native_sysroot
                .map(std::path::absolute)
                .transpose()
                .map_err(error)?,
            includes: Vec::new(),
            libraries: Vec::new(),
            search_path: std::env::var_os("PATH").unwrap_or_default(),
            system_root: std::env::var_os("SystemRoot"),
        };
        let mut inputs = Vec::new();
        if target.id() != scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID {
            if environment.native_sysroot.is_none() {
                environment.native_sysroot = Some(compiler.parent().and_then(Path::parent)
                    .ok_or_else(|| error("cannot infer LLVM-MinGW prefix; provide an explicit native sysroot"))?.to_owned());
            }
            let root = environment
                .native_sysroot
                .as_ref()
                .expect("native prefix was selected");
            environment
                .libraries
                .extend([root.join("x86_64-w64-mingw32/lib"), root.join("lib")]);
        } else if let Some(root) = &environment.native_sysroot {
            let config = root.join("msvc.toml");
            let contents = std::fs::read_to_string(&config)
                .map_err(|err| error(format!("{}: {err}", config.display())))?;
            let MsvcConfig { msvc } = toml::from_str(&contents).map_err(error)?;
            let tools = root.join(msvc.vc_tools);
            let sdk = root.join(msvc.windows_sdk);
            let include = sdk.join("Include").join(&msvc.sdk_version);
            let lib = sdk.join("Lib").join(&msvc.sdk_version);
            environment.includes = vec![
                tools.join("include"),
                include.join("ucrt"),
                include.join("shared"),
                include.join("um"),
                include.join("winrt"),
            ];
            environment.libraries = vec![
                tools.join("lib/x64"),
                lib.join("ucrt/x64"),
                lib.join("um/x64"),
            ];
            inputs.push(config);
        } else {
            environment.discover_microsoft(&mut inputs)?;
        }
        inputs.extend(environment.includes.iter().cloned());
        Ok((environment, inputs))
    }

    #[cfg(windows)]
    fn discover_microsoft(&mut self, inputs: &mut Vec<PathBuf>) -> Result<(), ToolchainError> {
        let tool = find_msvc_tools::find_tool("x86_64-pc-windows-msvc", "cl.exe")
            .ok_or_else(|| error("no Microsoft C++ Build Tools and Windows SDK found; provide an explicit native sysroot"))?;
        inputs.push(tool.path().to_owned());
        for (name, value) in tool.env() {
            if name.eq_ignore_ascii_case("INCLUDE") {
                self.includes = search_directories(value)?;
            } else if name.eq_ignore_ascii_case("LIB") {
                self.libraries = search_directories(value)?;
            } else if name.eq_ignore_ascii_case("PATH") {
                self.search_path = value.clone();
            }
        }
        Ok(())
    }

    #[cfg(not(windows))]
    fn discover_microsoft(&mut self, _: &mut Vec<PathBuf>) -> Result<(), ToolchainError> {
        Err(error(
            "Microsoft cross-compilation requires a native sysroot containing msvc.toml",
        ))
    }

    pub(crate) fn command(&self, compiler: &Path, triple: &str, microsoft: bool) -> Command {
        let mut command = Command::new(compiler);
        command
            .env_clear()
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .env("TZ", "UTC")
            .env("PATH", &self.search_path)
            .env("TEMP", std::env::temp_dir())
            .env("TMP", std::env::temp_dir())
            .env("TMPDIR", std::env::temp_dir());
        if let Some(root) = &self.system_root {
            command.env("SystemRoot", root);
        }
        self.configure(&mut command, triple, microsoft);
        command
    }

    pub(crate) fn configure(&self, command: &mut Command, triple: &str, microsoft: bool) {
        command.args([
            "-target",
            triple,
            "-march=x86-64",
            "-D_WIN32_WINNT=0x0a00",
            "-DWINVER=0x0a00",
            "-funwind-tables",
            "-fasynchronous-unwind-tables",
        ]);
        if microsoft {
            command.args(["-nostdlibinc", "-fms-runtime-lib=dll"]);
            for include in &self.includes {
                command.arg("-isystem").arg(include);
            }
        } else if let Some(root) = &self.native_sysroot {
            command.arg("--sysroot").arg(root);
        }
    }
}

#[cfg(windows)]
fn search_directories(value: &std::ffi::OsStr) -> Result<Vec<PathBuf>, ToolchainError> {
    std::env::split_paths(value)
        .filter(|path| !path.as_os_str().is_empty())
        .map(|path| std::path::absolute(path).map_err(error))
        .collect()
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct MsvcConfig {
    msvc: MsvcResources,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct MsvcResources {
    vc_tools: PathBuf,
    windows_sdk: PathBuf,
    sdk_version: String,
}
