use super::super::link_archive;
use super::*;

pub(super) fn cases(proof: &slib::ReplayedLayoutLinkSymbolUsesV1) -> Vec<(Failure, Mutation)> {
    let mut cases = Vec::new();
    let mut projection = |failure| cases.push((failure, Mutation::Projection(failure)));
    assert!(!proof.defined_symbols().owners().is_empty());
    for failure in [
        Failure::DefinedMissing,
        Failure::DefinedDuplicate,
        Failure::DefinedMember,
    ] {
        projection(failure);
    }
    let parts = proof.undefined_partitions();
    if !parts.legacy().requirements().is_empty() {
        projection(Failure::LegacyMissing);
        projection(Failure::LegacyWidth);
    }
    if !parts.cross_cone().requirements().is_empty() {
        for failure in [
            Failure::OrdinaryMissing,
            Failure::OrdinaryDuplicate,
            Failure::OrdinaryIndex,
        ] {
            projection(failure);
        }
    }
    if !parts.external_shape().is_empty() {
        for failure in [
            Failure::ShapeMissing,
            Failure::ShapeDuplicate,
            Failure::ShapeIndex,
        ] {
            projection(failure);
        }
    }
    let objects = proof.object_contents();
    for (failure, native) in [
        (Failure::UnknownRuntime, false),
        (Failure::WrongNativeSymbol, true),
    ] {
        let use_ = parts.legacy().requirements().iter().find(|requirement| {
            use slib::FinalUndefinedSymbolRequirementV1 as Requirement;
            if native {
                matches!(requirement.requirement(), Requirement::SourceExtern { .. })
            } else {
                matches!(requirement.requirement(), Requirement::RuntimeAbi { .. })
                    && objects
                        .objects()
                        .objects()
                        .iter()
                        .any(|object| object.member() == requirement.member())
            }
        });
        let Some(use_) = use_ else { continue };
        let use_ = use_.use_site();
        let binding = objects
            .patch_sites()
            .builtins()
            .strong_relocations()
            .bindings()
            .iter()
            .find(|binding| {
                binding.source_member() == use_.source_member() && binding.symbol() == use_.symbol()
            })
            .unwrap();
        let slib::StrongRelocationResolutionV1::ExternalCandidate {
            object_symbol_table_index,
        } = binding.resolution()
        else {
            panic!("runtime and native uses name external symbols")
        };
        let bytes = objects
            .objects()
            .objects()
            .iter()
            .find(|object| object.member() == binding.source_member())
            .map(|object| object.final_bytes())
            .or_else(|| {
                objects
                    .generated_objects()
                    .find(|object| object.member() == binding.source_member())
                    .map(|object| object.bytes())
            })
            .unwrap();
        let offset =
            link_archive::symbol_offset(bytes, object_symbol_table_index) + use_.symbol().len() - 1;
        assert_eq!(
            &bytes[offset + 1 - use_.symbol().len()..=offset],
            use_.symbol()
        );
        cases.push((
            failure,
            Mutation::Object {
                member: binding.source_member(),
                offset,
            },
        ));
    }
    cases
}
