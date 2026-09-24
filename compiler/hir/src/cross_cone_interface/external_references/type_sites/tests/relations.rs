use crate::{
    CanonicalDependencyBindingWitnessesV1, CanonicalExternalHirReferencesV1,
    CanonicalHirDependencyCallSitesV1,
};
use scoop_identity::{
    CborIdentityRecord, ConeIdentity, CoreBuiltinNominal, ExactTypeKey, NominalDeclarationOwner,
    NonEmptyVec, PendingIdentityValidation, PersistentTypeId, ValidatedIdentityGraph,
};

use super::*;
type Error = HirDependencyTypeRelationError;

struct Types {
    fixture: Fixture,
    graph: ValidatedIdentityGraph,
    unit: PersistentTypeId,
    any: PersistentTypeId,
    tuple: PersistentExactTypeId,
}

impl Types {
    fn new() -> Self {
        let fixture = Fixture::new();
        let mut pending = PendingIdentityValidation::new();
        for provider in [fixture.current, fixture.provider, ConeIdentity::CORE] {
            pending.register_authority(provider).unwrap();
        }
        pending
            .register_external_canonical_authority(fixture.function.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(fixture.context.clone())
            .unwrap();
        let unit = CoreBuiltinNominal::Unit.identity_record();
        let any = CoreBuiltinNominal::Any.identity_record();
        for declaration in [&unit, &any] {
            pending
                .register_external_canonical_authority(declaration.clone())
                .unwrap();
        }
        let exact = |owner| {
            CborIdentityRecord::<PersistentExactTypeId, _>::from_key(ExactTypeKey::Nominal(owner))
                .unwrap()
        };
        let unit_exact = exact(unit.id());
        let any_exact = exact(any.id());
        let tuple = CborIdentityRecord::<PersistentExactTypeId, _>::from_key(ExactTypeKey::Tuple(
            NonEmptyVec::from_first(unit_exact.id(), [any_exact.id()]),
        ))
        .unwrap();
        for record in [unit_exact, any_exact, tuple.clone()] {
            pending
                .register_external_canonical_authority(record)
                .unwrap();
        }
        Self {
            fixture,
            graph: pending.finish().unwrap(),
            unit: unit.id(),
            any: any.id(),
            tuple: tuple.id(),
        }
    }

    fn occurrence(
        &self,
        exact: PersistentExactTypeId,
        role: HirExpressionTypeRoleV1,
    ) -> HirDependencyTypeSiteV1 {
        let original = site(&self.fixture, 0, role);
        HirDependencyTypeSiteV1::new(original.position(), original.origin().clone(), role, exact)
    }

    fn reference(
        &self,
        owner: PersistentTypeId,
        sites: Vec<HirDependencyTypeSiteV1>,
    ) -> ExternalHirReferenceV1 {
        ExternalHirReferenceV1::try_new(
            ConeIdentity::CORE,
            ExternalHirTargetV1::Nominal(NominalDeclarationOwner::Concrete(owner)),
            CanonicalExternalHirReferenceRolesV1::try_new(vec![
                ExternalHirReferenceRoleV1::ExecutableTypeDependency,
            ])
            .unwrap(),
            CanonicalDependencyBindingWitnessesV1::try_new(vec![]).unwrap(),
            Default::default(),
            CanonicalHirDependencyTypeSitesV1::try_new(sites, &mut meter()).unwrap(),
        )
        .unwrap()
    }

    fn table(&self, references: Vec<ExternalHirReferenceV1>) -> CanonicalExternalHirReferencesV1 {
        CanonicalExternalHirReferencesV1::try_new_metered(references, &mut meter()).unwrap()
    }

    fn validate(&self, table: &CanonicalExternalHirReferencesV1) -> Result<(), Error> {
        table.validate_type_site_relations(
            self.fixture.current,
            &self.graph,
            &[ConeIdentity::CORE],
            &mut meter(),
        )
    }
}

#[test]
fn a_structural_type_requires_the_same_occurrence_in_every_foreign_nominal_record() {
    let types = Types::new();
    let occurrence = types.occurrence(types.tuple, HirExpressionTypeRoleV1::Value);
    let unit = types.reference(types.unit, vec![occurrence.clone()]);
    let any = types.reference(types.any, vec![occurrence]);
    let complete = types.table(vec![unit.clone(), any]);
    types.validate(&complete).unwrap();
    assert!(matches!(
        types.validate(&types.table(vec![unit])),
        Err(Error::NominalClosure(_))
    ));
    let wrong = types.table(vec![types.reference(
        types.any,
        vec![types.occurrence(types.fixture.unit, HirExpressionTypeRoleV1::Value)],
    )]);
    assert!(matches!(
        types.validate(&wrong),
        Err(Error::NominalClosure(_))
    ));
    assert!(matches!(
        complete.validate_type_site_relations(
            types.fixture.current,
            &types.graph,
            &[],
            &mut meter()
        ),
        Err(Error::UnreachableNominal { .. })
    ));
}

#[test]
fn one_position_cannot_name_conflicting_types_or_two_operand_operations() {
    let types = Types::new();
    let wrong = types.table(vec![
        types.reference(
            types.unit,
            vec![types.occurrence(types.tuple, HirExpressionTypeRoleV1::Value)],
        ),
        types.reference(
            types.any,
            vec![types.occurrence(types.fixture.unit, HirExpressionTypeRoleV1::Value)],
        ),
    ]);
    assert!(matches!(
        types.validate(&wrong),
        Err(Error::ConflictingPosition(_))
    ));
    let wrong = types.table(vec![types.reference(
        types.unit,
        vec![
            types.occurrence(types.fixture.unit, HirExpressionTypeRoleV1::SizeOf),
            types.occurrence(types.fixture.unit, HirExpressionTypeRoleV1::AlignOf),
        ],
    )]);
    assert!(matches!(
        types.validate(&wrong),
        Err(Error::ConflictingPosition(_))
    ));
}

#[test]
fn actual_call_results_require_an_equal_value_type_occurrence() {
    let types = Types::new();
    let call = ExternalHirReferenceV1::try_new(
        types.fixture.provider,
        types.fixture.target(),
        CanonicalExternalHirReferenceRolesV1::try_new(vec![
            ExternalHirReferenceRoleV1::ConcreteSelectedUse,
        ])
        .unwrap(),
        types.fixture.witnesses(),
        CanonicalHirDependencyCallSitesV1::try_new(vec![types.fixture.site(0, vec![0]).unwrap()])
            .unwrap(),
        Default::default(),
    )
    .unwrap();
    assert!(matches!(
        types.validate(&types.table(vec![call.clone()])),
        Err(Error::CallResult(_))
    ));
    let table = types.table(vec![
        call,
        types.reference(
            types.unit,
            vec![types.occurrence(types.fixture.unit, HirExpressionTypeRoleV1::Value)],
        ),
    ]);
    types.validate(&table).unwrap();
    let mut baseline = meter();
    table
        .validate_type_site_relations(
            types.fixture.current,
            &types.graph,
            &[ConeIdentity::CORE],
            &mut baseline,
        )
        .unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: baseline.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    table
        .validate_type_site_relations(
            types.fixture.current,
            &types.graph,
            &[ConeIdentity::CORE],
            &mut shared,
        )
        .unwrap();
    assert!(
        table
            .validate_type_site_relations(
                types.fixture.current,
                &types.graph,
                &[ConeIdentity::CORE],
                &mut shared
            )
            .is_err()
    );
}
