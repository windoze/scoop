use super::*;
use crate::ValidatedCxxToolchain;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) enum RuntimeLibraries {
    C {
        #[serde(with = "crate::build_record::values::path")]
        unwind_prefix: PathBuf,
    },
    Cxx(ValidatedCxxToolchain),
}

impl RuntimeLibraries {
    pub(super) fn resolve(
        target: scoop_lir::LirTargetProfile,
        options: &FinalLinkOptions,
        cxx: Option<ValidatedCxxToolchain>,
    ) -> Result<Self, ToolchainError> {
        if let Some(cxx) = cxx {
            return Ok(Self::Cxx(cxx));
        }
        let prefix = crate::selected_unwind_prefix(
            target,
            options.sysroot.as_deref(),
            options.unwind_prefix.as_deref(),
        )?;
        crate::runtime_unwind_include(target, Some(&prefix))?;
        let unwind_prefix = std::path::absolute(prefix).map_err(error)?;
        let archive = unwind_prefix.join("lib/libunwind.a");
        if !archive.is_file() {
            return Err(error(format!(
                "missing LLVM unwind archive {}; run scripts/build_llvm_unwind.py for {}",
                archive.display(),
                target.contract().canonical_triple()
            )));
        }
        Ok(Self::C { unwind_prefix })
    }

    pub(super) fn cxx(&self) -> bool {
        matches!(self, Self::Cxx(_))
    }

    pub(super) fn command(&self, startup: &ValidatedCBridgeToolchainInvocation) -> Command {
        match self {
            Self::C { .. } => startup.driver_command(),
            Self::Cxx(cxx) => cxx.command(startup),
        }
    }

    pub(super) fn driver<'a>(
        &'a self,
        startup: &'a ValidatedCBridgeToolchainInvocation,
    ) -> &'a Path {
        match self {
            Self::C { .. } => startup.compiler_driver(),
            Self::Cxx(cxx) => cxx.driver(),
        }
    }

    pub(super) fn unwind_prefix(&self) -> Option<&Path> {
        match self {
            Self::C { unwind_prefix } => Some(unwind_prefix),
            Self::Cxx(_) => None,
        }
    }
}
