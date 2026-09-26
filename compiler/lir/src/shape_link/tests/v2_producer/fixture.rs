use scoop_identity::*;
use scoop_wire::WireError;

use super::TARGET;
use super::initialization_fixture::attach_eager_initialization;
use super::lir_fixture::{
    DiagnosticGraph, exact, exact_layouts, function, nominal, provider_module,
};
use crate::*;

pub(super) struct Provider {
    pub(super) coordinate: ConeCoordinate,
    pub(super) identity: ConeIdentity,
    pub(super) exact: PersistentExactTypeId,
    pub(super) callable: StrongCallableDefinitionOwner,
    pub(super) callable_body: PersistentCallableBodyId,
    pub(super) initialization_unit: PersistentInitializationUnitId,
    pub(super) output: SingleConeStrongLirOutput,
    pub(super) section: StrongProductionSectionV2,
    pub(super) ordinary: CrossConeLirBridgeSectionV1,
    layouts: CanonicalExactLayoutExportsV1,
    descriptors: CanonicalExactDescriptorExportsV1,
    dispatch: CanonicalExactDispatchExportsV1,
    callables: CanonicalExactCallableAbiExportsV1,
}

impl Provider {
    pub(super) fn new() -> Self {
        Self::for_coordinate(ConeCoordinate::reserved_single_file())
    }

    pub(super) fn for_coordinate(coordinate: ConeCoordinate) -> Self {
        let identity = coordinate.identity().unwrap();
        let source = nominal(identity, "Parent");
        let exact_record = exact(&source);
        let exact = exact_record.id();
        let callable = StrongCallableDefinitionOwner::Function(function(identity, "dispatch"));
        let callable_body =
            PersistentCallableBodyId::from_key(&CallableBodyKey::strong(callable)).unwrap();
        let diagnostics = DiagnosticGraph::new(coordinate.clone(), source, exact_record.clone());
        let diagnostic_name =
            CanonicalExactTypeDiagnosticName::from_validated_graph(exact, &diagnostics)
                .unwrap()
                .as_str()
                .to_owned();
        let mut module = provider_module(identity, exact_record.clone(), callable, diagnostic_name);
        let initialization_unit = attach_eager_initialization(&mut module, "providerValue", exact);
        super::lir_fixture::provider_protocol(&mut module, exact);
        let output = SingleConeStrongLirOutput::try_new(module, Vec::new()).unwrap();
        let empty = StrongProductionDependencySelectionV2::empty(identity, TARGET).unwrap();
        let section = output
            .build_production_section_v2(
                coordinate.clone(),
                &[],
                EntryProductionSourceV1::Library,
                &empty,
                &[],
            )
            .unwrap();
        let (layouts, value) = exact_layouts(output.foundation(), exact_record);
        let registration = &section.registration_production().types().registrations()[0];
        let descriptor = ExactDescriptorExportV1::replay(
            TARGET,
            &layouts,
            registration,
            &diagnostics,
            output.foundation(),
        )
        .unwrap();
        let descriptors = CanonicalExactDescriptorExportsV1::try_new(
            TARGET,
            output.foundation(),
            vec![descriptor],
        )
        .unwrap();
        let callable_record = ExactCallableAbiExportV1::from_signature(
            TARGET,
            callable,
            scoop_identity::CanonicalScoopAbiFunctionSignature::new(
                ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), exact),
                vec![],
                value
                    .value_handle()
                    .unwrap()
                    .scoop_abi_return(TARGET)
                    .unwrap(),
                (ExactCallableProtocolV1::OrdinaryManaged).gc_effect(),
            )
            .unwrap(),
            output.foundation(),
        )
        .unwrap();
        let callables = CanonicalExactCallableAbiExportsV1::try_new(
            TARGET,
            output.foundation(),
            vec![callable_record],
        )
        .unwrap();
        let table = &output
            .module()
            .meta
            .type_descriptors
            .iter()
            .next()
            .unwrap()
            .1
            .vtable;
        let mut resolver =
            |_: CallableRef| -> Result<Option<StrongTypeDispatchCallableRefV2>, WireError> {
                Ok(None)
            };
        let dispatch_record = ExactDispatchExportV1::replay(
            TARGET,
            table.into(),
            &[],
            output.foundation(),
            &mut resolver,
        )
        .unwrap();
        let dispatch = CanonicalExactDispatchExportsV1::try_new(
            TARGET,
            output.foundation(),
            vec![dispatch_record],
        )
        .unwrap();
        let ordinary =
            CrossConeLirBridgeSectionV1::try_new(output.foundation(), Vec::new(), Vec::new())
                .unwrap();
        let layout_section =
            provider_layout_section(&output, &layouts, &descriptors, &dispatch, &callables);
        let section = section.validate_layout_abi(&layout_section).unwrap();
        Self {
            coordinate,
            identity,
            exact,
            callable,
            callable_body,
            initialization_unit,
            output,
            section,
            ordinary,
            layouts,
            descriptors,
            dispatch,
            callables,
        }
    }

    pub(super) fn initialization_definition(&self) -> StrongInitializationUnitDefinitionRefV2 {
        StrongInitializationUnitDefinitionRefV2::from_registrations(
            self.section.initialization_registrations(),
            self.initialization_unit,
        )
        .unwrap()
    }

    pub(super) fn shape_link_provider(&self) -> ShapeLinkProviderV1<'_> {
        ShapeLinkProviderV1::try_new(ShapeLinkProviderPartsV1 {
            foundation: self.output.foundation(),
            production: &self.section,
            ordinary: &self.ordinary,
            layouts: &self.layouts,
            callables: &self.callables,
            descriptors: &self.descriptors,
            dispatch: &self.dispatch,
        })
        .unwrap()
    }

    pub(super) fn layout_section(&self) -> CrossConeLayoutAbiSectionV1<'_> {
        provider_layout_section(
            &self.output,
            &self.layouts,
            &self.descriptors,
            &self.dispatch,
            &self.callables,
        )
    }
}

