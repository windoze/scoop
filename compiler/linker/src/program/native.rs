use super::*;
use scoop_identity::{NativeExternAbi, NativeExternalContract, NativeLibraryBinding};
use scoop_toolchain::SystemExportKind;

use crate::{NativeSymbolDefinition, NativeSymbolKind};

mod contracts;

pub(super) fn resolve(
    closure: &ProgramLinkClosure,
    runtime: &RuntimeObjectSet,
    profile: &ValidatedFinalLinkProfile,
    inputs: &mut ProgramInputs<'_>,
) -> Result<(), LinkError> {
    let mut declarations: BTreeMap<String, (&NativeExternalContract, Vec<String>)> =
        BTreeMap::new();
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
                Some((first, origins)) => {
                    origins.push(origin);
                    if *first != requirement.contract() {
                        return Err(error(format!(
                            "native contract conflict for {symbol}: {}; origins: {}",
                            contracts::difference(first, requirement.contract()),
                            origins.join(", ")
                        )));
                    }
                }
                None => {
                    declarations.insert(symbol, (requirement.contract(), vec![origin]));
                }
            }
        }
    }
    for (symbol, (contract, origins)) in &declarations {
        if contract.library() != NativeLibraryBinding::DefaultNativeNamespace {
            return Err(error(format!(
                "native input for {symbol} requires library {:?}, absent from the M23-9 runtime/system input set (additional providers belong to M23-10); origins: {}",
                contract.library(),
                origins.join(", ")
            )));
        }
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
        if let Some(definition) = runtime.symbols().definitions.get(symbol) {
            check_kind(symbol, contract, *definition)?;
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
        contracts::check_compiler_contract(symbol, contract, profile, closure)?;
        inputs.requirements.insert(symbol.clone());
    }
    for symbol in &inputs.requirements {
        if symbol == "_scoop_td_String" {
            continue;
        }
        let controlled = inputs.definitions.contains_key(symbol);
        let system = profile.system_provider().exports().contains_key(symbol);
        if controlled && system {
            return Err(error(format!(
                "multiple native providers for {symbol}: program/runtime and libSystem"
            )));
        }
        if system {
            inputs.dynamic.insert(symbol.clone());
        }
        if !controlled && !system {
            return Err(error(format!(
                "unresolved symbol {symbol} from the explicit Cone/runtime inputs; no additional native provider is supplied"
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
