use super::*;
use std::ffi::{OsStr, OsString};

impl WindowsFinalLinkProfile {
    pub fn command(&self, scratch: &Path, output: &Path) -> Result<Command, ToolchainError> {
        self.command_with_subsystem(scratch, output, scoop_identity::WindowsSubsystem::Console)
    }

    pub fn command_with_subsystem(
        &self,
        scratch: &Path,
        output: &Path,
        subsystem: scoop_identity::WindowsSubsystem,
    ) -> Result<Command, ToolchainError> {
        std::fs::create_dir_all(scratch).map_err(error)?;
        let environment = self.startup.windows_environment().map_err(error)?;
        let mut command = if self.microsoft() {
            let mut command = environment.bare_command(&self.data.driver);
            command.args(["--target=x86_64-pc-windows-msvc", "/nologo", "/MD"]);
            // LLVM's MSVC driver resolves -fuse-ld by basename. -B gives the
            // exact selected directory priority, including on cross hosts.
            command.arg(joined(
                "-fuse-ld=",
                self.data.linker.file_name().expect("linker filename"),
            ));
            command.arg(joined(
                "/clang:-B",
                self.data
                    .linker
                    .parent()
                    .expect("linker directory")
                    .as_os_str(),
            ));
            let mut libraries = OsString::new();
            for (index, path) in environment.libraries.iter().enumerate() {
                if index != 0 {
                    libraries.push(";");
                }
                libraries.push(dunce::simplified(path));
            }
            // A selected LIB prevents Clang from discovering a different SDK.
            command.env("LIB", libraries);
            for path in &environment.libraries {
                self.linker_arg(
                    &mut command,
                    joined("/libpath:", dunce::simplified(path).as_os_str()),
                );
            }
            command.arg(joined("/Fe", dunce::simplified(output).as_os_str()));
            command
        } else {
            let mut command = self.startup.driver_command_with(&self.data.driver);
            command.args([
                "-fuse-ld=lld",
                "-rtlib=compiler-rt",
                "-unwindlib=none",
                "-static",
                "-municode",
            ]);
            command.arg(joined(
                "--ld-path=",
                dunce::simplified(&self.data.linker).as_os_str(),
            ));
            if self.cxx() {
                command.arg("-stdlib=libc++");
            }
            command.arg("-o").arg(dunce::simplified(output));
            command
        };
        command
            .current_dir(dunce::simplified(scratch))
            .env("VSLANG", "1033")
            .env("TEMP", scratch)
            .env("TMP", scratch)
            .env("TMPDIR", scratch);
        for option in self.linker_args() {
            self.linker_arg(&mut command, option);
        }
        if self.microsoft() {
            self.linker_arg(
                &mut command,
                match subsystem {
                    scoop_identity::WindowsSubsystem::Console => "/subsystem:console,10.0",
                    scoop_identity::WindowsSubsystem::Gui => "/subsystem:windows,10.0",
                },
            );
            // Both PE subsystems enter the Unicode console-style C function.
            self.linker_arg(&mut command, "/entry:wmainCRTStartup");
        } else {
            command.arg(match subsystem {
                scoop_identity::WindowsSubsystem::Console => "-mconsole",
                scoop_identity::WindowsSubsystem::Gui => "-mwindows",
            });
        }
        Ok(command)
    }

    /// One linker argument, without comma splitting or shell interpretation.
    pub fn linker_arg(&self, command: &mut Command, argument: impl AsRef<OsStr>) {
        if self.microsoft() {
            command
                .arg("/clang:-Xlinker")
                .arg(joined("/clang:", argument.as_ref()));
        } else {
            command.arg("-Xlinker").arg(argument);
        }
    }

    pub fn append_system_libraries(&self, command: &mut Command) {
        command.arg(dunce::simplified(
            &self.data.unwind_prefix.join("lib/libunwind.a"),
        ));
    }

    pub fn configure_outputs(&self, command: &mut Command, pdb: Option<&Path>, map: Option<&Path>) {
        if let Some(pdb) = pdb {
            if self.microsoft() {
                self.linker_arg(command, "/debug:full");
                self.linker_arg(command, joined("/pdb:", dunce::simplified(pdb).as_os_str()));
                self.linker_arg(command, "/pdbaltpath:%_PDB%");
            } else {
                self.linker_arg(
                    command,
                    joined("--pdb=", dunce::simplified(pdb).as_os_str()),
                );
                self.linker_arg(
                    command,
                    joined(
                        "--Xlink=/pdbaltpath:",
                        pdb.file_name().expect("PDB filename"),
                    ),
                );
            }
        } else if !self.microsoft() {
            self.linker_arg(command, "--strip-all");
        }
        if let Some(map) = map {
            self.linker_arg(
                command,
                joined(
                    if self.microsoft() { "/map:" } else { "-Map=" },
                    dunce::simplified(map).as_os_str(),
                ),
            );
        }
    }

    pub fn trace(&self, command: &mut Command) {
        self.linker_arg(
            command,
            match self.data.linker_kind {
                LinkerKind::Microsoft => "/verbose:lib",
                LinkerKind::LldCoff => "/verbose",
                LinkerKind::LldMinGw => "--verbose",
            },
        );
    }

    /// Actual libraries observed by this link, for the executable update record.
    pub fn traced_inputs(&self, output: &std::process::Output, scratch: &Path) -> Vec<PathBuf> {
        let mut paths = BTreeSet::new();
        for bytes in [&output.stdout, &output.stderr] {
            for line in String::from_utf8_lossy(bytes).lines() {
                let text = line
                    .trim()
                    .split_once(": Reading ")
                    .map(|(_, path)| path)
                    .or_else(|| line.trim().strip_prefix("Searching "))
                    .unwrap_or(line.trim())
                    .trim_end_matches(':');
                let text = text
                    .split_once(".a(")
                    .map(|(path, _)| format!("{path}.a"))
                    .or_else(|| {
                        text.split_once(".lib(")
                            .map(|(path, _)| format!("{path}.lib"))
                    })
                    .unwrap_or_else(|| text.to_owned());
                let path = Path::new(&text);
                if path.is_absolute() && !path.starts_with(scratch) && path.is_file() {
                    paths.insert(path.to_owned());
                }
            }
        }
        paths.into_iter().collect()
    }
}

fn joined(prefix: &str, value: &OsStr) -> OsString {
    let mut result = OsString::from(prefix);
    result.push(value);
    result
}
