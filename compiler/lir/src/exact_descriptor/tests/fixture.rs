use scoop_identity::*;
use scoop_wire::{BudgetMeter, DecodeLimits};

use super::super::*;
use crate::*;

const TARGET: LirTargetProfile = LirTargetProfile::DARWIN_AARCH64;

pub(super) struct Fixture {
    exact: CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
    foreign_exact: PersistentExactTypeId,
    value: ExactValueLayoutV1,
    instance: ExactInstanceLayoutV1,
    layouts: CanonicalExactLayoutExportsV1,
    foundation: OdrFreeLirFoundation,
    vtable: PersistentDispatchTableId,
    registration: StrongShapeRegistrationV1<PersistentExactTypeId>,
    diagnostics: Graph,
    name: String,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let declaration = source("Empty");
        let exact_record = exact(&declaration);
        let foreign_exact = exact(&source("Foreign")).id();
        let value_bound = crate::exact_layout::tests::Bound::value(exact_record.clone());
        let value = ExactValueLayoutV1::ordinary_struct(
            value_bound.identity.clone(),
            false,
            &[],
            &value_bound.foundation,
            &mut meter(),
        )
        .unwrap();
        let instance_bound = crate::exact_layout::tests::Bound::instance(exact_record.clone());
        let instance = ExactInstanceLayoutV1::boxed_payload(
            instance_bound.identity.clone(),
            &value,
            &instance_bound.foundation,
            &mut meter(),
        )
        .unwrap();
        let vtable_record =
            CborIdentityRecord::from_key(DispatchTableKey::vtable(exact_record.id())).unwrap();
        let foundation = foundation(
            [&value_bound.foundation, &instance_bound.foundation],
            exact_record.id(),
            vtable_record.clone(),
        );
        let layouts = CanonicalExactLayoutExportsV1::try_new(
            TARGET,
            &foundation,
            vec![value.clone().into(), instance.clone().into()],
            &mut meter(),
        )
        .unwrap();
        let physical = StrongShapeDefinitionRefV1::from_foundation(
            ExternalStrongShapeSubjectV1::TypeRegistration(exact_record.id()),
            &foundation,
            &mut meter(),
        )
        .unwrap();
        let fingerprint_node =
            DigestNodeId::from_key(&DigestNodeKey::strong_registration(physical.definition()))
                .unwrap();
        let registration = StrongShapeRegistrationV1::from_artifact(
            exact_record.id(),
            physical.definition(),
            physical.symbol(),
            fingerprint_node,
        );
        let diagnostics = Graph::new(declaration, exact_record.key().clone());
        let name =
            CanonicalExactTypeDiagnosticName::from_validated_graph(exact_record.id(), &diagnostics)
                .unwrap()
                .as_str()
                .to_owned();
        Self {
            exact: exact_record,
            foreign_exact,
            value,
            instance,
            layouts,
            foundation,
            vtable: vtable_record.id(),
            registration,
            diagnostics,
            name,
        }
    }

    pub(super) fn exact(&self) -> PersistentExactTypeId {
        self.exact.id()
    }

    pub(super) fn foreign_exact(&self) -> PersistentExactTypeId {
        self.foreign_exact
    }

    pub(super) fn value_layout(&self) -> PersistentLayoutId {
        self.value.identity().layout()
    }

    pub(super) fn instance_layout(&self) -> PersistentLayoutId {
        self.instance.identity().layout()
    }

    pub(super) fn instance(&self) -> &ExactInstanceLayoutV1 {
        &self.instance
    }

    pub(super) fn foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundation
    }

    pub(super) fn vtable(&self) -> PersistentDispatchTableId {
        self.vtable
    }

    pub(super) fn registration(&self) -> StrongShapeRegistrationV1<PersistentExactTypeId> {
        self.registration
    }

    pub(super) fn name(&self) -> &str {
        &self.name
    }

    pub(super) fn meter(&self) -> BudgetMeter {
        meter()
    }

    pub(super) fn semantic(&self) -> StrongTypeDescriptorSemanticPlanV2 {
        self.semantic_parts(
            self.name.clone(),
            self.instance.shape().clone(),
            self.vtable,
        )
    }

    pub(super) fn semantic_with_name(
        &self,
        name: impl Into<String>,
    ) -> StrongTypeDescriptorSemanticPlanV2 {
        self.semantic_parts(name.into(), self.instance.shape().clone(), self.vtable)
    }

    pub(super) fn semantic_with_shape(
        &self,
        shape: TypeInstanceShapeV1,
    ) -> StrongTypeDescriptorSemanticPlanV2 {
        self.semantic_parts(self.name.clone(), shape, self.vtable)
    }

    pub(super) fn semantic_with_vtable(
        &self,
        vtable: PersistentDispatchTableId,
    ) -> StrongTypeDescriptorSemanticPlanV2 {
        self.semantic_parts(self.name.clone(), self.instance.shape().clone(), vtable)
    }

    pub(super) fn replay(
        &self,
        semantic: StrongTypeDescriptorSemanticPlanV2,
    ) -> Result<ExactDescriptorExportV1, ExactDescriptorError> {
        super::super::replay::replay_test_parts(
            TARGET,
            &self.layouts,
            &semantic,
            self.registration,
            &self.diagnostics,
            &self.foundation,
            &mut meter(),
        )
    }

    fn semantic_parts(
        &self,
        name: String,
        shape: TypeInstanceShapeV1,
        vtable: PersistentDispatchTableId,
    ) -> StrongTypeDescriptorSemanticPlanV2 {
        StrongTypeDescriptorSemanticPlanV2::from_artifact(
            self.exact(),
            name,
            self.instance_layout(),
            ExactLayoutExportV1::from(self.instance.clone()).scan(),
            shape,
            TypeDescriptorInlineScanV1::Null,
            None,
            StrongTypeVtableSemanticPlanV2::from_artifact(vtable, Vec::new()),
            Vec::new(),
        )
    }
}

