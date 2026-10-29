use serde::Deserialize;
use toml::Spanned;

use super::{NativeCompileFlag, NativeConfig, NativeIncludeFlag, flags};
use crate::{ManifestParseError, ManifestParseErrorKind};
use std::path::PathBuf;

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawNativeConfig {
    #[serde(default)]
    sources: Vec<crate::selection::RawConditionalPath>,
    #[serde(default)]
    include: Vec<Spanned<super::include::RawInclude>>,
    #[serde(default)]
    c_flags: Vec<Spanned<String>>,
    #[serde(default)]
    cxx_flags: Vec<Spanned<String>>,
    #[serde(default)]
    objc_flags: Vec<Spanned<String>>,
    #[serde(default)]
    cxx: bool,
    #[serde(default)]
    libraries: Vec<super::libraries::RawNativeLibrary>,
}

impl RawNativeConfig {
    pub(crate) fn parse(
        self,
        catalog: &[crate::SourceTarget<'_>],
    ) -> Result<NativeConfig, ManifestParseError> {
        Ok(NativeConfig {
            sources: self
                .sources
                .into_iter()
                .map(|raw| crate::selection::parse_native_path(raw, catalog))
                .collect::<Result<_, _>>()?,
            include: self
                .include
                .into_iter()
                .map(|raw| super::include::parse(raw, catalog))
                .collect::<Result<_, _>>()?,
            c_flags: parse_flags(self.c_flags)?,
            cxx_flags: parse_flags(self.cxx_flags)?,
            objc_flags: parse_flags(self.objc_flags)?,
            cxx: self.cxx,
            libraries: self
                .libraries
                .into_iter()
                .map(|raw| super::libraries::parse(raw, catalog))
                .collect::<Result<_, _>>()?,
        })
    }
}

fn parse_path(value: Spanned<String>) -> Result<PathBuf, ManifestParseError> {
    native_path(value.get_ref()).map_err(|reason| error(&value, reason))
}

pub(super) fn native_path(value: &str) -> Result<PathBuf, &'static str> {
    if value.is_empty() {
        return Err("path must not be empty");
    }
    if value.contains('\0') {
        return Err("path must not contain NUL");
    }
    Ok(PathBuf::from(value))
}

fn parse_flags(flags: Vec<Spanned<String>>) -> Result<Vec<NativeCompileFlag>, ManifestParseError> {
    let mut flags = flags.into_iter();
    let mut result = Vec::new();
    while let Some(flag) = flags.next() {
        let argument = flag.get_ref();
        if (!argument.starts_with('-') && !argument.starts_with('/'))
            || matches!(argument.as_str(), "-" | "/")
            || argument.contains('\0')
        {
            return Err(error(&flag, "expected one compiler option"));
        }
        if flags::driver_managed(argument) {
            return Err(error(&flag, "option is managed by the native driver"));
        }
        if let Some((kind, suffix)) = NativeIncludeFlag::split(argument) {
            let path = if suffix.is_empty() {
                parse_path(
                    flags
                        .next()
                        .ok_or_else(|| error(&flag, "include option requires a path"))?,
                )?
            } else {
                native_path(suffix).map_err(|reason| error(&flag, reason))?
            };
            result.push(NativeCompileFlag::Include { kind, path });
        } else if matches!(argument.as_str(), "-D" | "-U" | "/D" | "/U") {
            let value = flags
                .next()
                .ok_or_else(|| error(&flag, "macro option requires a value"))?;
            if value.get_ref().is_empty() || value.get_ref().contains('\0') {
                return Err(error(&value, "invalid macro option value"));
            }
            result.push(NativeCompileFlag::Argument(format!(
                "{argument}{}",
                value.get_ref()
            )));
        } else {
            result.push(NativeCompileFlag::Argument(flag.into_inner()));
        }
    }
    Ok(result)
}

fn error(value: &Spanned<String>, reason: &str) -> ManifestParseError {
    ManifestParseError::new(
        ManifestParseErrorKind::InvalidNative(format!(
            "invalid native option {:?}: {reason}",
            value.get_ref()
        )),
        Some(value.span()),
    )
}
