use super::*;
use crate::tests::m23_ordinary_core_only::support::trusted_core;
use crate::tests::m23_ordinary_dependencies::support::{alias_expansions, project_dependency_text};
use scoop_identity::ConeCoordinate;

#[test]
fn imported_intermediate_overrides_use_published_slots_and_property_edges() {
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "editing-implementations", "1.0.0").unwrap();
    let provider = "public abstract class Base<T> public constructor() { public abstract fun choose(baseValue: T): T\npublic abstract val flag: Boolean }\npublic open class Middle<T> public constructor() : Base<T>() { public open override fun choose(middleValue: T): T = middleValue\npublic open override val flag: Boolean get() = true }\npublic abstract class AbstractStep<T> public constructor() : Middle<T>() { public abstract override fun choose(abstractValue: T): T\npublic abstract override val flag: Boolean }";
    let (foundation, interface) = project_dependency_text(&core, &coordinate, provider, &[]);
    let aliases = alias_expansions(interface.type_aliases());
    let imported = core.import_dependency_foundation(&coordinate, &foundation, 209);
    let identity = crate::tests::test_source_identity("src/members.scoop");
    let world = hir::ImportedSemanticWorld::from_dependencies(
        identity.cone(),
        vec![
            core.provider(),
            hir::ImportedProviderInput {
                foundation: &imported,
                interface: &interface,
                alias_expansions: &aliases,
            },
        ],
        Vec::new(),
    )
    .unwrap();
    let core_inputs =
        CoreProtocolInput::from(core.foundation.import_core_inputs(&core.interface).unwrap());
    let text = "class Concrete(public override val flag: Boolean) : AbstractStep<Boolean>() { public override fun choose(localValue: Boolean): Boolean = localValue }\nfun root(base: Base<Boolean>): Boolean = base.choose(false)\nfun mid(middle: Middle<Boolean>): Boolean = middle.choose(true)\nfun step(abstractValue: AbstractStep<Boolean>): Boolean = abstractValue.choose(abstractValue.flag)\nfun rootFlag(base: Base<Boolean>): Boolean = base.flag\nfun middleFlag(middle: Middle<Boolean>): Boolean = middle.flag";
    let parsed = scoop_parser::parse_recovering(text).unwrap();
    let source = scoop_parser::declaration_source(
        &parsed,
        &identity,
        scoop_identity::SourceRole::Production,
        text,
    );
    let mut analysis =
        analyze_declarations(&[source], &core_inputs, &world, &[], || false).unwrap();
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    let root = target(&mut analysis, text, "choose(false)", 6);
    let middle = target(&mut analysis, text, "choose(true)", 6);
    let abstract_step = target(&mut analysis, text, "choose(abstractValue.flag)", 6);
    let concrete = target(&mut analysis, text, "choose(localValue:", 6);
    let concrete_property = target(&mut analysis, text, "flag: Boolean", 4);
    let mut property_targets = Vec::new();
    for (needle, prefix) in [
        ("base.flag", "base."),
        ("middle.flag", "middle."),
        ("abstractValue.flag", "abstractValue."),
    ] {
        let start = (text.find(needle).unwrap() + prefix.len()) as u32;
        let V::Known(targets) = analysis
            .member_implementation_targets(&identity, ast::Span::new(start, start + 4), || false)
            .unwrap()
        else {
            panic!("known imported property: {:?}", analysis.diagnostics());
        };
        assert_eq!(targets.len(), 1);
        property_targets.push(targets[0]);
    }
    let V::Known(index) = analysis.member_implementation_index(|| false).unwrap() else {
        panic!("known graph");
    };
    assert!(index.unavailable.is_empty(), "{index:?}");
    for (requirement, implementation) in [
        (root, middle),
        (middle, abstract_step),
        (abstract_step, concrete),
        (property_targets[0], property_targets[1]),
        (property_targets[1], property_targets[2]),
        (property_targets[2], concrete_property),
    ] {
        assert!(
            has_relation(index, requirement, implementation),
            "missing {requirement:?} -> {implementation:?}: {index:?}"
        );
    }
    let definitions = index
        .implementations
        .iter()
        .map(|implementation| implementation.declaration)
        .collect::<Vec<_>>();
    assert_eq!(definitions.len(), 4, "{index:?}");
    assert!(
        [middle, concrete, property_targets[1], concrete_property]
            .iter()
            .all(|member| definitions.contains(member))
    );
}
