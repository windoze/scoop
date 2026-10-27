use scoop_identity::{RequestedConeKind, WindowsSubsystem};
use serde::Deserialize;
use toml::Spanned;

use crate::{ManifestParseError, ManifestParseErrorKind};

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WindowsConfig {
    #[serde(with = "crate::serde_record::Subsystem")]
    subsystem: WindowsSubsystem,
}

impl WindowsConfig {
    pub const fn subsystem(&self) -> WindowsSubsystem {
        self.subsystem
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawWindowsConfig {
    subsystem: Option<Spanned<String>>,
}

pub(crate) fn parse(
    raw: Option<Spanned<RawWindowsConfig>>,
    kind: RequestedConeKind,
) -> Result<WindowsConfig, ManifestParseError> {
    let Some(raw) = raw else {
        return Ok(WindowsConfig::default());
    };
    if kind != RequestedConeKind::Executable {
        return Err(ManifestParseError::new(
            ManifestParseErrorKind::InvalidWindows(
                "[windows] executable configuration requires kind = \"executable\"".into(),
            ),
            Some(raw.span()),
        ));
    }
    let Some(value) = raw.into_inner().subsystem else {
        return Ok(WindowsConfig::default());
    };
    let subsystem = match value.get_ref().as_str() {
        "console" => WindowsSubsystem::Console,
        "gui" => WindowsSubsystem::Gui,
        _ => {
            return Err(ManifestParseError::new(
                ManifestParseErrorKind::InvalidWindows(
                    "windows.subsystem must be \"console\" or \"gui\"".into(),
                ),
                Some(value.span()),
            ));
        }
    };
    Ok(WindowsConfig { subsystem })
}
