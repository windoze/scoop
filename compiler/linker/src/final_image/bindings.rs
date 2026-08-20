use super::*;

pub(super) fn check(image: &FinalImage<'_>, inputs: &ProgramInputs<'_>) -> Result<(), LinkError> {
    let mut seen = BTreeSet::new();
    for import in image.file.imports().map_err(error)? {
        let name = std::str::from_utf8(import.name()).map_err(error)?;
        let expected = inputs
            .namespace
            .darwin()?
            .bindings
            .get(name)
            .ok_or_else(|| error(format!("unexpected final dynamic import {name}")))?;
        if import.library()
            != inputs.namespace.darwin()?.providers.providers[&expected.owner]
                .install_name
                .as_bytes()
            || inputs.definitions.contains_key(name)
        {
            return Err(error(format!(
                "final dynamic import {name} binds to a different provider"
            )));
        }
        seen.insert(name.to_owned());
    }
    for binding in image.bindings.values() {
        let expected = inputs
            .namespace
            .darwin()?
            .bindings
            .get(&binding.symbol)
            .ok_or_else(|| {
                error(format!(
                    "unexpected final binding {} from ordinal {}",
                    binding.symbol, binding.ordinal
                ))
            })?;
        let owner = binding
            .ordinal
            .checked_sub(1)
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| image.providers.get(index));
        if owner != Some(&expected.owner) || inputs.definitions.contains_key(&binding.symbol) {
            return Err(error(format!(
                "unexpected final binding {} from ordinal {}; expected provider {}",
                binding.symbol, binding.ordinal, expected.owner
            )));
        }
        seen.insert(binding.symbol.clone());
    }
    if let Some(symbol) = inputs
        .namespace
        .darwin()?
        .bindings
        .keys()
        .find(|symbol| !seen.contains(*symbol))
    {
        return Err(error(format!(
            "final image omitted dynamic import {symbol}"
        )));
    }
    for (address, binding) in &image.weak_bindings {
        let initial = image.bindings.get(address);
        if initial.is_some_and(|initial| {
            initial.symbol != binding.symbol || initial.addend != binding.addend
        }) {
            return Err(error(format!(
                "weak coalescing at {address:#x} differs from its initial binding"
            )));
        }
        if inputs.definitions.contains_key(&binding.symbol) {
            if image.exports.get(&binding.symbol) != Some(&image.symbol(&binding.symbol)?) {
                return Err(error(format!(
                    "weak binding {} has no matching final export",
                    binding.symbol
                )));
            }
        } else if initial.is_none() {
            return Err(error(format!(
                "weak binding {} has no initial dynamic binding",
                binding.symbol
            )));
        }
    }
    Ok(())
}
