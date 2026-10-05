use super::*;
use scoop_identity::{NativeExternAbi, NativeExternalContract, NativeLibraryBinding};

use crate::native_input::{NativeContent, NativeObjectId};
use crate::{NativeSymbolDefinition, NativeSymbolKind};

mod contracts;
pub(super) mod declarations;
mod selection;

pub(super) struct Declarations<'a> {
    contract: &'a NativeExternalContract,
    origins: Vec<String>,
    difference: Option<String>,
}

pub(super) fn resolve(
    closure: &ProgramLinkClosure,
    declarations: &BTreeMap<String, Declarations<'_>>,
    runtime: &RuntimeObjectSet,
    profile: &ValidatedFinalLinkProfile,
    library_paths: &[std::path::PathBuf],
    inputs: &mut ProgramInputs<'_>,
) -> Result<(), LinkError> {
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
    for (symbol, declaration) in declarations {
        let result = (|| {
            if inputs
                .definitions
                .get(symbol)
                .is_some_and(|owner| matches!(owner, DefinitionOwner::Scoop(_)))
                || inputs.compiler_owned(symbol)
            {
                return Err(error(format!(
                    "source extern {symbol} conflicts with a compiler-owned definition"
                )));
            }
            let candidate = selection::candidate(symbol, declaration.contract.library(), inputs)?;
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
                selection::Candidate::Dynamic(binding) => {
                    let tls = matches!(
                        declaration.contract,
                        NativeExternalContract::ReadOnlyTls { .. }
                            | NativeExternalContract::MutableTls { .. }
                    );
                    if tls != binding.is_tls() {
                        return Err(error(format!("native TLS storage mismatch for {symbol}")));
                    }
                    if let Some(definition) = binding.definition() {
                        check_kind(symbol, declaration.contract, definition)?;
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
    selection::resolve(inputs, declarations)?;
    inputs.namespace.project(library_paths, profile)?;
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
