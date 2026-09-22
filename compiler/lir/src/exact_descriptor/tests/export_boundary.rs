use super::*;
use scoop_wire::BudgetMeter;

#[test]
fn descriptor_exports_require_exact_dispatch_coverage() {
    let fixture = Fixture::new();
    for (descriptor, dispatch) in [(true, true), (false, false), (true, false), (false, true)] {
        let result = CrossConeLayoutAbiSectionV1::try_new(
            exports(&fixture, descriptor, dispatch),
            &[],
            Vec::new(),
            &NoDependencies,
            &mut fixture.meter(),
        );
        match (descriptor, dispatch) {
            (true, true) | (false, false) => {
                let section = result.unwrap();
                assert_eq!(
                    section.descriptors().records().len(),
                    usize::from(descriptor)
                );
                assert_eq!(section.dispatch().records().len(), usize::from(dispatch));
                assert_eq!(fixture.foundation().dispatch_tables().len(), 1);
            }
            (true, false) => assert!(matches!(
                result,
                Err(LayoutAbiSectionError::Dispatch(ExactDispatchTableError::Missing(table)))
                    if table == fixture.vtable()
            )),
            (false, true) => assert!(matches!(
                result,
                Err(LayoutAbiSectionError::Dispatch(
                    ExactDispatchTableError::Count {
                        expected: 0,
                        actual: 1,
                    }
                ))
            )),
        }
    }
}

fn exports(fixture: &Fixture, descriptor: bool, dispatch: bool) -> LayoutAbiExportConstituentsV1 {
    let target = LirTargetProfile::DARWIN_AARCH64;
    let foundation = fixture.foundation();
    let mut meter = fixture.meter();
    let record = fixture.replay(fixture.semantic()).unwrap();
    let layouts = CanonicalExactLayoutExportsV1::try_new(
        target,
        foundation,
        vec![
            record.value_layout().clone().into(),
            record.instance_layout().clone().into(),
        ],
        &mut meter,
    )
    .unwrap();
    let descriptors = CanonicalExactDescriptorExportsV1::try_new(
        target,
        foundation,
        descriptor.then_some(record).into_iter().collect(),
        &mut meter,
    )
    .unwrap();
    let identity = TypeDescriptorIdentity::new(
        RuntimeTypeMappingRecord::new(fixture.exact()).unwrap(),
        MaterializationRoot::cone_owned(),
    )
    .unwrap();
    let vtable = VtableRecord::new(&identity, Vec::new()).unwrap();
    let record = ExactDispatchExportV1::replay(
        target,
        (&vtable).into(),
        &[],
        foundation,
        &mut |_, _: &mut BudgetMeter| Ok(None),
        &mut meter,
    )
    .unwrap();
    let dispatch = CanonicalExactDispatchExportsV1::try_new(
        target,
        foundation,
        dispatch.then_some(record).into_iter().collect(),
        &mut meter,
    )
    .unwrap();
    let callables =
        CanonicalExactCallableAbiExportsV1::try_new(target, foundation, Vec::new(), &mut meter)
            .unwrap();
    let shapes = CanonicalParamFreeShapeSupportExportsV1::from_sources(
        &[],
        &layouts,
        &descriptors,
        foundation,
        &mut meter,
    )
    .unwrap();
    LayoutAbiExportConstituentsV1::try_new(layouts, descriptors, dispatch, callables, shapes)
        .unwrap()
}

struct NoDependencies;

impl LayoutAbiSectionSourceAuthorityV1<()> for NoDependencies {
    fn validate_local_exports(
        &self,
        _: &LayoutAbiExportConstituentsV1,
        _: &mut BudgetMeter,
    ) -> Result<(), ()> {
        Ok(())
    }
    fn committed_semantic_roots(&self) -> Result<&[LayoutAbiDependencyV1], ()> {
        Ok(&[])
    }
    fn validate_physical_imports(
        &self,
        imports: &[ExternalShapeLinkImportV1<'_>],
        _: &mut BudgetMeter,
    ) -> Result<(), ()> {
        imports.is_empty().then_some(()).ok_or(())
    }
}
