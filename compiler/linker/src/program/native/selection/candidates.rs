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

#[cfg(test)]
mod tests {
    use super::*;
    use NativeSymbolBinding::{Common, Strong, Weak};
    use scoop_lir::NativeObjectFormat::{Elf64, MachO64};

    fn provider(binding: NativeSymbolBinding) -> (Candidate, NativeSymbolBinding) {
        let definition = NativeSymbolDefinition {
            kind: NativeSymbolKind::Function,
            read_only: true,
            binding,
        };
        (
            Candidate::Dynamic(crate::namespace::NativeBinding::Elf(
                crate::namespace::elf::ElfBinding::System(definition),
            )),
            binding,
        )
    }

    fn selected(
        bindings: &[NativeSymbolBinding],
        format: NativeObjectFormat,
    ) -> NativeSymbolBinding {
        let result = choose(
            "fmod",
            bindings.iter().copied().map(provider).collect(),
            format,
        )
        .unwrap();
        let Some(Candidate::Dynamic(binding)) = result else {
            panic!("expected a provider")
        };
        binding.symbol_binding()
    }

    #[test]
    fn strong_provider_overrides_archive_fallback_in_either_order() {
        for format in [Elf64, MachO64] {
            assert_eq!(selected(&[Weak, Strong], format), Strong);
            assert_eq!(selected(&[Strong, Weak], format), Strong);
            assert_eq!(selected(&[Common, Strong, Weak], format), Strong);
        }
    }

    #[test]
    fn common_and_weak_precedence_is_target_specific() {
        for bindings in [[Common, Weak], [Weak, Common]] {
            assert_eq!(selected(&bindings, Elf64), Common);
            assert_eq!(selected(&bindings, MachO64), Weak);
        }
    }

    #[test]
    fn equally_weak_providers_reuse_first_candidate() {
        for binding in [Weak, Common] {
            let result = choose(
                "fallback",
                vec![(Candidate::Definition, binding), provider(binding)],
                Elf64,
            )
            .unwrap();
            assert!(matches!(result, Some(Candidate::Definition)));
        }
        assert!(choose("missing", vec![], Elf64).unwrap().is_none());
    }

    #[test]
    fn multiple_strong_providers_remain_ambiguous() {
        let result = choose(
            "fmod",
            vec![provider(Weak), provider(Strong), provider(Strong)],
            Elf64,
        );
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("ambiguous native providers for fmod")
        );
    }
}