fn provider_layout_section(
    output: &SingleConeStrongLirOutput,
    layouts: &CanonicalExactLayoutExportsV1,
    descriptors: &CanonicalExactDescriptorExportsV1,
    dispatch: &CanonicalExactDispatchExportsV1,
    callables: &CanonicalExactCallableAbiExportsV1,
) -> CrossConeLayoutAbiSectionV1<'static> {
    let shape_support = CanonicalParamFreeShapeSupportExportsV1::from_sources(
        &[],
        layouts,
        descriptors,
        output.foundation(),
    )
    .unwrap();
    let exports = LayoutAbiExportConstituentsV1::try_new(
        layouts.clone(),
        descriptors.clone(),
        dispatch.clone(),
        callables.clone(),
        shape_support,
        CrossConeLirBridgeSectionV1::try_new(output.foundation(), Vec::new(), Vec::new()).unwrap(),
    )
    .unwrap();
    CrossConeLayoutAbiSectionV1::try_new(exports, &[], Vec::new(), &[]).unwrap()
}

pub(super) fn consumer_layout_section<'a>(
    coordinate: &ConeCoordinate,
    output: &SingleConeStrongLirOutput,
    registrations: &StrongRegistrationProductionSurfaceV2,
    provider: &'a Provider,
    dependencies: &[&'a LayoutAbiExportConstituentsV1],
    imports: Vec<ExternalShapeLinkImportV1>,
) -> CrossConeLayoutAbiSectionV1<'a> {
    let source = nominal(output.foundation().producer(), "Child");
    let exact_record = exact(&source);
    let exact = exact_record.id();
    let diagnostics = DiagnosticGraph::new(coordinate.clone(), source, exact_record.clone());
    let (layouts, _) = exact_layouts(output.foundation(), exact_record);
    let descriptor = ExactDescriptorExportV1::replay(
        TARGET,
        &layouts,
        &registrations.types().registrations()[0],
        &diagnostics,
        output.foundation(),
    )
    .unwrap();
    let descriptors =
        CanonicalExactDescriptorExportsV1::try_new(TARGET, output.foundation(), vec![descriptor])
            .unwrap();
    let StrongCallableDefinitionOwner::Function(function) = provider.callable else {
        unreachable!()
    };
    let input = ExactDispatchEntryInputV1 {
        position: ExactDispatchPositionV1::from_u32(0),
        slot: PersistentDispatchSlotId::from_key(&DispatchSlotKey::virtual_method(function))
            .unwrap(),
        slot_signature: ExactDispatchSlotSignatureV1::new(
            ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), provider.exact),
            scoop_identity::GcEffect::Managed,
        ),
        implementation: ExactDispatchImplementationV1::DirectStrongTarget {
            target: provider.callable,
            receiver: ExactDispatchReceiverAdaptationV1::Identity,
        },
        abi: DispatchCallableAbiV1::Exact {
            record: &provider.callables.records()[0],
            receiver: CallableAbiReceiverInputV1::NoReceiver,
        },
        slot_receiver_layout: None,
    };
    let expected = StrongTypeDispatchCallableRefV2::DependencyExternal {
        provider: provider.identity,
        body: provider.callable_body,
    };
    let mut resolver =
        |callable: CallableRef| -> Result<Option<StrongTypeDispatchCallableRefV2>, WireError> {
            Ok(match callable {
                CallableRef::External(id)
                    if output.module().meta.external_callables[id].body()
                        == provider.callable_body =>
                {
                    Some(expected)
                }
                _ => None,
            })
        };
    let table = &output
        .module()
        .meta
        .type_descriptors
        .iter()
        .next()
        .unwrap()
        .1
        .vtable;
    let dispatch = ExactDispatchExportV1::replay(
        TARGET,
        table.into(),
        &[input],
        output.foundation(),
        &mut resolver,
    )
    .unwrap();
    assert_eq!(dispatch.owner_exact(), exact);
    let dispatch =
        CanonicalExactDispatchExportsV1::try_new(TARGET, output.foundation(), vec![dispatch])
            .unwrap();
    let callables =
        CanonicalExactCallableAbiExportsV1::try_new(TARGET, output.foundation(), Vec::new())
            .unwrap();
    let shape_support = CanonicalParamFreeShapeSupportExportsV1::from_sources(
        &[],
        &layouts,
        &descriptors,
        output.foundation(),
    )
    .unwrap();
    let exports = LayoutAbiExportConstituentsV1::try_new(
        layouts,
        descriptors,
        dispatch,
        callables,
        shape_support,
        CrossConeLirBridgeSectionV1::try_new(output.foundation(), Vec::new(), Vec::new()).unwrap(),
    )
    .unwrap();
    CrossConeLayoutAbiSectionV1::try_new(exports, dependencies, imports, &[]).unwrap()
}
