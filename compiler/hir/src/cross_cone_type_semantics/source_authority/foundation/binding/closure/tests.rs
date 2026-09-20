use super::*;
use scoop_identity::DefinitionOrigin;
use scoop_identity::*;
use scoop_wire::{DecodeLimits, decode_canonical, encode};

mod fixture;
mod public;
mod rejection;
use fixture::*;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[test]
fn byte_restored_dependencies_replay_inheritance_without_merging_local_inventory() {
    let base = Artifact::class("base", None).load(None);
    let child = Artifact::class("child", Some(&base)).load(Some(&base));
    let base_bound = base.bind();
    let child_bound = child.bind();
    let public = empty_public();
    let base_provider = provider(&base_bound, &public);
    let child_provider = provider(&child_bound, &public);
    let closure =
        TypeFoundationSourceClosureV1::try_new(child_provider, &[base_provider], &mut meter())
            .unwrap();
    assert_eq!(closure.current_provider(), child.provider());
    assert_eq!(
        closure.local_source_roots().unwrap(),
        &[SourceNominalId::Concrete(child.owner)]
    );
    assert_eq!(
        closure.local_exact_facts().unwrap().values(),
        &[child.exact]
    );
    assert_eq!(
        closure.required_representation_owners().unwrap().values(),
        &[child.owner]
    );
    assert_eq!(
        closure.dependency_facts().unwrap(),
        &[TypeSectionDependencyFactV1 {
            provider: base.provider(),
            exact: base.exact,
        }]
    );
    assert_eq!(closure.local_inheritance_edges().unwrap().len(), 1);
    for input in [&base, &child] {
        let owner = SourceNominalId::Concrete(input.owner);
        assert_eq!(
            closure.nominal_declaration_key(owner).unwrap().origin(),
            input.provider()
        );
        closure
            .validate_definition_source(closure.nominal_definition_source(owner).unwrap())
            .unwrap();
        assert!(matches!(
            closure.fact_shape(input.exact).unwrap(),
            ExactTypeFactShapeV1::Reference
        ));
    }
    let graph = CheckedNominalInheritanceGraphV1::validate_with_source_roots(
        child
            .source
            .entries()
            .local_inheritance_edges
            .records()
            .iter()
            .chain(base.source.entries().local_inheritance_edges.records()),
        [
            SourceNominalId::Concrete(child.owner),
            SourceNominalId::Concrete(base.owner),
        ],
        &closure,
        &mut meter(),
    )
    .unwrap();
    assert!(
        graph
            .is_subclass(child.exact, base.exact, &mut meter())
            .unwrap()
    );
    assert!(
        !graph
            .is_subclass(base.exact, child.exact, &mut meter())
            .unwrap()
    );
    child
        .source
        .entries()
        .representations
        .validate_source_semantics(&closure, &mut meter(), &WirePath::root())
        .unwrap();
    let facts = CanonicalExactTypeFactsV1::try_new(vec![
        ExactTypeFactsV1::try_new(
            child.exact,
            ExactTypeKindV1::Reference,
            ExactTypeGcV1::ContainsManagedReferences,
        )
        .unwrap(),
    ])
    .unwrap();
    facts.validate_semantics(&closure, &mut meter()).unwrap();
    assert!(matches!(
        closure
            .representation_source(base.owner)
            .unwrap()
            .public_value_shape,
        NominalRepresentationPublicValueShapeV1::NoPublicValueShape
    ));
}

#[test]
fn derived_source_representation_cannot_be_replaced_by_dependency_representation() {
    let base = Artifact::class("base", None).load(None);
    let child = Artifact::class("child", Some(&base)).load(Some(&base));
    let base_bound = base.bind();
    let child_bound = child.bind();
    let public = empty_public();
    let closure = TypeFoundationSourceClosureV1::try_new(
        provider(&child_bound, &public),
        &[provider(&base_bound, &public)],
        &mut meter(),
    )
    .unwrap();
    let candidate = &base.source.entries().representations;
    assert!(
        candidate
            .validate_source_semantics(&closure, &mut meter(), &WirePath::root())
            .is_err()
    );
    let original = child
        .source
        .entries()
        .representations
        .get(child.owner)
        .unwrap();
    let forged = NominalRepresentationSupportV1::try_new(
        child_bound
            .nominal_key(SourceNominalId::Concrete(child.owner))
            .unwrap(),
        original.declaration_access().clone(),
        NominalRepresentationShapeV1::Class {
            base: OptionalSignatureType::Absent,
            declared_fields: vec![],
        },
    )
    .unwrap();
    assert!(
        CanonicalNominalRepresentationSupportV1::try_new(vec![forged])
            .unwrap()
            .validate_source_semantics(&closure, &mut meter(), &WirePath::root())
            .is_err()
    );
}
