use super::*;
use crate::{
    CanonicalExactTypeFactsV1, ExactTypeFactShapeV1 as Shape, ExactTypeFactsSemanticAuthority,
    ExactTypeFactsSemanticError as Error, ExactTypeGcV1 as Gc, ExactTypeKindV1 as Kind, ZstStatus,
};
use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, ExactTypeKey,
    PackagePath, PersistentTypeId, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};

use std::{cell::Cell, collections::BTreeMap};

struct Shapes(BTreeMap<PersistentExactTypeId, Shape>);
impl ExactTypeFactsSemanticAuthority<&'static str> for Shapes {
    fn fact_shape(&self, exact: PersistentExactTypeId) -> Result<&Shape, &'static str> {
        self.0.get(&exact).ok_or("no local source shape")
    }
}
struct Dependencies<'a> {
    checked: CheckedExactTypeFactsV1<'a>,
    redirect: Option<PersistentExactTypeId>,
    calls: Cell<usize>,
}
impl ExactTypeFactsDependencyLookupV1 for Dependencies<'_> {
    fn get_dependency_fact(
        &self,
        exact: PersistentExactTypeId,
    ) -> Option<CheckedExactTypeFactV1<'_>> {
        self.calls.set(self.calls.get() + 1);
        self.checked.get_checked(self.redirect.unwrap_or(exact))
    }
}
fn exact(name: &str) -> PersistentExactTypeId {
    let key = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Struct,
        0,
    );
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        PersistentTypeId::from_source_declaration(&key).unwrap(),
    ))
    .unwrap()
}
fn value(exact: PersistentExactTypeId, zero: bool, gc: Gc) -> ExactTypeFactsV1 {
    ExactTypeFactsV1::try_new(
        exact,
        Kind::Value {
            zst: if zero {
                ZstStatus::ZeroSized
            } else {
                ZstStatus::NonZero
            },
        },
        gc,
    )
    .unwrap()
}

#[test]
fn local_struct_recurses_through_checked_dependencies_without_copying_their_rows() {
    let (number, reference, local) = (exact("Number"), exact("Reference"), exact("Local"));
    let external = CanonicalExactTypeFactsV1::try_new(vec![
        value(number, false, Gc::GcFree),
        ExactTypeFactsV1::try_new(reference, Kind::Reference, Gc::ContainsManagedReferences)
            .unwrap(),
    ])
    .unwrap();
    let source = Shapes(BTreeMap::from([
        (number, Shape::Scalar),
        (reference, Shape::Reference),
    ]));
    let dependencies = Dependencies {
        checked: external.validate_semantics(&source).unwrap(),
        redirect: None,
        calls: Cell::new(0),
    };
    let local_table = CanonicalExactTypeFactsV1::try_new(vec![value(
        local,
        false,
        Gc::ContainsManagedReferences,
    )])
    .unwrap();
    let local_source = Shapes(BTreeMap::from([(
        local,
        Shape::OrdinaryStruct {
            fields: vec![number, reference],
        },
    )]));
    let checked = local_table
        .validate_semantics_with_dependencies(&local_source, &dependencies)
        .unwrap();
    assert_eq!(checked.records(), local_table.records());
    assert_eq!(checked.records().len(), 1);
    assert!(checked.get(number).is_none());
    assert_eq!(dependencies.calls.get(), 2);
}

#[test]
fn missing_and_wrong_identity_dependency_facts_are_rejected() {
    let (field, wrong, local) = (exact("Field"), exact("Wrong"), exact("Local"));
    let external =
        CanonicalExactTypeFactsV1::try_new(vec![value(wrong, false, Gc::GcFree)]).unwrap();
    let mut dependencies = Dependencies {
        checked: external
            .validate_semantics(&Shapes(BTreeMap::from([(wrong, Shape::Scalar)])))
            .unwrap(),
        redirect: None,
        calls: Cell::new(0),
    };
    let local_table =
        CanonicalExactTypeFactsV1::try_new(vec![value(local, false, Gc::GcFree)]).unwrap();
    let source = Shapes(BTreeMap::from([(
        local,
        Shape::OrdinaryStruct {
            fields: vec![field],
        },
    )]));
    assert!(
        matches!(local_table.validate_semantics_with_dependencies(&source, &dependencies),
        Err(Error::MissingFacts(id)) if id == field)
    );
    dependencies.redirect = Some(wrong);
    assert!(
        matches!(local_table.validate_semantics_with_dependencies(&source, &dependencies),
        Err(Error::DependencyIdentity { expected, actual }) if expected == field && actual == wrong)
    );
}

#[test]
fn external_zero_sized_facts_preserve_struct_and_c_layout_rules() {
    let (unit, local) = (exact("Unit"), exact("Local"));
    let external = CanonicalExactTypeFactsV1::try_new(vec![value(unit, true, Gc::GcFree)]).unwrap();
    let dependencies = Dependencies {
        checked: external
            .validate_semantics(&Shapes(BTreeMap::from([(unit, Shape::Unit)])))
            .unwrap(),
        redirect: None,
        calls: Cell::new(0),
    };
    let ordinary =
        CanonicalExactTypeFactsV1::try_new(vec![value(local, true, Gc::GcFree)]).unwrap();
    let source = Shapes(BTreeMap::from([(
        local,
        Shape::OrdinaryStruct { fields: vec![unit] },
    )]));
    ordinary
        .validate_semantics_with_dependencies(&source, &dependencies)
        .unwrap();
    let c_layout =
        CanonicalExactTypeFactsV1::try_new(vec![value(local, false, Gc::GcFree)]).unwrap();
    let source = Shapes(BTreeMap::from([(
        local,
        Shape::CLayoutStruct { fields: vec![unit] },
    )]));
    assert!(
        matches!(c_layout.validate_semantics_with_dependencies(&source, &dependencies),
        Err(Error::ZeroSizedCLayoutField { owner, field }) if owner == local && field == unit)
    );
}

#[test]
fn a_local_record_cannot_be_replaced_by_a_conflicting_dependency_fact() {
    let local = exact("Local");
    let external =
        CanonicalExactTypeFactsV1::try_new(vec![value(local, true, Gc::GcFree)]).unwrap();
    let dependencies = Dependencies {
        checked: external
            .validate_semantics(&Shapes(BTreeMap::from([(local, Shape::Unit)])))
            .unwrap(),
        redirect: None,
        calls: Cell::new(0),
    };
    let local_table =
        CanonicalExactTypeFactsV1::try_new(vec![value(local, true, Gc::GcFree)]).unwrap();
    let source = Shapes(BTreeMap::from([(local, Shape::Scalar)]));
    assert!(matches!(
        local_table.validate_semantics_with_dependencies(&source, &dependencies),
        Err(Error::Mismatch { .. })
    ));
    assert_eq!(dependencies.calls.get(), 0);
}
