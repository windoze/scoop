use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviousExport {
    pub install_name: String,
    pub compatibility_version: u32,
    pub interface: NativeExport,
}

#[derive(Default)]
pub(in crate::system_provider) struct StubDirectives {
    pub install_name: Option<String>,
    pub compatibility_version: Option<u32>,
    pub previous: BTreeMap<String, PreviousExport>,
}

impl StubDirectives {
    pub(super) fn apply(
        &mut self,
        exports: &mut BTreeMap<String, NativeExport>,
        directive: &str,
        deployment: DarwinPackedVersionV1,
    ) -> Result<(), ToolchainError> {
        if let Some(previous) = directive.strip_prefix("$ld$previous$") {
            return self.previous(exports, previous, deployment);
        }
        let mut parts = directive.trim_start_matches("$ld$").splitn(3, '$');
        let operation = parts.next().unwrap_or_default();
        let condition = parts.next().unwrap_or_default();
        let symbol = parts.next().unwrap_or_default();
        let version = condition
            .strip_prefix("os")
            .ok_or_else(|| invalid(directive))?;
        let version = parse_version(version)?;
        if version & !0xff != deployment.packed() & !0xff {
            return Ok(());
        }
        match operation {
            "hide" => {
                exports.remove(symbol);
            }
            "add" | "weak" => insert(
                exports,
                symbol.to_owned(),
                NativeExport {
                    kind: SystemExportKind::Symbol,
                    weak: operation == "weak",
                },
            )?,
            "install_name" => self.install_name = Some(symbol.to_owned()),
            "compatibility_version" => {
                self.compatibility_version = Some(parse_compatibility(symbol)?)
            }
            _ => return Err(invalid(directive)),
        }
        Ok(())
    }

    fn previous(
        &mut self,
        exports: &mut BTreeMap<String, NativeExport>,
        directive: &str,
        deployment: DarwinPackedVersionV1,
    ) -> Result<(), ToolchainError> {
        let fields: Vec<_> = directive.splitn(6, '$').collect();
        let [name, compatibility, platform, start, end, symbol] = fields.as_slice() else {
            return Err(invalid(directive));
        };
        let symbol = symbol.strip_suffix('$').ok_or_else(|| invalid(directive))?;
        let platform: u32 = platform.parse().map_err(|_| invalid(directive))?;
        let start = parse_version(start)?;
        let end = parse_version(end)?;
        let compatibility = parse_compatibility(compatibility)?;
        if name.is_empty() || start >= end {
            return Err(invalid(directive));
        }
        // Mach-O platform 1 is macOS; the interval includes start and excludes end.
        if platform != 1 || deployment.packed() < start || deployment.packed() >= end {
            return Ok(());
        }
        if symbol.is_empty() {
            self.install_name = Some((*name).to_owned());
            self.compatibility_version = Some(compatibility);
        } else {
            let interface = *exports.entry(symbol.to_owned()).or_insert(NativeExport {
                kind: SystemExportKind::Symbol,
                weak: false,
            });
            self.previous.insert(
                symbol.to_owned(),
                PreviousExport {
                    install_name: (*name).to_owned(),
                    compatibility_version: compatibility,
                    interface,
                },
            );
        }
        Ok(())
    }
}

fn parse_version(value: &str) -> Result<u32, ToolchainError> {
    let value = if value.contains('.') {
        value.to_owned()
    } else {
        format!("{value}.0")
    };
    crate::c_bridge::parse_darwin_version(&value, "SDK linker directive")
        .map(|version| version.packed())
}

fn parse_compatibility(value: &str) -> Result<u32, ToolchainError> {
    match value {
        "" | "0" | "0.0" | "0.0.0" => Ok(0),
        _ => parse_version(value),
    }
}

fn invalid(value: &str) -> ToolchainError {
    ToolchainError(format!("invalid SDK linker directive {value}"))
}
