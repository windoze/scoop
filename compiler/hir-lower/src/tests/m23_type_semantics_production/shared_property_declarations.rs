use super::*;
use hir::{
    CanonicalCallableInterfacesV1 as Callables, CanonicalPropertyInterfacesV1 as Properties,
};
use scoop_identity::{
    AccessorRole, CallableTemplateOrigin, PropertyOwner, SourceDeclarationKey,
    ValidatedIdentityGraph,
};
use scoop_wire::{decode_canonical, encode};
use source_dispatch::with_hir_source;

mod render;

const STANDALONE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-shared-property-declarations/standalone.scoop"
));
const COMBINED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-shared-property-declarations/combined.scoop"
));

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[test]
fn ordinary_property_metadata_keeps_complete_restricted_accessors_and_generic_owners() {
    for (case, source) in [("standalone", STANDALONE), ("combined", COMBINED)] {
        with_hir_source(source, |output, _| {
            let export = output.output().export.module();
            let properties = Properties::from_export_hir_with_budget(export, &mut meter()).unwrap();
            let callables = Callables::from_export_hir(export).unwrap();
            properties.validate_accessor_closure(&callables).unwrap();
            assert!(!properties.support_records().is_empty());
            let foundation = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
            let mut identities =
                source_inventory::identity_closure_for_foundation(output, foundation);
            let decoded: hir::DecodedCanonicalPropertyInterfacesV1 =
                decode_canonical(&encode(&properties).unwrap(), DecodeLimits::default()).unwrap();
            assert_eq!(decoded.resolve(&mut identities).unwrap(), properties);
            let rows = render::table(&properties, &callables, &identities);
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                "../../tests/fixtures/m23-shared-property-declarations/{case}.snap"
            ));
            if std::env::var_os("SCOOP_UPDATE_SHARED_PROPERTY_SNAPSHOTS").is_some() {
                std::fs::write(&path, &rows).unwrap();
            }
            assert_eq!(rows, std::fs::read_to_string(path).unwrap());
        });
    }
}

#[test]
fn shared_property_inventory_rejects_omission_duplicate_and_public_promotion() {
    with_hir_source(STANDALONE, |output, _| {
        let export = output.output().export.module();
        let nominals = hir::CanonicalNominalInterfacesV1::from_export_hir(export).unwrap();
        let properties = Properties::from_export_hir(export).unwrap();
        for record in properties.support_records() {
            let remaining = properties
                .support_records()
                .iter()
                .filter(|other| other.declaration() != record.declaration())
                .cloned()
                .collect();
            let omitted =
                Properties::with_support(properties.records().to_vec(), remaining).unwrap();
            assert_eq!(
                omitted.validate_declaration_inventory(&nominals, &mut meter()),
                Err(hir::PropertyDeclarationInventoryError::Missing(
                    record.declaration()
                ))
            );
            assert!(properties.get(record.declaration()).is_none());
            assert_eq!(
                hir::PropertyInterfaceRecordV1::from_declaration(
                    record.clone(),
                    hir::PropertyPublicAccessV1::DirectOnly,
                    hir::PropertySetterPublicAccessV1::Restricted
                ),
                Err(
                    hir::PropertyInterfaceRecordBuildError::NonPublicDeclaration(
                        record.declared_visibility()
                    )
                )
            );
        }
        let public = properties.records()[0].clone();
        assert!(
            matches!(Properties::with_support(vec![public.clone()], vec![public.declaration_data().clone()]), Err(hir::PropertyInterfaceSetBuildError::DuplicateDeclaration(id)) if id == public.declaration())
        );
    });
}

#[test]
fn restricted_setter_is_required_and_checked_against_the_logical_property() {
    with_hir_source(STANDALONE, |output, _| {
        let export = output.output().export.module();
        let properties = Properties::from_export_hir(export).unwrap();
        let callables = Callables::from_export_hir(export).unwrap();
        let property = &properties.records()[0];
        let setter = property.accessors().setter().unwrap();
        let declaration = CallableTemplateOrigin::Accessor(setter);
        let original = callables.declaration(declaration).unwrap();
        assert_eq!(
            original.declared_visibility(),
            hir::DeclaredVisibilityV1::Private
        );
        assert_eq!(original.modality(), hir::CallableModalityV1::Final);
        assert!(callables.get(declaration).is_none());
        let support: Vec<_> = callables
            .support_records()
            .iter()
            .filter(|r| r.declaration() != declaration)
            .cloned()
            .collect();
        let missing =
            Callables::with_support(callables.records().to_vec(), support.clone()).unwrap();
        assert!(
            matches!(properties.validate_accessor_closure(&missing), Err(hir::PropertyAccessorClosureValidationError::MissingSourceAccessor { accessor, role: AccessorRole::Setter, .. }) if accessor == setter)
        );
        let wrong_result = hir::CallableDeclarationRecordV1::try_new(
            declaration,
            original.owner(),
            original.type_parameters().clone(),
            original.receiver().cloned(),
            original.parameters().clone(),
            property.value_type().clone(),
            original.effects(),
            original.modality(),
            original.declared_visibility(),
            original.slot_relations().clone(),
        )
        .unwrap();
        let mut wrong = support;
        wrong.push(wrong_result);
        let wrong = Callables::with_support(callables.records().to_vec(), wrong).unwrap();
        assert!(
            matches!(properties.validate_accessor_closure(&wrong), Err(hir::PropertyAccessorClosureValidationError::Result { accessor, .. }) if accessor == setter)
        );
    });
}

#[test]
fn property_projection_shares_budget_and_ignores_unrelated_private_top_level_declarations() {
    let project = |source: &str| {
        with_hir_source(source, |output, _| {
            let export = output.output().export.module();
            let mut measured = meter();
            let properties =
                Properties::from_export_hir_with_budget(export, &mut measured).unwrap();
            let mut bounded = BudgetMeter::new(DecodeLimits {
                validation_work_units: measured.usage().validation_work_units,
                ..DecodeLimits::default()
            });
            Properties::from_export_hir_with_budget(export, &mut bounded).unwrap();
            assert!(Properties::from_export_hir_with_budget(export, &mut bounded).is_err());
            encode(&properties).unwrap()
        })
    };
    let prefix = "private val unrelated: Int = 19\n";
    let padding = format!("//{}\n", " ".repeat(prefix.len() - 3));
    assert_eq!(
        project(&format!("{padding}{STANDALONE}")),
        project(&format!("{prefix}{STANDALONE}"))
    );
}
