//! Reuse the completed native link discovery across actual program relinks.
use super::*;
use crate::build_record::{self, Input, Record};
use scoop_protocol::HostPathCarrier;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const CACHE_DIRECTORY: &str = "final-tools-v6";

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests;

#[derive(Serialize, Deserialize)]
enum Discovery {
    Darwin(darwin::Discovery),
    Linux(linux::Discovery),
    Windows(windows::Discovery),
}

pub(super) fn resolve(
    target: &ResolvedTargetProfile,
    options: &FinalLinkOptions,
    cxx: bool,
    cache: &Path,
) -> Result<ValidatedFinalLinkProfile, ToolchainError> {
    let path = |value: &Path| {
        HostPathCarrier::from_path(&std::path::absolute(value).map_err(error)?).map_err(error)
    };
    let unwind = (target.c_bridge_toolchain().native_environment()
        != scoop_target_data::NativeEnvironment::Darwin
        && (!cxx
            || target
                .c_bridge_toolchain()
                .native_environment()
                .is_windows()))
    .then(|| {
        crate::selected_unwind_prefix(
            target.lir_target(),
            options.sysroot.as_deref(),
            options.unwind_prefix.as_deref(),
        )
    })
    .transpose()?
    .as_deref()
    .map(path)
    .transpose()?;
    let key = build_record::fingerprint(&(
        crate::native_record::ResolvedRecord::from_target(target)?,
        options.mode,
        options.sysroot.as_deref().map(path).transpose()?,
        options.unwind_prefix.as_deref().map(path).transpose()?,
        options.linker.as_deref().map(path).transpose()?,
        unwind,
        options.native_roots(target.lir_target())?,
        cxx,
    ))?;
    let record = build_record::cached(
        &cache.join(CACHE_DIRECTORY),
        key,
        |_| Ok(true),
        || {
            let cxx = cxx
                .then(|| crate::ValidatedCxxToolchain::resolve_cached(target, cache))
                .transpose()?;
            let profile = ValidatedFinalLinkProfile::from_tools(
                target.c_bridge_toolchain().clone(),
                options,
                cxx,
            )?;
            let paths: BTreeSet<_> = profile.input_paths()?.into_iter().collect();
            let inputs = paths
                .iter()
                .map(|p| Input::read(p))
                .collect::<Result<_, _>>()?;
            let result = match profile {
                ValidatedFinalLinkProfile::Darwin(profile) => {
                    Discovery::Darwin(profile.into_discovery())
                }
                ValidatedFinalLinkProfile::Linux(profile) => {
                    Discovery::Linux(profile.into_discovery())
                }
                ValidatedFinalLinkProfile::Windows(profile) => {
                    Discovery::Windows(profile.into_discovery())
                }
            };
            Ok(Record { inputs, result })
        },
    )?;
    let startup = target.c_bridge_toolchain().clone();
    match record.result {
        Discovery::Darwin(data) => Ok(ValidatedFinalLinkProfile::Darwin(
            DarwinFinalLinkProfile::from_discovery(startup, data),
        )),
        Discovery::Linux(data) => LinuxFinalLinkProfile::from_discovery(startup, data)
            .map(ValidatedFinalLinkProfile::Linux),
        Discovery::Windows(data) => Ok(ValidatedFinalLinkProfile::Windows(
            WindowsFinalLinkProfile::from_discovery(startup, data),
        )),
    }
}
