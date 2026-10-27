use scoop_identity::{RequestedConeKind, WindowsSubsystem};
use serde::Deserialize;
use toml::Spanned;

use crate::{ConeRelativePath, ManifestParseError, ManifestParseErrorKind};

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WindowsConfig {
    #[serde(with = "crate::serde_record::Subsystem")]
    subsystem: WindowsSubsystem,
    #[serde(default)]
    rc: Option<ConeRelativePath>,
}

impl WindowsConfig {
    pub const fn subsystem(&self) -> WindowsSubsystem {
        self.subsystem
    }

    pub fn rc(&self) -> Option<&ConeRelativePath> {
        self.rc.as_ref()
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawWindowsConfig {
    subsystem: Option<Spanned<String>>,
    rc: Option<Spanned<String>>,
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
    let raw = raw.into_inner();
    let subsystem = match raw.subsystem {
        None => WindowsSubsystem::Console,
        Some(value) => match value.get_ref().as_str() {
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
        },
    };
    let rc = raw
        .rc
        .map(|value| {
            ConeRelativePath::native(value.get_ref()).map_err(|reason| {
                ManifestParseError::new(
                    ManifestParseErrorKind::InvalidWindows(format!("windows.rc: {reason}")),
                    Some(value.span()),
                )
            })
        })
        .transpose()?;
    Ok(WindowsConfig { subsystem, rc })
}
