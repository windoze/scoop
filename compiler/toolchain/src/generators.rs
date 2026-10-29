use crate::ToolchainError;
use scoop_protocol::{GeneratorRegistrationV1, HostPathCarrier};
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

mod inputs;
pub use inputs::record_generator_input;

/// Explicit host programs, resolved once against the receiving CLI's cwd.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GeneratorPrograms {
    explicit: BTreeMap<String, PathBuf>,
}

impl GeneratorPrograms {
    pub fn from_cli(options: &[OsString], cwd: &Path) -> Result<Self, ToolchainError> {
        let mut programs = Self::default();
        for option in options {
            let bytes = option.as_encoded_bytes();
            let Some(separator) = bytes.iter().position(|&byte| byte == b'=') else {
                return Err(error("generator registration must be NAME=PATH"));
            };
            let name = std::str::from_utf8(&bytes[..separator])
                .map_err(|_| error("generator name must be valid UTF-8"))?;
            // Splitting at ASCII '=' preserves the platform OsStr encoding boundary.
            let path = unsafe { OsStr::from_encoded_bytes_unchecked(&bytes[separator + 1..]) };
            if name.trim().is_empty() || path.is_empty() {
                return Err(error(
                    "generator registration requires a nonempty name and path",
                ));
            }
            if programs
                .explicit
                .insert(name.to_owned(), cwd.join(path))
                .is_some()
            {
                return Err(error(format!("duplicate generator registration `{name}`")));
            }
        }
        Ok(programs)
    }

    pub fn from_registrations(
        registrations: &[GeneratorRegistrationV1],
    ) -> Result<Self, ToolchainError> {
        let explicit = registrations
            .iter()
            .map(|registration| {
                let path = registration.program().to_path_buf().map_err(error)?;
                if !path.is_absolute() {
                    return Err(error(
                        "generator program path must be absolute in a machine request",
                    ));
                }
                Ok((registration.name().to_owned(), path))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { explicit })
    }

    pub fn registrations(&self) -> Result<Vec<GeneratorRegistrationV1>, ToolchainError> {
        self.explicit
            .iter()
            .map(|(name, path)| {
                GeneratorRegistrationV1::new(
                    name.clone(),
                    HostPathCarrier::from_path(path).map_err(error)?,
                )
                .map_err(error)
            })
            .collect()
    }

    pub fn program_for_compiler(&self, name: &str, compiler: &Path) -> Option<PathBuf> {
        self.explicit.get(name).cloned().or_else(|| {
            (name == "scoop.codec").then(|| {
                compiler.with_file_name(crate::host_executable_name("scoop-codec-generator"))
            })
        })
    }

    /// Resolve only a generator selected by an actual compilation task.
    pub fn program(&self, name: &str) -> Result<Option<PathBuf>, ToolchainError> {
        if let Some(path) = self.explicit.get(name) {
            return Ok(Some(path.clone()));
        }
        if name == "scoop.codec" {
            return crate::installed_host_tool("scoop-codec-generator").map(Some);
        }
        Ok(None)
    }
}

fn error(message: impl std::fmt::Display) -> ToolchainError {
    ToolchainError(message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registrations_preserve_paths_without_probing_unused_programs() {
        let cwd = crate::development_workspace_root();
        let absolute = cwd.join("unbuilt/other=generator");
        let options = [OsString::from("one=unbuilt/my generator"), {
            let mut value = OsString::from("two=");
            value.push(&absolute);
            value
        }];
        let programs = GeneratorPrograms::from_cli(&options, &cwd).unwrap();
        assert_eq!(
            programs.program("one").unwrap(),
            Some(cwd.join("unbuilt/my generator"))
        );
        assert_eq!(programs.program("two").unwrap(), Some(absolute));
        assert_eq!(programs.program("unused").unwrap(), None);
    }

    #[test]
    fn invalid_or_duplicate_explicit_registrations_are_rejected() {
        let cwd = crate::development_workspace_root();
        for options in [
            vec!["missing-equals"],
            vec![" =program"],
            vec!["name="],
            vec!["same=a", "same=b"],
        ] {
            let options = options.into_iter().map(OsString::from).collect::<Vec<_>>();
            assert!(GeneratorPrograms::from_cli(&options, &cwd).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn generator_paths_keep_non_utf8_host_bytes() {
        use std::os::unix::ffi::OsStringExt;
        let cwd = crate::development_workspace_root();
        let programs = GeneratorPrograms::from_cli(
            &[OsString::from_vec(b"bytes=generator-\xff".to_vec())],
            &cwd,
        )
        .unwrap();
        assert_eq!(
            programs.program("bytes").unwrap(),
            Some(cwd.join(OsString::from_vec(b"generator-\xff".to_vec())))
        );
    }
}
