use super::*;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(super) struct Record {
    tbd_version: u32,
    targets: Vec<String>,
    pub install_name: String,
    #[serde(default)]
    exports: Vec<Exports>,
    #[serde(default)]
    reexports: Vec<Exports>,
    #[serde(default)]
    reexported_libraries: Vec<Libraries>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
struct Exports {
    targets: Vec<String>,
    #[serde(default)]
    symbols: Vec<String>,
    #[serde(default)]
    weak_symbols: Vec<String>,
    #[serde(default)]
    thread_local_symbols: Vec<String>,
    #[serde(default)]
    objc_classes: Vec<String>,
    #[serde(default)]
    objc_eh_types: Vec<String>,
    #[serde(default)]
    objc_ivars: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct Libraries {
    targets: Vec<String>,
    libraries: Vec<String>,
}

pub(super) fn parse(bytes: &[u8]) -> Result<Vec<Record>, ToolchainError> {
    serde_yaml_ng::Deserializer::from_slice(bytes)
        .map(|document| {
            let record = Record::deserialize(document)
                .map_err(|error| ToolchainError(format!("invalid SDK text stub: {error}")))?;
            if record.tbd_version != 4 {
                return Err(ToolchainError(format!(
                    "SDK text stub version {} is not v4",
                    record.tbd_version
                )));
            }
            Ok(record)
        })
        .collect()
}

impl Record {
    pub(super) fn collect(
        &self,
        deployment: DarwinPackedVersionV1,
        exports: &mut BTreeMap<String, SystemExportKind>,
        pending: &mut Vec<String>,
    ) -> Result<(), ToolchainError> {
        let target = ["arm64-macos", "arm64e-macos"]
            .into_iter()
            .find(|target| self.targets.iter().any(|value| value == target))
            .ok_or_else(|| {
                ToolchainError(format!(
                    "SDK {} has no compatible macOS/arm64 target",
                    self.install_name
                ))
            })?;
        for library in &self.reexported_libraries {
            if library.targets.iter().any(|value| value == target) {
                pending.extend(library.libraries.iter().cloned());
            }
        }
        let mut directives = Vec::new();
        for section in self
            .exports
            .iter()
            .chain(&self.reexports)
            .filter(|section| section.targets.iter().any(|value| value == target))
        {
            for symbol in section.symbols.iter().chain(&section.weak_symbols) {
                if symbol.starts_with("$ld$") {
                    directives.push(symbol);
                } else {
                    insert(exports, symbol.clone(), SystemExportKind::Symbol)?;
                }
            }
            for symbol in &section.thread_local_symbols {
                insert(exports, symbol.clone(), SystemExportKind::ThreadLocal)?;
            }
            for (names, prefixes) in [
                (
                    &section.objc_classes,
                    &["_OBJC_CLASS_$_", "_OBJC_METACLASS_$_"][..],
                ),
                (&section.objc_eh_types, &["_OBJC_EHTYPE_$_"][..]),
                (&section.objc_ivars, &["_OBJC_IVAR_$_"][..]),
            ] {
                for name in names {
                    for prefix in prefixes {
                        insert(exports, format!("{prefix}{name}"), SystemExportKind::Symbol)?;
                    }
                }
            }
        }
        for directive in directives {
            apply_directive(exports, directive, deployment)?;
        }
        Ok(())
    }
}

fn insert(
    exports: &mut BTreeMap<String, SystemExportKind>,
    symbol: String,
    kind: SystemExportKind,
) -> Result<(), ToolchainError> {
    if let Some(previous) = exports.insert(symbol.clone(), kind)
        && previous != kind
    {
        return Err(ToolchainError(format!(
            "SDK export {symbol} has conflicting TLS storage"
        )));
    }
    Ok(())
}

fn apply_directive(
    exports: &mut BTreeMap<String, SystemExportKind>,
    directive: &str,
    deployment: DarwinPackedVersionV1,
) -> Result<(), ToolchainError> {
    let mut parts = directive.trim_start_matches("$ld$").splitn(3, '$');
    let operation = parts.next().unwrap_or_default();
    let condition = parts.next().unwrap_or_default();
    let symbol = parts.next().unwrap_or_default();
    let version = condition
        .strip_prefix("os")
        .ok_or_else(|| ToolchainError(format!("invalid SDK linker directive {directive}")))?;
    let version = crate::c_bridge::parse_darwin_version(version, "SDK linker directive")?;
    if version.packed() & !0xff != deployment.packed() & !0xff {
        return Ok(());
    }
    match operation {
        "hide" => {
            exports.remove(symbol);
        }
        "add" | "weak" => {
            insert(exports, symbol.to_owned(), SystemExportKind::Symbol)?;
        }
        _ => {
            return Err(ToolchainError(format!(
                "SDK requires unhandled linker directive {directive}"
            )));
        }
    }
    Ok(())
}
