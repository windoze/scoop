use super::super::surface_fixture::{declaration_front, nominal_surface};
use super::default_value_access::declarations::restrict;
use super::*;
use crate::cross_cone_hir_authority::{
    CanonicalCrossConeHirSurfaceAuthority, CrossConeHirDefaultCallableAccessError as Error,
};
use scoop_hir::*;
use scoop_identity::{DefinitionOrigin, OptionalSignatureType};
use scoop_wire::WirePath;

mod nested;
mod providers;
mod support;

#[test]
fn callable_access_reads_function_and_generic_declarations_with_all_owner_restrictions() {
    let bytes = support::surface(cone());
    let original = declaration_front(&bytes);
    let declarations: Vec<_> = original
        .hir_interface
        .callable_interfaces()
        .all_declarations()
        .filter_map(|r| match r.declaration() {
            CallableTemplateOrigin::Function(id) => {
                Some((r.declaration(), DefinitionOriginSubject::Function(id)))
            }
            CallableTemplateOrigin::GenericFunction(id) => Some((
                r.declaration(),
                DefinitionOriginSubject::GenericFunction(id),
            )),
            _ => None,
        })
        .collect();
    assert_eq!(declarations.len(), 2);
    for (declaration, subject) in declarations {
        for visibility in [
            DeclaredVisibilityV1::Private,
            DeclaredVisibilityV1::Internal,
        ] {
            let mut front = declaration_front(&bytes);
            support::install_call(&mut front, declaration);
            support::validate(&mut front).unwrap();
            restrict(&mut front, subject, visibility);
            assert!(matches!(support::failure(&mut front), Error::WitnessDomain));
        }
    }
    let mut front = declaration_front(&bytes);
    let nominal = front.hir_interface.nominal_interfaces().records()[0].declaration();
    let method = front
        .hir_interface
        .callable_interfaces()
        .records()
        .iter()
        .find(|r| matches!(r.declaration(), CallableTemplateOrigin::Function(_)))
        .unwrap()
        .declaration();
    support::install_call(&mut front, method);
    let SourceNominalId::Concrete(nominal) = nominal else {
        panic!("nominal fixture")
    };
    restrict(
        &mut front,
        DefinitionOriginSubject::Type(nominal),
        DeclaredVisibilityV1::Internal,
    );
    assert!(matches!(support::failure(&mut front), Error::WitnessDomain));
}

#[test]
fn callable_access_uses_each_accessor_visibility_instead_of_its_public_property() {
    let bytes = support::surface(cone());
    let mut front = declaration_front(&bytes);
    let properties: Vec<_> = front
        .hir_interface
        .property_interfaces()
        .all_declarations()
        .map(|p| p.accessors())
        .collect();
    for accessors in properties {
        support::install_call(
            &mut front,
            CallableTemplateOrigin::Accessor(accessors.getter()),
        );
        support::validate(&mut front).unwrap();
        if let Some(setter) = accessors.setter() {
            support::install_call(&mut front, CallableTemplateOrigin::Accessor(setter));
            assert!(matches!(support::failure(&mut front), Error::WitnessDomain));
        }
    }
}

#[test]
fn callable_access_replays_derived_equality_from_actual_nominal_visibility() {
    let bytes = support::surface(cone());
    let mut front = declaration_front(&bytes);
    let (owner, origin, ty) = support::context(&front);
    let value = support::value(&ty, &origin);
    support::install(
        &mut front,
        owner,
        origin,
        ty.clone(),
        DefaultExpressionKindV1::MethodCall {
            receiver: Box::new(value.clone()),
            callee: DefaultMethodCalleeV1::DerivedEquality {
                owner_type: ty.clone(),
            },
            arguments: vec![value],
        },
        ExportDefaultCallableTargetV1::DerivedEquality {
            owner_type: ty.clone(),
        },
    );
    support::validate(&mut front).unwrap();
    let SignatureTypeKey::Nominal(nominal) = ty else {
        panic!("nominal fixture")
    };
    restrict(
        &mut front,
        DefinitionOriginSubject::Type(nominal),
        DeclaredVisibilityV1::Private,
    );
    assert!(matches!(support::failure(&mut front), Error::WitnessDomain));
}

#[test]
fn callable_access_rejects_reference_origin_mismatches_and_keeps_the_artifact_budget() {
    let bytes = support::surface(cone());
    let mut front = declaration_front(&bytes);
    let (owner, _, _) = support::context(&front);
    support::install_call(&mut front, owner);
    let before = front
        .graph
        .envelope
        .meter_mut()
        .usage()
        .validation_work_units;
    support::validate(&mut front).unwrap();
    let meter = front.graph.envelope.meter_mut();
    let work = meter.usage().validation_work_units - before;
    assert!(work > 1);
    let remaining = meter.limits().validation_work_units - meter.usage().validation_work_units;
    meter
        .charge_work(remaining - work + 1, &WirePath::root())
        .unwrap();
    assert!(
        format!("{:?}", support::validate(&mut front).unwrap_err()).contains("ValidationWorkUnits")
    );

    let mut front = declaration_front(&bytes);
    support::install_call(&mut front, owner);
    let template = &front.hir_interface.default_templates().records()[0];
    let original = &template.references().callables()[0];
    let context = front
        .foundations
        .hir
        .source_context_key(original.definition_origin().origin().context())
        .unwrap();
    let changed = ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(
            original.definition_origin().origin().source().clone(),
            SourceSpan::new(0, 0).unwrap(),
            context,
        )
        .unwrap(),
    );
    let references = ExportDefaultReferenceSetV1::try_new(
        vec![ExportDefaultReferenceV1::new(
            original.target().clone(),
            changed,
            original.witness().clone(),
        )],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    support::replace_references(&mut front, references);
    let Err(Error::Template { key, source }) = support::validate(&mut front) else {
        panic!("origin mismatch")
    };
    assert_eq!(key.owner(), owner);
    assert!(matches!(*source, Error::ReferenceMatch { .. }));
}
