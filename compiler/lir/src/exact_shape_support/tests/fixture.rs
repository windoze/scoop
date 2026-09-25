use std::{collections::BTreeMap, sync::Arc};

use scoop_identity::*;

use super::super::*;
use crate::*;

mod layouts;

const TARGET: LirTargetProfile = LirTargetProfile::DARWIN_AARCH64;

pub(super) struct Fixture {
    source: SourceDeclarationKey,
    foundation: OdrFreeLirFoundation,
    layouts: CanonicalExactLayoutExportsV1,
    descriptors: CanonicalExactDescriptorExportsV1,
}

impl Fixture {
    pub(super) fn new(kind: SourceNominalKind, wrong_step_payload: bool) -> Self {
        Self::for_provider(
            ConeCoordinate::reserved_single_file(),
            kind,
            wrong_step_payload,
        )
    }

    pub(super) fn for_provider(
        coordinate: ConeCoordinate,
        kind: SourceNominalKind,
        wrong_step_payload: bool,
    ) -> Self {
        let provider = coordinate.identity().unwrap();
        let source = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                provider,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("Subject").unwrap(),
            kind,
            0,
        );
        let built = layouts::build(&source, wrong_step_payload);
        let foundation = foundation(provider, &built);
        let layouts =
            CanonicalExactLayoutExportsV1::try_new(TARGET, &foundation, built.records).unwrap();
        let graph = Graph::new(source.clone(), &built.shapes, coordinate);
        let descriptors = built
            .shapes
            .iter()
            .map(|shape| descriptor(shape, &foundation, &graph))
            .collect();
        let descriptors =
            CanonicalExactDescriptorExportsV1::try_new(TARGET, &foundation, descriptors).unwrap();
        Self {
            source,
            foundation,
            layouts,
            descriptors,
        }
    }

    pub(super) fn replay(
        &self,
    ) -> Result<ParamFreeShapeSupportExportV1, ParamFreeShapeSupportExportError> {
        ParamFreeShapeSupportExportV1::replay(
            &self.source,
            &self.layouts,
            &self.descriptors,
            &self.foundation,
        )
    }

    pub(super) const fn source(&self) -> &SourceDeclarationKey {
        &self.source
    }

    pub(super) fn source_nominal(&self) -> PersistentTypeId {
        PersistentTypeId::from_source_declaration(&self.source).unwrap()
    }

    pub(super) const fn foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundation
    }

    pub(super) const fn layouts(&self) -> &CanonicalExactLayoutExportsV1 {
        &self.layouts
    }

    pub(super) const fn descriptors(&self) -> &CanonicalExactDescriptorExportsV1 {
        &self.descriptors
    }
}

fn descriptor(
    shape: &layouts::Shape,
    foundation: &OdrFreeLirFoundation,
    graph: &Graph,
) -> ExactDescriptorExportV1 {
    let exact = shape.exact.id();
    let physical = StrongShapeDefinitionRefV1::from_foundation(
        ExternalStrongShapeSubjectV1::TypeDescriptor(exact),
        foundation,
    )
    .unwrap();
    let registration = StrongShapeDefinitionRefV1::from_foundation(
        ExternalStrongShapeSubjectV1::TypeRegistration(exact),
        foundation,
    )
    .unwrap();
    let registration = StrongShapeRegistrationV1::from_artifact(
        exact,
        registration.definition(),
        registration.symbol(),
        DigestNodeId::from_key(&DigestNodeKey::strong_registration(
            registration.definition(),
        ))
        .unwrap(),
    );
    let diagnostic = CanonicalExactTypeDiagnosticName::from_validated_graph(exact, graph).unwrap();
    let vtable = PersistentDispatchTableId::from_key(&DispatchTableKey::vtable(exact)).unwrap();
    ExactDescriptorExportV1::from_parts(crate::exact_descriptor::DescriptorBodyPartsV1 {
        exact: shape.exact.clone(),
        value_layout: Arc::new(shape.value.clone()),
        instance_layout: Arc::new(shape.instance.clone()),
        shape: shape.instance.shape().clone(),
        object_scan: shape.instance.shape().object_scan().clone(),
        ancestry: ExactDescriptorAncestryV1::from_artifact(None, Vec::new()),
        dispatch: ExactDescriptorDispatchV1::from_artifact(vtable, Vec::new()),
        diagnostic_name: diagnostic,
        physical,
        definition: StrongShapeDefinitionV1::from_artifact(
            exact,
            physical.definition(),
            physical.symbol(),
        ),
        registration,
    })
}

