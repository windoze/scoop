use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{ConeIdentity, NominalDeclarationOwner, PersistentExactTypeId};

use super::m23_ordinary_core_only::support::{parsed_ordinary, trusted_core};
use super::*;
use crate::{CurrentConeSources, lower_current_cone};

mod public_projection;
use public_projection::public_interface;

mod automatic_materialization;
mod declaration_dump;
mod declaration_type_sites;
mod executable_type_sites;
mod fact_providers;
mod generic_bodies;
mod generic_initializations;
mod materialization_requirements;
mod materialized_type_uses;
mod setter_domains;
mod shape_demands;
mod shared_accessor_forms;
mod shared_callable_declarations;
mod shared_callable_selection;
mod shared_constructor_selection;
mod shared_declaration_combinations;
mod shared_default_receivers;
mod shared_nominal_declarations;
mod shared_object_initialization;
mod shared_property_declarations;
mod source_constructors;
mod source_default_captures;
mod source_default_derived_order;
mod source_dispatch;
mod source_fields;
mod source_inventory;
mod source_mir_types;
mod source_native_boundary;
mod source_only_nominals;

mod materialized_selections;
mod production;
use production::produce_cross_cone_type_semantics;

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
    let section = &production;

    let roots = output.output().local.materialization().roots();
    assert_eq!(roots.len(), 7);
    for root in roots {
        assert!(
            section
                .representation_support()
                .get(root.source())
                .is_some()
        );
        assert!(section.inheritance().get(root.exact()).is_some());
        assert!(section.exact_facts().get(root.exact()).is_some());
    }
    assert!(!section.selected().records().is_empty());
    assert!(public.default_templates().records().is_empty());
    assert_eq!(
        hir::select_param_free_source_constructors(
            output.output().export.cone,
            &public,
            &source_inventory::identity_closure(&output),
        )
        .unwrap()
        .len(),
        4
    );
    assert!(section.inheritance().records().iter().any(|record| {
        matches!(
            record.edges().direct_base(),
            hir::DirectClassBaseV1::ClassBase { .. }
        )
    }));
    assert!(
        section
            .inheritance()
            .records()
            .iter()
            .any(|record| { !record.edges().direct_interfaces().is_empty() })
    );

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
        public
            .nominal_interfaces()
            .get(NominalDeclarationOwner::GenericTemplate(generic))
            .is_some()
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

mod additional;