fn source(name: &str) -> SourceDeclarationKey {
    crate::exact_layout::tests::source(name, SourceNominalKind::Struct, 0)
}

fn exact(source: &SourceDeclarationKey) -> CborIdentityRecord<PersistentExactTypeId, ExactTypeKey> {
    crate::exact_layout::tests::exact(source)
}

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

fn foundation(
    sources: [&OdrFreeLirFoundation; 2],
    exact: PersistentExactTypeId,
    vtable: CborIdentityRecord<PersistentDispatchTableId, DispatchTableKey>,
) -> OdrFreeLirFoundation {
    let mut layouts = Vec::new();
    let mut scans = Vec::new();
    let mut plans = Vec::new();
    let mut atoms = Vec::new();
    let mut symbols = Vec::new();
    for source in sources {
        layouts.extend_from_slice(source.layouts());
        scans.extend_from_slice(source.scans());
        plans.extend_from_slice(source.definition_plans());
        atoms.extend_from_slice(source.definition_atoms());
        symbols.extend_from_slice(source.symbol_requests());
    }
    let descriptor = add_subject(
        ExternalStrongShapeSubjectV1::TypeDescriptor(exact),
        &mut plans,
        &mut atoms,
        &mut symbols,
    );
    atoms.push(
        CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
            descriptor,
            DefinitionAtomRole::AddressTakenConstant,
            DefinitionAtomSubkey::ExactType(exact),
        ))
        .unwrap(),
    );
    add_subject(
        ExternalStrongShapeSubjectV1::TypeRegistration(exact),
        &mut plans,
        &mut atoms,
        &mut symbols,
    );
    add_subject(
        ExternalStrongShapeSubjectV1::DispatchTable(vtable.id()),
        &mut plans,
        &mut atoms,
        &mut symbols,
    );
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_layouts(layouts).unwrap();
    canonical.set_scans(scans).unwrap();
    canonical.set_dispatch_tables(vec![vtable]).unwrap();
    canonical.set_definition_plans(plans).unwrap();
    canonical.set_definition_atoms(atoms).unwrap();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(symbols).unwrap());
    OdrFreeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap()
}

fn add_subject(
    subject: ExternalStrongShapeSubjectV1,
    plans: &mut Vec<CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>>,
    atoms: &mut Vec<CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>>,
    symbols: &mut Vec<PersistentSymbolRequest>,
) -> ObjectDefinitionPlanId {
    let (plan, symbol) = subject
        .expected_definition(ConeIdentity::SINGLE_FILE)
        .unwrap();
    let plan = CborIdentityRecord::from_key(plan).unwrap();
    atoms.push(
        CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
            plan.id(),
            DefinitionAtomRole::Primary,
            DefinitionAtomSubkey::Singleton,
        ))
        .unwrap(),
    );
    symbols.push(PersistentSymbolRequest::new(symbol, LinkageClass::ConeStrong).unwrap());
    let id = plan.id();
    plans.push(plan);
    id
}

struct Graph {
    exact: PersistentExactTypeId,
    exact_key: ExactTypeKey,
    nominal: PersistentTypeId,
    source: SourceDeclarationKey,
    coordinate: ConeCoordinate,
}

impl Graph {
    fn new(source: SourceDeclarationKey, exact_key: ExactTypeKey) -> Self {
        let nominal = PersistentTypeId::from_source_declaration(&source).unwrap();
        let exact = PersistentExactTypeId::from_key(&exact_key).unwrap();
        Self {
            exact,
            exact_key,
            nominal,
            source,
            coordinate: ConeCoordinate::reserved_single_file(),
        }
    }
}

impl ExactTypeDiagnosticGraph for Graph {
    fn exact_type_key(&self, id: PersistentExactTypeId) -> Option<&ExactTypeKey> {
        (id == self.exact).then_some(&self.exact_key)
    }

    fn source_type_declaration(&self, id: PersistentTypeId) -> Option<&SourceDeclarationKey> {
        (id == self.nominal).then_some(&self.source)
    }

    fn generated_nominal_key(&self, _id: PersistentTypeId) -> Option<&GeneratedNominalKey> {
        None
    }

    fn source_generic_type_declaration(
        &self,
        _id: PersistentGenericTypeId,
    ) -> Option<&SourceDeclarationKey> {
        None
    }

    fn cone_coordinate(&self, id: ConeIdentity) -> Option<&ConeCoordinate> {
        (id == ConeIdentity::SINGLE_FILE).then_some(&self.coordinate)
    }
}
