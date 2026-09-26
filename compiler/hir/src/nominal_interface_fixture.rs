//! Public-only unit-test declarations; complete support fixtures use real HIR.

use crate as hir;

use scoop_identity::{
    CallableTemplateOrigin, PersistentConstructorId, PersistentExportBindingId, PropertyOwner,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn public_record(
    declaration: hir::SourceNominalId,
    kind: hir::PublicNominalKindV1,
    binders: hir::CanonicalBinderListV1,
    supertypes: hir::CanonicalSignatureTypesV1,
    constructors: hir::CanonicalPersistentIdsV1<PersistentConstructorId>,
    members: hir::CanonicalPublicMemberRefsV1,
    nested: hir::CanonicalPersistentIdsV1<PersistentExportBindingId>,
    shape: hir::NominalSourceShapeV1,
) -> Result<hir::NominalInterfaceRecordV1, hir::NominalInterfaceRecordBuildError> {
    let declared = members
        .members()
        .iter()
        .filter_map(|member| match *member {
            hir::PublicMemberRefV1::Callable(CallableTemplateOrigin::Function(id)) => {
                Some(hir::NestedSourceMemberRefV1::Function(id))
            }
            hir::PublicMemberRefV1::Callable(CallableTemplateOrigin::GenericFunction(id)) => {
                Some(hir::NestedSourceMemberRefV1::GenericFunction(id))
            }
            hir::PublicMemberRefV1::Property(PropertyOwner::Property(id)) => {
                Some(hir::NestedSourceMemberRefV1::Property(id))
            }
            _ => None,
        })
        .collect();
    let modality = if kind == hir::PublicNominalKindV1::Interface {
        hir::NominalInheritanceModalityV1::Interface
    } else {
        hir::NominalInheritanceModalityV1::Final
    };
    let details = hir::NominalDeclarationDetailsV1::new(
        modality,
        hir::DeclaredVisibilityV1::Public,
        constructors.clone(),
        hir::CanonicalNestedMemberRefsV1::try_new(declared).unwrap(),
        hir::CanonicalNestedNominalRefsV1::default(),
        if kind == hir::PublicNominalKindV1::Interface {
            hir::NominalDispatchOrderV1::Interface {
                parents: supertypes.values().to_vec(),
                members: Vec::new(),
            }
        } else {
            hir::NominalDispatchOrderV1::empty(kind)
        },
        hir::CanonicalNominalDispatchSelectionsV1::empty(),
        if kind == hir::PublicNominalKindV1::Struct {
            constructors.values().first().copied()
        } else {
            None
        },
    );
    hir::NominalInterfaceRecordV1::try_new(
        declaration,
        kind,
        binders,
        supertypes,
        constructors,
        members,
        nested,
        shape,
        details,
    )
}
