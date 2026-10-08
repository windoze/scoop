use serde::Deserialize;
use toml::Spanned;

use super::{NativeCompileFlag, NativeConfig, NativeIncludeFlag, flags};
use crate::{ConeRelativePath, ManifestParseError, ManifestParseErrorKind};

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawNativeConfig {
    #[serde(default)]
    sources: Vec<crate::selection::RawConditionalPath>,
    #[serde(default)]
    include: Vec<Spanned<String>>,
    #[serde(default)]
    c_flags: Vec<Spanned<String>>,
}

impl RawNativeConfig {
    pub(crate) fn parse(self) -> Result<NativeConfig, ManifestParseError> {
        Ok(NativeConfig {
            sources: self
                .sources
                .into_iter()
                .map(crate::selection::parse_path)
                .collect::<Result<_, _>>()?,
            include: self
                .include
                .into_iter()
                .map(parse_path)
                .collect::<Result<_, _>>()?,
            c_flags: parse_flags(self.c_flags)?,
        })
    }
}

fn parse_path(value: Spanned<String>) -> Result<ConeRelativePath, ManifestParseError> {
    ConeRelativePath::new(value.get_ref()).map_err(|reason| error(&value, reason))
}

fn parse_flags(flags: Vec<Spanned<String>>) -> Result<Vec<NativeCompileFlag>, ManifestParseError> {
    let mut flags = flags.into_iter();
    let mut result = Vec::new();
    while let Some(flag) = flags.next() {
        let argument = flag.get_ref();
        if !argument.starts_with('-') || argument == "-" || argument.contains('\0') {
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
                ConeRelativePath::new(suffix).map_err(|reason| error(&flag, reason))?
            };
            result.push(NativeCompileFlag::Include { kind, path });
        } else if matches!(argument.as_str(), "-D" | "-U") {
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
