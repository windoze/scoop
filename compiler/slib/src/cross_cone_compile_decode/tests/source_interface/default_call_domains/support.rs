//! Isolates domain replay over actual typed foundation records. Publication
//! fixtures independently exercise the source, parameter, body and receiver joins.
use super::*;
use crate::cross_cone_hir_authority::CanonicalCrossConeHirSurfaceAuthority;
use scoop_hir::*;
use scoop_identity::{
    CallableOwner, CoreBuiltinNominal, DefinitionOrigin, DefinitionOwnerAtom, DispatchSlotKey,
    PersistentDispatchSlotId, PersistentGenericTypeId,
};
use scoop_wire::{BudgetMeter, WirePath};
use std::collections::BTreeMap;

mod artifact;
mod declarations;
mod functions;
mod templates;
pub(super) use declarations::{applied, callable};

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Class,
    Interface,
    Struct,
}
pub(super) enum Dispatch {
    Direct,
    Virtual,
    Interface,
    Inherited(CallableTemplateOrigin),
}

#[derive(Clone)]
struct Nominal {
    key: SourceDeclarationKey,
    id: SourceNominalId,
    kind: Kind,
    visibility: Visibility,
    arity: u32,
    parents: Vec<SignatureTypeKey>,
    members: Vec<NestedSourceMemberRefV1>,
}

#[derive(Clone)]
pub(super) struct Builder {
    pub cone: ConeRecord,
    pub token: SignatureTypeKey,
    source: SourceIdentity,
    context: SourceContextKey,
    nominals: Vec<Nominal>,
    functions: Vec<CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>>,
    slots: BTreeMap<
        PersistentDispatchSlotId,
        CborIdentityRecord<PersistentDispatchSlotId, DispatchSlotKey>,
    >,
    callables: Vec<CallableDeclarationRecordV1>,
    public: BTreeMap<CallableTemplateOrigin, PublicLookupAccessV1>,
}

#[derive(Clone)]
pub(super) struct Fixture {
    cone: ConeRecord,
    foundation: CanonicalHirFoundation,
    pub interface: CrossConeHirInterfaceSectionV1,
}

impl Builder {
    pub fn new() -> Self {
        let cone = cone();
        let source = SourceIdentity::new(
            cone.identity(),
            NormalizedSourcePath::new("src/Domains.scoop").unwrap(),
        )
        .unwrap();
        let key = SourceDeclarationKey::nominal(
            site(cone.identity(), vec![]),
            CanonicalIdentifier::new("Token").unwrap(),
            SourceNominalKind::Struct,
            0,
        );
        let id =
            SourceNominalId::Concrete(PersistentTypeId::from_source_declaration(&key).unwrap());
        let SignatureTypeKey::Nominal(token) = applied(id, vec![]) else {
            panic!("concrete token");
        };
        Self {
            cone,
            token: SignatureTypeKey::Nominal(token),
            context: SourceContextKey::File {
                source: source.clone(),
            },
            source,
            nominals: vec![Nominal {
                key,
                id,
                kind: Kind::Struct,
                visibility: Visibility::Public,
                arity: 0,
                parents: vec![],
                members: vec![],
            }],
            functions: vec![],
            slots: BTreeMap::new(),
            callables: vec![],
            public: BTreeMap::new(),
        }
    }
}

fn site(cone: ConeIdentity, owners: Vec<DefinitionOwnerAtom>) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        cone,
        PackagePath::root(),
        DefinitionOwnerChain::from_outer_to_inner(owners),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
fn owner_atom(id: SourceNominalId) -> DefinitionOwnerAtom {
    match id {
        SourceNominalId::Concrete(id) => DefinitionOwnerAtom::Type(id),
        SourceNominalId::GenericTemplate(id) => DefinitionOwnerAtom::GenericType(id),
    }
}
pub(super) fn region(constraints: Vec<Constraint>) -> Domain {
    Domain::from_constraints(
        constraints,
        &mut BudgetMeter::new(DecodeLimits::default()),
        &WirePath::root(),
    )
    .unwrap()
}
