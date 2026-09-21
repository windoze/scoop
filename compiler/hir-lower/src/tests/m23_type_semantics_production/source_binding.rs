use super::source_inventory::identity_closure;
use super::*;
use scoop_identity::{ConeCoordinate, ValidatedIdentityGraph};
use scoop_wire::{decode_canonical, encode};

mod constructors;
mod declaration_domain;
mod default_access;
mod default_origins;
mod dispatch;
mod dispatch_binding;
mod inheritance;
mod nominal_constructors;
mod nominal_dispatch;
mod nominal_members;
mod nominal_nested;
mod nominal_parameters;
mod nominals;
mod parameter_candidates;
mod parameters;
mod properties;
mod protected_callables;
mod protected_declarations;
mod rejection;
mod replay;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

struct Fixture {
    source: hir::TypeFoundationSourceAuthorityV1,
    foundation: hir::OdrFreeHirFoundation,
    identities: ValidatedIdentityGraph,
}

impl Fixture {
    fn from_output(output: &hir::OrdinaryHirOutput<'_>) -> Self {
        let source =
            hir::CrossConeTypeSemanticsFoundationV1::from_ordinary_hir(output, &mut meter())
                .unwrap()
                .source_transcript(&mut meter())
                .unwrap();
        let canonical = hir::CanonicalHirFoundation::from_type_semantics_output(output).unwrap();
        let mut identities = identity_closure(output);
        let decoded: hir::DecodedHirFoundation =
            decode_canonical(&encode(&canonical).unwrap(), DecodeLimits::default()).unwrap();
        let coordinate = ConeCoordinate::new("test", "scoop-hir-lower", "0.0.0").unwrap();
        let foundation = hir::OdrFreeHirFoundation::from_validated(
            decoded
                .validate_with_dependency_sources(&coordinate, &mut identities, &mut meter())
                .unwrap(),
        )
        .unwrap();
        let decoded: hir::DecodedTypeFoundationSourceAuthorityV1 =
            decode_canonical(&encode(&source).unwrap(), DecodeLimits::default()).unwrap();
        let source = decoded.resolve(&mut identities, &mut meter()).unwrap();
        Self {
            source,
            foundation,
            identities,
        }
    }

    fn bind(
        &self,
    ) -> Result<hir::BoundTypeFoundationSourcesV1<'_>, hir::TypeFoundationBindingError> {
        self.source
            .bind_to_foundation(&self.foundation, &self.identities, &mut meter())
    }
}

#[test]
fn real_source_transcript_binds_to_byte_restored_hir_foundation() {
    let fixture = Fixture::from_output(&lower_public_nominals());
    let bound = fixture.bind().unwrap();
    for exact in fixture.source.entries().exact_keys.values() {
        assert_eq!(
            scoop_identity::PersistentExactTypeId::from_key(bound.exact_type_key(*exact).unwrap())
                .unwrap(),
            *exact
        );
    }
    for source in fixture.source.entries().sources.records() {
        assert_eq!(
            hir::SourceNominalId::from_source_declaration(
                bound.nominal_key(source.owner()).unwrap()
            )
            .unwrap(),
            source.owner()
        );
        assert!(bound.contains_definition_source(source.access().definition_origin()));
    }
    assert_eq!(bound.source(), &fixture.source);
}

#[test]
fn binding_rejects_keys_absent_from_owner_even_when_dependency_graph_resolves_them() {
    let fixture = Fixture::from_output(&lower_public_nominals());
    for field in [2, 3, 5] {
        let mut canonical = fixture.foundation.as_canonical().clone();
        match field {
            2 => canonical.set_exact_types(vec![]).unwrap(),
            3 => canonical.set_types(vec![]).unwrap(),
            5 => canonical.set_generated_types(vec![]).unwrap(),
            _ => unreachable!(),
        }
        let incomplete = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
        let error = fixture
            .source
            .bind_to_foundation(&incomplete, &fixture.identities, &mut meter())
            .unwrap_err();
        assert!(matches!(
            (field, error),
            (2, hir::TypeFoundationBindingError::MissingExact(_))
                | (3, hir::TypeFoundationBindingError::MissingNominal(_))
                | (5, hir::TypeFoundationBindingError::MissingGenerated(_))
        ));
    }
}

#[test]
fn binding_requires_the_same_validated_identity_graph() {
    let fixture = Fixture::from_output(&lower_public_nominals());
    let empty = scoop_identity::PendingIdentityValidation::new()
        .finish()
        .unwrap();
    assert!(matches!(
        fixture
            .source
            .bind_to_foundation(&fixture.foundation, &empty, &mut meter()),
        Err(hir::TypeFoundationBindingError::Identity(_))
    ));
}

#[test]
fn binding_checks_index_budget_before_publishing_borrowed_keys() {
    let fixture = Fixture::from_output(&lower_public_nominals());
    for limits in [
        DecodeLimits {
            semantic_table_entries: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            decoded_nodes: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(matches!(
            fixture.source.bind_to_foundation(
                &fixture.foundation,
                &fixture.identities,
                &mut BudgetMeter::new(limits)
            ),
            Err(hir::TypeFoundationBindingError::Resource(_))
        ));
    }
}

mod default_declarations;
