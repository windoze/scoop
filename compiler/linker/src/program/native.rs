use super::*;
use scoop_identity::{NativeExternAbi, NativeExternalContract, NativeLibraryBinding};
use scoop_toolchain::SystemExportKind;

use crate::{NativeSymbolDefinition, NativeSymbolKind};

mod contracts;

struct Declarations<'a> {
    contract: &'a NativeExternalContract,
    origins: Vec<String>,
    difference: Option<String>,
}

pub(super) fn resolve(
    closure: &ProgramLinkClosure,
    runtime: &RuntimeObjectSet,
    profile: &ValidatedFinalLinkProfile,
    library_paths: &[std::path::PathBuf],
    inputs: &mut ProgramInputs<'_>,
) -> Result<(), LinkError> {
    let mut declarations: BTreeMap<String, Declarations<'_>> = BTreeMap::new();
    for (artifact, symbols) in closure.artifacts() {
        for requirement in symbols.native_requirements().contracts() {
            let symbol = String::from_utf8(
                requirement
                    .symbol_key()
                    .native_link_symbol()
                    .as_bytes()
                    .to_vec(),
            )
            .map_err(error)?;
            let origin = format!(
                "{} (native declarations {:?})",
                artifact.manifest().cone().coordinate(),
                requirement.sources()
            );
            match declarations.get_mut(&symbol) {
                Some(existing) => {
                    existing.origins.push(origin);
                    if existing.contract != requirement.contract() && existing.difference.is_none()
                    {
                        existing.difference = Some(contracts::difference(
                            existing.contract,
                            requirement.contract(),
                        ));
                    }
                }
                None => {
                    declarations.insert(
                        symbol,
                        Declarations {
                            contract: requirement.contract(),
                            origins: vec![origin],
                            difference: None,
                        },
                    );
                }
            }
        }
    }
    for (symbol, declarations) in &declarations {
        if let Some(difference) = &declarations.difference {
            return Err(error(format!(
                "native contract conflict for {symbol}: {difference}; origins: {}",
                declarations.origins.join(", ")
            )));
        }
    }
    let mut libraries = BTreeMap::new();
    for (artifact, symbols) in closure.artifacts() {
        for requirement in symbols.native_requirements().contracts() {
            if let Some(record) = requirement.library().requirement() {
                let entry = libraries
                    .entry(record.id())
                    .or_insert_with(|| (record.key().clone(), Vec::new()));
                entry.1.push(format!(
                    "{} (native declarations {:?})",
                    artifact.manifest().cone().coordinate(),
                    requirement.sources()
                ));
            }
        }
    }
    inputs.native = NativeInputs::read(libraries, library_paths, profile)?;
    let mut native_origins: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for file in inputs.native.ordered_files() {
        for symbol in file.info.definitions.keys() {
            if symbol == "_main" || symbol == "_scoop_td_String" || symbol.starts_with("_scoop$") {
                return Err(error(format!(
                    "native object {} defines compiler-owned symbol {symbol}",
                    file.locator.display()
                )));
            }
            if let Some(previous) = inputs
                .definitions
                .insert(symbol.clone(), DefinitionOwner::Native(file.id))
            {
                return Err(error(format!(
                    "native object {} conflicts with {previous:?} at {symbol}",
                    file.locator.display()
                )));
            }
        }
        for symbol in &file.info.requirements {
            inputs.requirements.insert(symbol.clone());
            native_origins
                .entry(symbol.clone())
                .or_default()
                .push(format!("native {} ({})", file.id, file.locator.display()));
        }
        inputs.objects.push(InputObject {
            origin: ObjectOrigin::Native(file.id),
            bytes: ObjectBytes::Native {
                file: file.bytes.clone(),
                range: file.slice.clone(),
            },
        });
    }
    for (symbol, declarations) in &declarations {
        let Declarations {
            contract,
            origins,
            difference: _,
        } = declarations;
        if inputs
            .definitions
            .get(symbol)
            .is_some_and(|owner| matches!(owner, DefinitionOwner::Scoop(_)))
            || symbol == "_main"
            || symbol == "_scoop_td_String"
            || symbol.starts_with("_scoop$")
        {
            return Err(error(format!(
                "source extern {symbol} conflicts with a compiler-owned definition; origins: {}",
                origins.join(", ")
            )));
        }
        let explicit = match contract.library() {
            NativeLibraryBinding::Requirement(id) => Some(inputs.native.libraries[&id].input),
            NativeLibraryBinding::DefaultNativeNamespace => None,
        };
        let native_definition = inputs.native.files.values().find_map(|file| {
            file.info
                .definitions
                .get(symbol)
                .map(|definition| (file.id, *definition))
        });
        if let Some(expected) = explicit {
            if native_definition.map(|(id, _)| id) != Some(expected) {
                return Err(error(format!(
                    "native library {expected} does not provide {symbol}; origins: {}",
                    origins.join(", ")
                )));
            }
        }
        if let Some((_, definition)) = native_definition {
            check_kind(symbol, contract, definition)
                .map_err(|err| error(format!("{err}; origins: {}", origins.join(", "))))?;
        } else if let Some(definition) = runtime.symbols().definitions.get(symbol) {
            check_kind(symbol, contract, *definition)
                .map_err(|err| error(format!("{err}; origins: {}", origins.join(", "))))?;
        } else if let Some(kind) = profile.system_provider().exports().get(symbol) {
            let tls = matches!(
                contract,
                NativeExternalContract::ReadOnlyTls { .. }
                    | NativeExternalContract::MutableTls { .. }
            );
            if tls != (*kind == SystemExportKind::ThreadLocal) {
                return Err(error(format!(
                    "native TLS storage mismatch for {symbol}; origins: {}",
                    origins.join(", ")
                )));
            }
        } else {
            return Err(error(format!(
                "unresolved native symbol {symbol} in default namespace; origins: {}",
                origins.join(", ")
            )));
        }
        contracts::check_compiler_contract(symbol, contract, profile, closure)
            .map_err(|err| error(format!("{err}; origins: {}", origins.join(", "))))?;
    }
    for symbol in &inputs.requirements {
        if symbol == "_scoop_td_String" {
            continue;
        }
        let controlled = inputs.definitions.contains_key(symbol);
        let system = profile.system_provider().exports().contains_key(symbol);
        if system && !controlled {
            inputs.dynamic.insert(symbol.clone());
        }
        if !controlled && !system {
            return Err(error(format!(
                "unresolved symbol {symbol} from the explicit Cone/runtime/native inputs; origins: {:?}",
                native_origins.get(symbol)
            )));
        }
    }
    Ok(())
}

fn check_kind(
    symbol: &str,
    contract: &NativeExternalContract,
    definition: NativeSymbolDefinition,
) -> Result<(), LinkError> {
    let (kind, mutable) = match contract {
        NativeExternalContract::Function { .. } => (NativeSymbolKind::Function, false),
        NativeExternalContract::ReadOnlyData { .. } => (NativeSymbolKind::Data, false),
        NativeExternalContract::MutableData { .. } => (NativeSymbolKind::Data, true),
        NativeExternalContract::ReadOnlyTls { .. } => (NativeSymbolKind::ThreadLocal, false),
        NativeExternalContract::MutableTls { .. } => (NativeSymbolKind::ThreadLocal, true),
    };
    if definition.kind != kind || (mutable && definition.read_only) {
        return Err(error(format!(
            "native definition {symbol} has incompatible function/data/TLS/mutability: {definition:?}"
        )));
    }
    Ok(())
}
