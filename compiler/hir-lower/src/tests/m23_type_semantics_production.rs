use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{
    BindingTarget, ConeIdentity, ExportBindingKey, NominalDeclarationOwner, PersistentExactTypeId,
    PersistentExportBindingId,
};
use scoop_wire::{BudgetMeter, DecodeLimits, WireError, WirePath};

use super::m23_ordinary_core_only::support::{parsed_ordinary, trusted_core};
use super::*;
use crate::{CurrentConeSources, lower_current_cone};
use hir::NominalInheritanceSemanticAuthority as _;

mod public_projection;
use public_projection::public_interface;

mod automatic_materialization;
mod declaration_dump;
mod default_type_access;
mod fact_providers;
mod setter_domains;
mod shape_demands;
mod shared_callable_declarations;
mod shared_nominal_declarations;
mod shared_property_declarations;
mod shared_source_domains;
mod source_binding;
mod source_constructor_gc;
mod source_constructor_safety;
mod source_constructors;
mod source_default_access;
mod source_default_access_declarations;
mod source_default_bodies;
mod source_default_derived_order;
mod source_default_references;
mod source_default_table;
mod source_default_templates;
mod source_dispatch;
mod source_fields;
mod source_foundation;
mod source_inventory;
mod source_mir_types;
mod source_native_boundary;
mod source_nominal_callables;
mod source_nominal_constructors;
mod source_nominal_parameters;
mod source_nominal_properties;
mod source_nominals;
mod source_only_nominals;
mod source_parameters;
mod source_properties;
mod source_protected_callables;
mod source_shapes;

fn produce_cross_cone_type_semantics(
    output: &hir::DependencyHirOutput,
    public: &hir::CrossConeHirInterfaceSectionV1,
) -> Result<hir::CrossConeTypeSemanticsProductionV1, hir::CrossConeTypeSemanticsProductionError> {
    crate::produce_cross_cone_type_semantics(
        output,
        public,
        &mut BudgetMeter::new(DecodeLimits::default()),
    )
}

struct FactShapes(BTreeMap<PersistentExactTypeId, hir::ExactTypeFactShapeV1>);

impl hir::ExactTypeFactsSemanticAuthority<&'static str> for FactShapes {
    fn fact_shape(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&hir::ExactTypeFactShapeV1, &'static str> {
        self.0.get(&exact).ok_or("missing dependency fact shape")
    }
}

struct DependencyFacts<'a>(hir::CheckedExactTypeFactsV1<'a>);

impl hir::ExactTypeFactsDependencyLookupV1 for DependencyFacts<'_> {
    fn get_dependency_fact(
        &self,
        exact: PersistentExactTypeId,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<Option<hir::CheckedExactTypeFactV1<'_>>, WireError> {
        Ok(self.0.get_checked(exact))
    }
}

fn object_decl(name: &str) -> Decl {
    Decl::Object(ast::ObjectDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        supertypes: Vec::new(),
        members: Vec::new(),
        span: sp(),
    })
}

fn lower_public_nominals() -> hir::DependencyHirOutput {
    lower_public_declarations(vec![
        struct_decl("EmptyValue", Vec::new()),
        struct_decl("WordValue", vec![("value", ty_named("Int"))]),
        enum_decl(
            "Flag",
            Vec::new(),
            vec![variant_unit("Off"), variant_unit("On")],
        ),
        class_decl(
            ast::ClassModifier::Open,
            "OpenBase",
            Vec::new(),
            None,
            Vec::new(),
            Vec::new(),
        ),
        interface_decl("EmptyInterface", Vec::new()),
        class_decl(
            ast::ClassModifier::Final,
            "Derived",
            Vec::new(),
            Some(("OpenBase", Vec::new())),
            vec!["EmptyInterface"],
            Vec::new(),
        ),
        object_decl("EmptyObject"),
        generic_struct_decl("GenericValue", vec!["T"], Vec::new()),
    ])
}

