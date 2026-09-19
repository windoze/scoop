use super::*;
use crate::cross_cone_type_semantics::inheritance::inheritance_interface_fixture;
use crate::*;
use scoop_identity::*;
use scoop_wire::DecodeLimits;

mod cases;
mod nested;
mod protocol;
use protocol::Authority;

#[derive(Clone, Copy)]
enum Case {
    Valid,
    Missing,
    Extra,
    WrongParameters,
    MissingDefault,
    Nested,
    NestedMissing,
}
fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

fn validate(
    case: Case,
    limits: DecodeLimits,
) -> Result<usize, ProtectedSourceClosureError<&'static str>> {
    let mut bundle = inheritance_interface_fixture();
    let mut nested_owner = None;
    let mut expected = bundle
        .protected
        .records()
        .iter()
        .filter_map(|record| match record {
            ProtectedDeclarationInterfaceV1::Callable(record) => Some(record.declaration()),
            ProtectedDeclarationInterfaceV1::Constructor(record) => {
                Some(CallableTemplateOrigin::Constructor(record.declaration()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    for owner in bundle.table.records() {
        expected.extend(
            owner
                .constructors()
                .records()
                .iter()
                .map(|record| CallableTemplateOrigin::Constructor(record.declaration())),
        );
    }
    if matches!(case, Case::Nested | Case::NestedMissing) {
        let (record, owners) = nested::fixture(&mut bundle.fixture, bundle.base.source);
        nested_owner = owners.last().copied();
        expected.extend(owners);
        let reference = record.reference();
        let mut records = bundle.protected.records().to_vec();
        records.push(record);
        bundle.fixture.protected_roots = CanonicalProtectedDeclarationRefsV1::try_new(
            records
                .iter()
                .map(ProtectedDeclarationInterfaceV1::reference)
                .collect(),
        )
        .unwrap();
        bundle.protected = CanonicalProtectedDeclarationInterfacesV1::try_new(records).unwrap();
        let mut members = bundle
            .table
            .get(bundle.base.exact)
            .unwrap()
            .protected_members()
            .values()
            .to_vec();
        members.push(reference);
        let members = CanonicalProtectedDeclarationRefsV1::try_new(members).unwrap();
        bundle
            .fixture
            .inheritance_interfaces
            .members
            .insert(bundle.base.exact, members.clone());
        bundle.change(bundle.base, |record| {
            *record = NominalInheritanceInterfaceV1::try_new(
                record.edges().clone(),
                record.domains().clone(),
                record.constructors().clone(),
                record.slots().clone(),
                members,
                record.slot_schemas().clone(),
            )
            .unwrap();
        });
    }
    expected.sort_unstable();
    expected.dedup();
    let expected_count = expected.len();
    let mut records = expected
        .iter()
        .map(|owner| {
            ProtectedCallableSourceInterfaceV1::try_new(
                *owner,
                CanonicalProtectedSourceParametersV1::try_new(vec![]).unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    match case {
        Case::Missing => {
            records.pop();
        }
        Case::NestedMissing => records.retain(|record| Some(record.owner()) != nested_owner),
        Case::Extra => {
            let InheritanceCallableDeclarationV1::Function(extra) = bundle.target else {
                panic!("function target");
            };
            records.push(
                ProtectedCallableSourceInterfaceV1::try_new(
                    CallableTemplateOrigin::Function(extra),
                    CanonicalProtectedSourceParametersV1::try_new(vec![]).unwrap(),
                )
                .unwrap(),
            );
        }
        Case::WrongParameters => {
            records[0] = ProtectedCallableSourceInterfaceV1::try_new(records[0].owner(), CanonicalProtectedSourceParametersV1::try_new(vec![
                ProtectedSourceParameterV1::new(CanonicalIdentifier::new("wrong").unwrap(), SignatureTypeKey::Nominal(crate::cross_cone_type_semantics::protected_interfaces::tests::support::nominal(bundle.fixture.unit)), ProtectedParameterCallingV1::Required, bundle.fixture.graph.origins[&bundle.base.source].clone()),
            ]).unwrap()).unwrap();
        }
        Case::Valid | Case::MissingDefault | Case::Nested => {}
    }
    let keys = ProtectedDefaultKeyIndexV1::try_new(if matches!(case, Case::MissingDefault) {
        vec![ProtectedDefaultTemplateKeyV1::try_new(expected[0], 0).unwrap()]
    } else {
        vec![]
    })
    .unwrap();
    let table = CanonicalProtectedCallableSourceInterfacesV1::try_new(records).unwrap();
    let graph_source = bundle.fixture.graph.clone();
    let graph = CheckedNominalInheritanceGraphV1::validate_with_source_roots(
        graph_source.records.values(),
        graph_source.keys.keys().copied(),
        &graph_source,
        &mut meter(),
    )
    .unwrap();
    let protected = bundle
        .protected
        .validate_sources(
            &graph,
            &CanonicalNominalRepresentationSupportV1::default(),
            &mut bundle.fixture,
            &mut meter(),
        )
        .unwrap();
    let inheritance = bundle
        .table
        .validate_interfaces(&graph, protected, &mut bundle.fixture, &mut meter())
        .unwrap();
    let checked = table.validate_protocols(
        protected,
        inheritance,
        &graph,
        &keys,
        &mut bundle.fixture,
        &mut Authority,
        &mut BudgetMeter::new(limits),
    )?;
    assert_eq!(checked.entries().len(), expected_count);
    assert_eq!(checked.table(), &table);
    for owner in expected {
        let source = checked.get(owner).unwrap();
        assert_eq!(source.owner(), source.protocol().record().owner());
        let replay = source
            .validate_owner_source(&graph, &mut bundle.fixture, &mut meter())
            .unwrap();
        assert_eq!(replay.declaration(), owner);
        assert_eq!(replay.payload(), source.payload());
        assert_eq!(
            replay.declaration_access().source(),
            source.declaration_access()
        );
    }
    Ok(checked.entries().len())
}
