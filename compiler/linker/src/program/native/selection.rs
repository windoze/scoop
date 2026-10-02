use super::*;

pub(super) enum Candidate {
    Definition,
    Object(NativeObjectId),
    Dynamic(crate::dynamic::DynamicBinding),
}

pub(super) fn candidate(
    symbol: &str,
    binding: NativeLibraryBinding,
    inputs: &ProgramInputs<'_>,
) -> Result<Candidate, LinkError> {
    let explicit = match binding {
        NativeLibraryBinding::DefaultNativeNamespace => None,
        NativeLibraryBinding::Requirement(id) => Some(inputs.native.libraries[&id].input),
    };
    if let Some(owner) = inputs.definitions.get(symbol) {
        return match owner {
            DefinitionOwner::Native(id) if explicit.is_none_or(|input| input == id.input()) => {
                Ok(Candidate::Object(*id))
            }
            _ if explicit.is_none() => Ok(Candidate::Definition),
            _ => Err(error(format!(
                "native library {explicit:?} cannot bind {symbol}, already defined by {owner:?}"
            ))),
        };
    }
    let mut candidates = Vec::new();
    for file in inputs
        .native
        .files
        .values()
        .filter(|file| explicit.is_none_or(|id| id == file.id))
    {
        if let Some((id, _)) = file.candidate(symbol) {
            candidates.push(Candidate::Object(id));
        }
    }
    candidates.extend(
        inputs
            .providers
            .candidates(&inputs.native, symbol, explicit)?
            .into_iter()
            .map(Candidate::Dynamic),
    );
    if candidates.len() > 1 {
        return Err(error(format!(
            "ambiguous native providers for {symbol}: {} candidates",
            candidates.len()
        )));
    }
    candidates.pop().ok_or_else(|| error(match explicit {
        Some(id) => format!("native library {id} does not provide {symbol}"),
        None => format!("unresolved native symbol {symbol} in default namespace (unresolved symbol {symbol})"),
    }))
}

pub(super) fn include(
    inputs: &mut ProgramInputs<'_>,
    id: NativeObjectId,
    reason: &str,
) -> Result<(), LinkError> {
    let file = &inputs.native.files[&id.input()];
    let (index, range) = inputs.native.object(id)?;
    let origin = format!("native object {id} selected by {reason}");
    let diagnostic = format!("{origin} ({})", file.locator.display());
    let references = index
        .check_selected(&file.bytes[range])
        .map_err(|err| error(format!("{diagnostic}: {err}")))?;
    for symbol in index.info.definitions.keys() {
        if symbol == "_main" || symbol == "_scoop_td_String" || symbol.starts_with("_scoop$") {
            return Err(error(format!(
                "{diagnostic} defines compiler-owned symbol {symbol}"
            )));
        }
        if let Some(previous) = inputs
            .definitions
            .insert(symbol.clone(), DefinitionOwner::Native(id))
        {
            return Err(error(format!(
                "{diagnostic} conflicts with {previous:?} at {symbol}"
            )));
        }
    }
    for symbol in &index.info.requirements {
        inputs.requirements.insert(symbol.clone());
        inputs
            .requirement_origins
            .entry(symbol.clone())
            .or_default()
            .push(origin.clone());
    }
    inputs.native.selected.insert(id, reason.into());
    inputs.native.references.insert(id, references);
    Ok(())
}
