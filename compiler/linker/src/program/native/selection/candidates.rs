use super::*;
use scoop_lir::NativeObjectFormat;

pub(in crate::program::native) enum Candidate {
    Definition,
    Object(NativeObjectId),
    Dynamic(crate::namespace::NativeBinding),
}

pub(in crate::program::native) fn candidate(
    symbol: &str,
    binding: NativeLibraryBinding,
    inputs: &ProgramInputs<'_>,
) -> Result<Candidate, LinkError> {
    let explicit = match binding {
        NativeLibraryBinding::DefaultNativeNamespace => None,
        NativeLibraryBinding::Requirement(id) => {
            Some(inputs.native.libraries[&id].inputs.as_slice())
        }
    };
    let system_alias = match binding {
        NativeLibraryBinding::DefaultNativeNamespace => false,
        NativeLibraryBinding::Requirement(id) => inputs.native.libraries[&id].system_alias,
    };
    if let Some(owner) = inputs.definitions.get(symbol) {
        return match owner {
            DefinitionOwner::Native(id)
                if explicit.is_none_or(|input| input.contains(&id.input())) =>
            {
                Ok(Candidate::Object(*id))
            }
            _ if explicit.is_none() => Ok(Candidate::Definition),
            _ => Err(error(format!(
                "native library {explicit:?} cannot bind {symbol}, already defined by {owner:?}"
            ))),
        };
    }
    let mut candidates: Vec<_> = inputs
        .namespace
        .candidates(&inputs.native, symbol, explicit, system_alias)?
        .into_iter()
        .map(|binding| {
            let strength = binding.symbol_binding();
            (Candidate::Dynamic(binding), strength)
        })
        .collect();
    for file in inputs
        .native
        .ordered_files()
        .into_iter()
        .filter(|file| explicit.is_none_or(|ids| ids.contains(&file.id)))
    {
        if let Some((id, index)) = file.candidate(symbol) {
            candidates.push((Candidate::Object(id), index.definitions[symbol].binding));
        }
    }
    choose(symbol, candidates, inputs.target.native_object_format())?.ok_or_else(|| error(match explicit {
        Some(ids) => format!("native library {} does not provide {symbol}", ids.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")),
        None => format!("unresolved native symbol {symbol} in default namespace (unresolved symbol {symbol})"),
    }))
}

fn choose(
    symbol: &str,
    candidates: Vec<(Candidate, NativeSymbolBinding)>,
    format: NativeObjectFormat,
) -> Result<Option<Candidate>, LinkError> {
    let Some(priority) = candidates
        .iter()
        .map(|(_, binding)| binding.precedence(format))
        .max()
    else {
        return Ok(None);
    };
    let mut best = candidates
        .into_iter()
        .filter(|(_, binding)| binding.precedence(format) == priority);
    let (candidate, binding) = best.next().expect("maximum candidate priority exists");
    if binding == NativeSymbolBinding::Strong && best.next().is_some() {
        return Err(error(format!(
            "ambiguous native providers for {symbol}: multiple strong definitions"
        )));
    }
    Ok(Some(candidate))
}
