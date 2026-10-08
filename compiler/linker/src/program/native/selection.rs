use super::*;

#[cfg(test)]
mod tests;

pub(super) enum Candidate {
    Definition,
    Object(NativeObjectId),
    Dynamic(crate::namespace::NativeBinding),
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
            .namespace
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
        if inputs.compiler_owned(symbol) {
            return Err(error(format!(
                "{diagnostic} defines compiler-owned symbol {symbol}"
            )));
        }
        if let Some(previous) = inputs
            .definitions
            .insert(symbol.clone(), DefinitionOwner::Native(id))
        {
            let previous = match previous {
                DefinitionOwner::Native(previous) => inputs.native.files[&previous.input()]
                    .locator
                    .display()
                    .to_string(),
                other => format!("{other:?}"),
            };
            return Err(error(format!(
                "{diagnostic} conflicts with {previous} at {symbol}"
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
    if let Some(references) = references {
        inputs.native.references.insert(id, references);
    }
    Ok(())
}

pub(super) fn resolve(
    inputs: &mut ProgramInputs<'_>,
    declarations: &BTreeMap<String, Declarations<'_>>,
) -> Result<(), LinkError> {
    let mut pending = inputs.requirements.clone();
    while let Some(symbol) = pending.pop_first() {
        if inputs.linker_defined(&symbol) {
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
        let candidate = candidate(&symbol, binding, inputs)
            .map_err(|err| error(format!("{err}; reference chain: {symbol} <- {origin}")))?;
        match candidate {
            Candidate::Object(id) => {
                if !inputs.native.selected.contains_key(&id) {
                    include(inputs, id, &format!("{symbol} <- {origin}"))?;
                    let (index, _) = inputs.native.object(id)?;
                    pending.extend(index.info.requirements.iter().cloned());
                }
            }
            Candidate::Definition => continue,
            Candidate::Dynamic(binding) => {
                for requirement in inputs.namespace.bind(symbol.clone(), binding)? {
                    inputs.requirements.insert(requirement.clone());
                    inputs
                        .requirement_origins
                        .entry(requirement.clone())
                        .or_default()
                        .push(format!("native dynamic input selected by {symbol}"));
                    pending.insert(requirement);
                }
            }
        }
    }
    // A member selected for another symbol can add a definition after a
    // dynamic binding was chosen. Revisit only these changed resolutions.
    let changed: Vec<_> = inputs
        .namespace
        .bound_symbols()
        .into_iter()
        .filter(|symbol| inputs.definitions.contains_key(*symbol))
        .cloned()
        .collect();
    for symbol in changed {
        let declaration = declarations.get(&symbol);
        let binding = declaration.map_or(NativeLibraryBinding::DefaultNativeNamespace, |decl| {
            decl.contract.library()
        });
        let candidate = candidate(&symbol, binding, inputs).map_err(|err| {
            let origins = declaration
                .map(|decl| decl.origins.join(", "))
                .unwrap_or_default();
            error(format!("{err}; origins: {origins}"))
        })?;
        if let (Candidate::Object(id), Some(declaration)) = (candidate, declaration) {
            let (index, _) = inputs.native.object(id)?;
            check_kind(
                &symbol,
                declaration.contract,
                index.info.definitions[&symbol],
            )?;
        }
        inputs.namespace.unbind(&symbol);
    }
    Ok(())
}
