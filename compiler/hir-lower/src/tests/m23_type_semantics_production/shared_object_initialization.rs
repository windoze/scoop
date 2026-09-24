use super::*;
use scoop_identity::{CborIdentityRecord, InitializationUnitKey};

const SOURCE: &str =
    "public class Owner { public companion object {} }\nprivate object Unrelated {}";

#[test]
fn shared_object_units_retain_source_only_keys_without_materializing_them() {
    source_dispatch::with_hir_source(SOURCE, |output, _| {
        let public = public_interface(output);
        let identities = source_inventory::identity_closure(output);
        let foundation = hir::OdrFreeHirFoundation::try_new(
            hir::CanonicalHirFoundation::from_type_semantics_output(output).unwrap(),
        )
        .unwrap();
        let metadata = hir::SharedTypeMetadataV1 {
            provider: output.output().export.cone,
            identities: &identities,
            foundation: &foundation,
            public: &public,
        };
        let mut meter = BudgetMeter::new(DecodeLimits::default());
        let units = metadata.object_initialization_units(&mut meter).unwrap();
        assert_eq!(units.len(), 2);
        let mut source_only = 0;
        for owner in units.keys() {
            if public
                .nominal_interfaces()
                .declaration(hir::SourceNominalId::Concrete(*owner))
                .is_none()
            {
                source_only += 1;
            }
        }
        assert_eq!(source_only, 1);
        assert!(matches!(
            hir::SharedTypeMetadataV1 {
                provider: ConeIdentity::CORE,
                ..metadata
            }
            .object_initialization_units(&mut meter),
            Err(hir::SharedTypeMetadataError::ObjectInitializationOwner(_))
        ));
        let mut limited = BudgetMeter::new(DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        });
        assert!(matches!(
            metadata.object_initialization_units(&mut limited),
            Err(hir::SharedTypeMetadataError::Resource(_))
        ));
    });
}

#[test]
fn shared_object_units_reject_conflicting_roles_and_non_object_owners() {
    source_dispatch::with_hir_source(SOURCE, |output, _| {
        let public = public_interface(output);
        let identities = source_inventory::identity_closure(output);
        let original = hir::CanonicalHirFoundation::from_type_semantics_output(output).unwrap();
        let export = output.output().export.module();
        let object = export
            .initialization_unit_identities
            .records()
            .iter()
            .find_map(|record| match record.key() {
                InitializationUnitKey::Companion(owner) => Some(*owner),
                _ => None,
            })
            .unwrap();
        let class = public
            .nominal_interfaces()
            .all_records()
            .find_map(
                |record| match (record.declaration(), record.source_shape()) {
                    (
                        hir::SourceNominalId::Concrete(owner),
                        hir::NominalSourceShapeV1::Class(_),
                    ) => Some(owner),
                    _ => None,
                },
            )
            .unwrap();
        for (keys, duplicate) in [
            (
                vec![
                    InitializationUnitKey::Object(object),
                    InitializationUnitKey::Companion(object),
                ],
                true,
            ),
            (vec![InitializationUnitKey::Object(class)], false),
        ] {
            let mut foundation = original.clone();
            foundation
                .set_initialization_units(
                    keys.into_iter()
                        .map(|key| CborIdentityRecord::from_key(key).unwrap())
                        .collect(),
                )
                .unwrap();
            let foundation = hir::OdrFreeHirFoundation::try_new(foundation).unwrap();
            let result = hir::SharedTypeMetadataV1 {
                provider: output.output().export.cone,
                identities: &identities,
                foundation: &foundation,
                public: &public,
            }
            .object_initialization_units(&mut BudgetMeter::new(DecodeLimits::default()));
            if duplicate {
                assert!(
                    matches!(result, Err(hir::SharedTypeMetadataError::DuplicateObjectInitialization(owner)) if owner == object)
                );
            } else {
                assert!(
                    matches!(result, Err(hir::SharedTypeMetadataError::ObjectInitializationOwner(owner)) if owner == class)
                );
            }
        }
    });
}
