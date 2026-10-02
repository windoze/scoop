use super::*;
use scoop_identity::{NativeExternAbi, NativeExternalContract, NativeLibraryBinding};
use scoop_toolchain::SystemExportKind;

use crate::native_input::{NativeContent, NativeObjectId};
use crate::{NativeSymbolDefinition, NativeSymbolKind};

mod contracts;
mod declarations;
mod selection;

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
    let (declarations, libraries) = declarations::read(closure)?;
    inputs.native = NativeInputs::read(libraries, library_paths, profile)?;
    let direct: Vec<_> = inputs
        .native
        .ordered_files()
        .iter()
        .filter(|file| matches!(file.content, NativeContent::Object(_)))
        .map(|file| NativeObjectId::Direct(file.id))
        .collect();
    for id in direct {
        selection::include(inputs, id, "direct input")?;
    }
    for (symbol, declaration) in &declarations {
        let result = (|| {
            if inputs
                .definitions
                .get(symbol)
                .is_some_and(|owner| matches!(owner, DefinitionOwner::Scoop(_)))
                || symbol == "_main"
                || symbol == "_scoop_td_String"
                || symbol.starts_with("_scoop$")
            {
                return Err(error(format!(
                    "source extern {symbol} conflicts with a compiler-owned definition"
                )));
            }
            let candidate =
                selection::candidate(symbol, declaration.contract.library(), inputs, profile)?;
            match candidate {
                selection::Candidate::Object(id) => {
                    let (index, _) = inputs.native.object(id)?;
                    check_kind(symbol, declaration.contract, index.info.definitions[symbol])?;
                }
                selection::Candidate::Definition => {
                    let definition =
                        runtime.symbols().definitions.get(symbol).ok_or_else(|| {
                            error(format!("source extern {symbol} has no native definition"))
                        })?;
                    check_kind(symbol, declaration.contract, *definition)?;
                }
                selection::Candidate::System => {
                    let tls = matches!(
                        declaration.contract,
                        NativeExternalContract::ReadOnlyTls { .. }
                            | NativeExternalContract::MutableTls { .. }
                    );
                    if tls
                        != (profile.system_provider().exports()[symbol]
                            == SystemExportKind::ThreadLocal)
                    {
                        return Err(error(format!("native TLS storage mismatch for {symbol}")));
                    }
                }
            }
            contracts::check_compiler_contract(symbol, declaration.contract, profile, closure)
        })();
        result.map_err(|err| {
            error(format!(
                "{err}; origins: {}",
                declaration.origins.join(", ")
            ))
        })?;
    }
    let mut pending = inputs.requirements.clone();
    while let Some(symbol) = pending.pop_first() {
        if symbol == "_scoop_td_String" {
            continue;
        }
        let binding = declarations
            .get(&symbol)
            .map_or(NativeLibraryBinding::DefaultNativeNamespace, |decl| {
                decl.contract.library()
            });
        let origin = inputs
            .requirement_origins
            .get(&symbol)
            .map(|origins| origins.join(", "))
            .unwrap_or_else(|| "program startup/linker support".into());
        let candidate = selection::candidate(&symbol, binding, inputs, profile)
            .map_err(|err| error(format!("{err}; reference chain: {symbol} <- {origin}")))?;
        match candidate {
            selection::Candidate::Object(id) => {
                if !inputs.native.selected.contains_key(&id) {
                    selection::include(inputs, id, &format!("{symbol} <- {origin}"))?;
                    let (index, _) = inputs.native.object(id)?;
                    pending.extend(index.info.requirements.iter().cloned());
                }
            }
            selection::Candidate::Definition => continue,
            selection::Candidate::System => {
                inputs.dynamic.insert(symbol);
            }
        }
    }
    for file in inputs.native.ordered_files() {
        for (id, _, range) in file.objects() {
            if inputs.native.selected.contains_key(&id) {
                inputs.objects.push(InputObject {
                    origin: ObjectOrigin::Native(id),
                    bytes: ObjectBytes::Native {
                        file: file.bytes.clone(),
                        range,
                    },
                });
            }
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