fn foundation(provider: ConeIdentity, built: &layouts::LayoutFixture) -> OdrFreeLirFoundation {
    let mut layouts = Vec::new();
    let mut scans = Vec::new();
    let mut plans = Vec::new();
    let mut atoms = Vec::new();
    let mut symbols = Vec::new();
    for source in &built.foundations {
        layouts.extend_from_slice(source.layouts());
        scans.extend_from_slice(source.scans());
        plans.extend_from_slice(source.definition_plans());
        atoms.extend_from_slice(source.definition_atoms());
        symbols.extend_from_slice(source.symbol_requests());
    }
    let mut dispatch = Vec::new();
    for shape in &built.shapes {
        let exact = shape.exact.id();
        let table = CborIdentityRecord::from_key(DispatchTableKey::vtable(exact)).unwrap();
        for subject in [
            ExternalStrongShapeSubjectV1::TypeDescriptor(exact),
            ExternalStrongShapeSubjectV1::TypeRegistration(exact),
            ExternalStrongShapeSubjectV1::DispatchTable(table.id()),
        ] {
            add_subject(provider, subject, &mut plans, &mut atoms, &mut symbols);
        }
        dispatch.push(table);
    }
    let mut canonical = CanonicalLirFoundation::empty();
    canonical
        .set_materialized_exact_types(built.shapes.iter().map(|shape| shape.exact.id()).collect())
        .unwrap();
    canonical.set_layouts(layouts).unwrap();
    canonical.set_scans(scans).unwrap();
    canonical.set_dispatch_tables(dispatch).unwrap();
    canonical.set_definition_plans(plans).unwrap();
    canonical.set_definition_atoms(atoms).unwrap();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(symbols).unwrap());
    OdrFreeLirFoundation::try_new(provider, canonical).unwrap()
}

fn add_subject(
    provider: ConeIdentity,
    subject: ExternalStrongShapeSubjectV1,
    plans: &mut Vec<CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>>,
    atoms: &mut Vec<CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>>,
    symbols: &mut Vec<PersistentSymbolRequest>,
) {
    let (plan, symbol) = subject.expected_definition(provider).unwrap();
    let plan = CborIdentityRecord::from_key(plan).unwrap();
    atoms.push(
        CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
            plan.id(),
            DefinitionAtomRole::Primary,
            DefinitionAtomSubkey::Singleton,
        ))
        .unwrap(),
    );
    plans.push(plan);
    symbols.push(PersistentSymbolRequest::new(symbol, LinkageClass::ConeStrong).unwrap());
}

struct Graph {
    exact: BTreeMap<PersistentExactTypeId, ExactTypeKey>,
    generated: BTreeMap<PersistentTypeId, GeneratedNominalKey>,
    source_nominal: PersistentTypeId,
    source: SourceDeclarationKey,
    coordinate: ConeCoordinate,
}

impl Graph {
    fn new(
        source: SourceDeclarationKey,
        shapes: &[layouts::Shape],
        coordinate: ConeCoordinate,
    ) -> Self {
        let source_nominal = PersistentTypeId::from_source_declaration(&source).unwrap();
        let exact = shapes
            .iter()
            .map(|shape| (shape.exact.id(), shape.exact.key().clone()))
            .collect();
        let source_exact =
            PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(source_nominal)).unwrap();
        let mut generated = BTreeMap::new();
        for key in [
            GeneratedNominalKey::BoxedValue {
                payload: source_exact,
            },
            GeneratedNominalKey::CoroutineStep {
                result: source_exact,
            },
            GeneratedNominalKey::CoroutineSlot {
                value: source_exact,
            },
        ] {
            generated.insert(PersistentTypeId::from_generated_key(&key).unwrap(), key);
        }
        Self {
            exact,
            generated,
            source_nominal,
            source,
            coordinate,
        }
    }
}

impl ExactTypeDiagnosticGraph for Graph {
    fn exact_type_key(&self, id: PersistentExactTypeId) -> Option<&ExactTypeKey> {
        self.exact.get(&id)
    }

    fn source_type_declaration(&self, id: PersistentTypeId) -> Option<&SourceDeclarationKey> {
        (id == self.source_nominal).then_some(&self.source)
    }

    fn generated_nominal_key(&self, id: PersistentTypeId) -> Option<&GeneratedNominalKey> {
        self.generated.get(&id)
    }

    fn source_generic_type_declaration(
        &self,
        _id: PersistentGenericTypeId,
    ) -> Option<&SourceDeclarationKey> {
        None
    }

    fn cone_coordinate(&self, id: ConeIdentity) -> Option<&ConeCoordinate> {
        (id == self.source.origin()).then_some(&self.coordinate)
    }
}
