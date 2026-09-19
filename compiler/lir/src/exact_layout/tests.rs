use scoop_identity::*;
use scoop_wire::{BudgetMeter, DecodeLimits, encode};

use super::*;
use crate::*;

mod c_layout;
mod enums;
mod instances;
mod value_descriptors;
mod values;

const TARGET: LirTargetProfile = LirTargetProfile::DARWIN_AARCH64;

pub(crate) fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

pub(crate) fn source(name: &str, kind: SourceNominalKind, parameters: u32) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        kind,
        parameters,
    )
}

pub(crate) fn exact(
    source: &SourceDeclarationKey,
) -> CborIdentityRecord<PersistentExactTypeId, ExactTypeKey> {
    CborIdentityRecord::from_key(ExactTypeKey::Nominal(
        PersistentTypeId::from_source_declaration(source).unwrap(),
    ))
    .unwrap()
}

pub(crate) struct Bound {
    pub(crate) identity: ExactLayoutIdentityV1,
    pub(crate) foundation: OdrFreeLirFoundation,
}

impl Bound {
    pub(crate) fn new(
        exact: CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
        role: RepresentationRole,
        scan_role: ScanRole,
    ) -> Self {
        let layout =
            CborIdentityRecord::from_key(LayoutKey::new(exact.id(), TARGET.wire_id(), role))
                .unwrap();
        let scan = CborIdentityRecord::from_key(ScanKey::new(layout.id(), scan_role)).unwrap();
        let mut foundation = CanonicalLirFoundation::empty();
        let mut plans = Vec::new();
        let mut atoms = Vec::new();
        let mut symbols = Vec::new();
        for subject in [
            ExternalStrongShapeSubjectV1::Layout(layout.id()),
            ExternalStrongShapeSubjectV1::Scan(scan.id()),
        ] {
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
            plans.push(plan);
            symbols.push(PersistentSymbolRequest::new(symbol, LinkageClass::ConeStrong).unwrap());
        }
        foundation.set_layouts(vec![layout]).unwrap();
        foundation.set_scans(vec![scan]).unwrap();
        foundation.set_definition_plans(plans).unwrap();
        foundation.set_definition_atoms(atoms).unwrap();
        foundation.set_symbol_requests(PersistentSymbolRequestTable::new(symbols).unwrap());
        let foundation =
            OdrFreeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, foundation).unwrap();
        let identity =
            ExactLayoutIdentityV1::from_foundation(TARGET, exact, role, &foundation, &mut meter())
                .unwrap();
        Self {
            identity,
            foundation,
        }
    }
    pub(crate) fn value(exact: CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>) -> Self {
        Self::new(
            exact,
            RepresentationRole::ManagedValue,
            ScanRole::InlineValue,
        )
    }
    pub(crate) fn instance(exact: CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>) -> Self {
        Self::new(
            exact,
            RepresentationRole::ManagedObject,
            ScanRole::ManagedObject,
        )
    }
}

pub(crate) fn integer(name: &str, kind: IntegerKind) -> ExactValueLayoutV1 {
    let bound = Bound::value(exact(&source(name, SourceNominalKind::Struct, 0)));
    ExactValueLayoutV1::scalar(
        bound.identity,
        ScalarRepresentationKindV1::Integer(kind),
        &bound.foundation,
        &mut meter(),
    )
    .unwrap()
}

pub(crate) fn unit() -> ExactValueLayoutV1 {
    let bound = Bound::value(exact(&CoreBuiltinNominal::Unit.declaration_key()));
    ExactValueLayoutV1::unit(bound.identity, &bound.foundation, &mut meter()).unwrap()
}

pub(crate) fn managed() -> ExactValueLayoutV1 {
    let bound = Bound::value(exact(&source("Object", SourceNominalKind::Class, 0)));
    ExactValueLayoutV1::qualified_pointer(
        bound.identity,
        NichePointerKind::Managed,
        &bound.foundation,
        &mut meter(),
    )
    .unwrap()
}

pub(crate) fn field(
    owner: &SourceDeclarationKey,
    name: &str,
) -> CborIdentityRecord<PersistentFieldId, FieldIdentityKey> {
    CborIdentityRecord::from_key(
        FieldIdentityKey::source_declared(owner, CanonicalIdentifier::new(name).unwrap()).unwrap(),
    )
    .unwrap()
}

fn scan(value: &ExactValueLayoutV1) -> &RefScan {
    super::replay::storage_scan(value.value().storage())
}

fn assert_wire_roundtrip(value: impl Into<ExactLayoutExportV1>) {
    let expected = value.into();
    let bytes = encode(&expected).unwrap();
    let raw =
        scoop_wire::decode_canonical::<DecodedExactLayoutExportV1>(&bytes, DecodeLimits::default())
            .unwrap();
    assert_eq!(encode(&raw).unwrap(), bytes);
    assert_eq!(
        raw.validate_against(&expected, &mut meter()).unwrap(),
        expected
    );
}
