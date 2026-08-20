use super::*;
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextStubInterface {
    pub install_name: String,
    pub current_version: u32,
    pub compatibility_version: u32,
    pub exports: BTreeMap<String, NativeExport>,
    pub reexports: Vec<String>,
    pub previous_exports: BTreeMap<String, PreviousExport>,
}

pub fn read_text_stubs(
    bytes: &[u8],
    deployment: DarwinPackedVersionV1,
    system: bool,
) -> Result<Vec<TextStubInterface>, ToolchainError> {
    let records: Vec<_> = parse(bytes)?
        .into_iter()
        .map(|record| {
            if record
                .flags
                .iter()
                .any(|flag| flag != "not_app_extension_safe" && flag != "not_for_dyld_shared_cache")
            {
                return Err(ToolchainError(format!(
                    "text stub {} has unsupported load flags {:?}",
                    record.install_name, record.flags
                )));
            }
            let mut exports = BTreeMap::new();
            let mut reexports = Vec::new();
            let directives = record.collect(deployment, system, &mut exports, &mut reexports)?;
            Ok(TextStubInterface {
                install_name: directives.install_name.unwrap_or(record.install_name),
                current_version: record.current_version,
                compatibility_version: directives
                    .compatibility_version
                    .unwrap_or(record.compatibility_version),
                exports,
                reexports,
                previous_exports: directives.previous,
            })
        })
        .collect::<Result<_, _>>()?;
    let mut unique = Vec::new();
    let mut names = BTreeMap::new();
    for record in records {
        if let Some(index) = names.get(&record.install_name) {
            if unique[*index] != record {
                return Err(ToolchainError(format!(
                    "conflicting text stub records for install name {}",
                    record.install_name
                )));
            }
        } else {
            names.insert(record.install_name.clone(), unique.len());
            unique.push(record);
        }
    }
    Ok(unique)
}

pub fn write_link_stub(
    install_name: &str,
    current_version: u32,
    compatibility_version: u32,
    imports: &BTreeMap<String, NativeExport>,
) -> Result<Vec<u8>, ToolchainError> {
    let mut symbols = Vec::new();
    let mut weak_symbols = Vec::new();
    let mut thread_local_symbols = Vec::new();
    for (symbol, export) in imports {
        match (export.kind, export.weak) {
            (SystemExportKind::Symbol, false) => symbols.push(symbol.as_str()),
            (SystemExportKind::Symbol, true) => weak_symbols.push(symbol.as_str()),
            (SystemExportKind::ThreadLocal, false) => thread_local_symbols.push(symbol.as_str()),
            (SystemExportKind::ThreadLocal, true) => {
                return Err(ToolchainError(format!(
                    "v4 stub cannot preserve weak TLS export {symbol}"
                )));
            }
        }
    }
    let value = Stub {
        tbd_version: 4,
        targets: ["arm64-macos"],
        install_name,
        current_version: version_text(current_version),
        compatibility_version: version_text(compatibility_version),
        exports: [ExportList {
            targets: ["arm64-macos"],
            symbols,
            weak_symbols,
            thread_local_symbols,
        }],
    };
    let yaml = serde_yaml_ng::to_string(&value)
        .map_err(|err| ToolchainError(format!("cannot write native link stub: {err}")))?;
    Ok(format!("--- !tapi-tbd\n{yaml}...\n").into_bytes())
}

fn version_text(value: u32) -> String {
    format!("{}.{}.{}", value >> 16, (value >> 8) & 255, value & 255)
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct Stub<'a> {
    tbd_version: u32,
    targets: [&'a str; 1],
    install_name: &'a str,
    current_version: String,
    compatibility_version: String,
    exports: [ExportList<'a>; 1],
}
#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct ExportList<'a> {
    targets: [&'a str; 1],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    symbols: Vec<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    weak_symbols: Vec<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    thread_local_symbols: Vec<&'a str>,
}