fn lower_public_declarations(declarations: Vec<Decl>) -> hir::DependencyHirOutput {
    let core = trusted_core();
    let mut source = file(declarations);
    make_core_public(&mut source);
    let ordinary = parsed_ordinary(source);
    let core_inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
    let world = core.world(ordinary.cone());
    let input = CurrentConeSources::try_new(&ordinary, core_inputs, &world).unwrap();
    lower_current_cone(scoop_identity::RequestedConeKind::Library, &input).unwrap()
}

#[test]
fn producer_uses_real_ordinary_hir_for_param_free_nominals() {
    let output = lower_public_nominals();
    let public = public_interface(&output);
    let production = produce_cross_cone_type_semantics(&output, &public).unwrap();
    let section = production.section();

    assert_eq!(section.representation_support().records().len(), 7);
    assert_eq!(section.inheritance().records().len(), 7);
    assert_eq!(section.exact_facts().records().len(), 7);
    assert_eq!(production.dependency_facts().len(), 1);
    assert!(!section.definition_sources().is_empty());
    assert!(section.selected().records().is_empty());
    assert!(section.protected_declarations().records().is_empty());
    assert_eq!(section.protected_source_interfaces().records().len(), 4);
    assert!(section.protected_defaults().records().is_empty());
    assert_eq!(
        section
            .inheritance()
            .records()
            .iter()
            .map(|record| record.constructors().records().len())
            .sum::<usize>(),
        4
    );
    assert_eq!(production.local_inheritance_edges().len(), 7);
    assert!(production.local_inheritance_edges().iter().any(|edges| {
        matches!(
            edges.direct_base(),
            hir::DirectClassBaseV1::ClassBase { .. }
        )
    }));
    assert!(
        production
            .local_inheritance_edges()
            .iter()
            .any(|edges| !edges.direct_interfaces().is_empty())
    );
    assert_eq!(
        production.local_exact_facts().values(),
        section
            .exact_facts()
            .records()
            .iter()
            .map(|record| record.exact())
            .collect::<Vec<_>>()
    );

    let foundation = production.foundation();
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    let graph = hir::CheckedNominalInheritanceGraphV1::validate_with_source_roots(
        foundation.local_inheritance_edges().iter(),
        foundation.source_roots().iter().copied(),
        foundation,
        &mut meter,
    )
    .unwrap();
    for record in section.inheritance().records() {
        assert_eq!(graph.get(record.owner()).unwrap().edges(), record.edges());
        graph
            .validate_nominal_domains(record.owner(), record.domains(), &mut meter)
            .unwrap();
    }
    section
        .representation_support()
        .validate_source_semantics(foundation, &mut meter, &WirePath::root())
        .unwrap();

    let mut mutated_representations = section.representation_support().records().to_vec();
    let mutated = mutated_representations
        .iter_mut()
        .find(|record| {
            matches!(
                record.shape(),
                hir::NominalRepresentationShapeV1::Struct { fields, .. } if fields.len() == 1
            )
        })
        .unwrap();
    let key = foundation
        .nominal_declaration_key(hir::SourceNominalId::Concrete(mutated.owner()))
        .unwrap();
    *mutated = hir::NominalRepresentationSupportV1::try_new(
        key,
        mutated.declaration_access().clone(),
        hir::NominalRepresentationShapeV1::Struct {
            fields: Vec::new(),
            c_layout_policy: hir::NominalCLayoutPolicyV1::Ordinary,
        },
    )
    .unwrap();
    let mutated_representations =
        hir::CanonicalNominalRepresentationSupportV1::try_new(mutated_representations).unwrap();
    assert!(
        mutated_representations
            .validate_source_semantics(foundation, &mut meter, &WirePath::root())
            .is_err()
    );

    let wrong_domains = hir::NominalAccessDomainsV1::new(
        hir::PersistentLookupDomainV1::new(hir::PersistentAccessDomainV1::universal()),
        hir::PersistentInheritanceDomainV1::new(hir::PersistentAccessDomainV1::universal()),
        hir::PersistentSlotContractDomainV1::new(hir::PersistentAccessDomainV1::empty()),
    );
    assert!(
        graph
            .validate_nominal_domains(word_value_exact(section), &wrong_domains, &mut meter)
            .is_err()
    );

    let dependency_shapes = FactShapes(
        production
            .dependency_facts()
            .iter()
            .map(|dependency| (dependency.exact, hir::ExactTypeFactShapeV1::Scalar))
            .collect(),
    );
    let dependency_table = hir::CanonicalExactTypeFactsV1::try_new(
        production
            .dependency_facts()
            .iter()
            .map(|dependency| {
                hir::ExactTypeFactsV1::try_new(
                    dependency.exact,
                    hir::ExactTypeKindV1::Value {
                        zst: hir::ZstStatus::NonZero,
                    },
                    hir::ExactTypeGcV1::GcFree,
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    let checked_dependencies = dependency_table
        .validate_semantics(&dependency_shapes, &mut meter)
        .unwrap();
    section
        .exact_facts()
        .validate_semantics_with_dependencies(
            foundation,
            &DependencyFacts(checked_dependencies),
            &mut meter,
        )
        .unwrap();

    let generic = output
        .output()
        .export
        .structs
        .iter()
        .find_map(|(id, declaration)| (declaration.name == "GenericValue").then_some(id))
        .unwrap();
    let generic = output.output().export.nominal_identities[generic]
        .generic_type_id()
        .unwrap();
    assert!(
        production
            .source_roots()
            .contains(&NominalDeclarationOwner::GenericTemplate(generic))
    );
    assert!(
        section
            .representation_support()
            .records()
            .iter()
            .all(|record| {
                NominalDeclarationOwner::Concrete(record.owner())
                    != NominalDeclarationOwner::GenericTemplate(generic)
            })
    );

    let empty_value = section
        .representation_support()
        .records()
        .iter()
        .find(|record| {
            matches!(
                record.shape(),
                hir::NominalRepresentationShapeV1::Struct { fields, .. } if fields.is_empty()
            )
        })
        .unwrap();
    let exact = scoop_identity::PersistentExactTypeId::from_key(
        &scoop_identity::ExactTypeKey::Nominal(empty_value.owner()),
    )
    .unwrap();
    let facts = section.exact_facts().get(exact).unwrap();
    assert_eq!(
        facts.kind(),
        hir::ExactTypeKindV1::Value {
            zst: hir::ZstStatus::ZeroSized
        }
    );
    assert_eq!(facts.gc(), hir::ExactTypeGcV1::GcFree);

    let word_value = section
        .representation_support()
        .records()
        .iter()
        .find(|record| {
            matches!(
                record.shape(),
                hir::NominalRepresentationShapeV1::Struct { fields, .. } if fields.len() == 1
            )
        })
        .unwrap();
    let exact = scoop_identity::PersistentExactTypeId::from_key(
        &scoop_identity::ExactTypeKey::Nominal(word_value.owner()),
    )
    .unwrap();
    assert_eq!(
        section.exact_facts().get(exact).unwrap().kind(),
        hir::ExactTypeKindV1::Value {
            zst: hir::ZstStatus::NonZero
        }
    );

    let enumeration = section
        .representation_support()
        .records()
        .iter()
        .find(|record| {
            matches!(
                record.shape(),
                hir::NominalRepresentationShapeV1::Enum { .. }
            )
        })
        .unwrap();
    let exact = scoop_identity::PersistentExactTypeId::from_key(
        &scoop_identity::ExactTypeKey::Nominal(enumeration.owner()),
    )
    .unwrap();
    assert_eq!(
        section.exact_facts().get(exact).unwrap().kind(),
        hir::ExactTypeKindV1::Value {
            zst: hir::ZstStatus::NonZero
        }
    );
}

fn word_value_exact(section: &hir::CrossConeTypeSemanticsSectionV1) -> PersistentExactTypeId {
    let owner = section
        .representation_support()
        .records()
        .iter()
        .find(|record| {
            matches!(
                record.shape(),
                hir::NominalRepresentationShapeV1::Struct { fields, .. } if fields.len() == 1
            )
        })
        .unwrap()
        .owner();
    PersistentExactTypeId::from_key(&scoop_identity::ExactTypeKey::Nominal(owner)).unwrap()
}

mod additional;
