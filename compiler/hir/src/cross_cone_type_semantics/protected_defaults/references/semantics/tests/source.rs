use super::*;
use crate::cross_cone_type_semantics::inheritance::tests::support::site;

pub(super) fn owner(
    fixture: &mut Fixture,
    concrete: SourceNominalId,
    generic: bool,
) -> SourceNominalId {
    if !generic {
        return concrete;
    }
    let key = SourceDeclarationKey::nominal(
        site(&[]),
        CanonicalIdentifier::new("GenericOwner").unwrap(),
        SourceNominalKind::Class,
        1,
    );
    let owner = SourceNominalId::from_source_declaration(&key).unwrap();
    let origin = fixture.graph.origins[&concrete].clone();
    fixture.graph.keys.insert(owner, key);
    fixture.graph.access.insert(
        owner,
        DeclarationAccessSourceV1::try_new(DeclaredVisibilityV1::Public, vec![], origin.clone())
            .unwrap(),
    );
    fixture.graph.origins.insert(owner, origin);
    owner
}
pub(super) fn callable(
    fixture: &mut Fixture,
    owner: SourceNominalId,
    target: SourceNominalId,
) -> Record {
    let SourceNominalId::Concrete(target) = target else {
        panic!("concrete reference target");
    };
    let ty = SignatureTypeKey::Nominal(target);
    let key = SourceDeclarationKey::function(
        site(&[owner]),
        CanonicalIdentifier::new("defaultOwner").unwrap(),
        0,
        None,
        vec![ty.clone()],
    );
    let declaration = CallableTemplateOrigin::Function(
        PersistentFunctionId::from_source_declaration(&key).unwrap(),
    );
    fixture.declarations.insert(declaration, key);
    let effects = fixture
        .payload(
            fixture.unit,
            declaration,
            vec![],
            SignatureTypeKey::Nominal(nominal(fixture.unit)),
        )
        .effects();
    let record = NominalSupportCallableInterfaceV1::try_new(
        declaration,
        DeclarationAccessSourceV1::try_new(
            DeclaredVisibilityV1::Public,
            vec![owner],
            fixture.graph.origins[&owner].clone(),
        )
        .unwrap(),
        NominalSourceCallablePayloadV1::try_new(
            declaration,
            owner,
            CanonicalBinderListV1::try_new(vec![]).unwrap(),
            CanonicalSourceParameterShapesV1::try_new(vec![SourceParameterShapeV1::new(
                CanonicalIdentifier::new("value").unwrap(),
                ty.clone(),
            )])
            .unwrap(),
            ty,
            effects,
            CallableModalityV1::Final,
            CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    if matches!(owner, SourceNominalId::GenericTemplate(_)) {
        Record::Protected(
            ProtectedCallableInterfaceV1::try_new(
                declaration,
                DeclarationAccessSourceV1::try_new(
                    DeclaredVisibilityV1::Protected,
                    vec![owner],
                    fixture.graph.origins[&owner].clone(),
                )
                .unwrap(),
                ProtectedCallablePayloadV1::try_new(
                    declaration,
                    owner,
                    record.payload().type_parameters().clone(),
                    record.payload().parameters().clone(),
                    record.payload().result().clone(),
                    record.payload().effects(),
                    record.payload().modality(),
                    record.payload().slot_relations().clone(),
                )
                .unwrap(),
            )
            .unwrap(),
        )
    } else {
        Record::Support(record)
    }
}

pub(super) enum Record {
    Protected(ProtectedCallableInterfaceV1),
    Support(NominalSupportCallableInterfaceV1),
}
impl Record {
    pub fn declaration(&self) -> CallableTemplateOrigin {
        match self {
            Self::Protected(record) => record.declaration(),
            Self::Support(record) => record.declaration(),
        }
    }
    pub fn check<'a>(
        &'a self,
        graph: &CheckedNominalInheritanceGraphV1<'_>,
        fixture: &'a mut Fixture,
    ) -> ProtectedDefaultOwnerSourceV1<'a> {
        match self {
            Self::Protected(record) => ProtectedDefaultOwnerSourceV1::Protected(
                record.validate_source(graph, fixture).unwrap(),
            ),
            Self::Support(record) => ProtectedDefaultOwnerSourceV1::NominalSupport(
                record.validate_source(graph, fixture).unwrap(),
            ),
        }
    }
}
