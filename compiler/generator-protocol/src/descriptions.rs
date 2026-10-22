//! One-level descriptions retain declaration scopes and actual source members.

use crate::signature::{Result, SignatureProjection, error};
use crate::{Annotations, CallableDeclaration, FieldId, NominalId, QueryJson, Reference};
use scoop_hir as hir;
use serde_json::{Value, json};

mod members;
mod methods;
mod parameters;
mod types;
pub use members::member_description;
pub use types::type_description;

fn nominal_parameters(
    queries: &dyn hir::SemanticQueries,
    nominal: &hir::SemanticNominalDescription,
) -> Result<Vec<Value>> {
    let binders = nominal.declaration_binders();
    let references = binders.signature_binders();
    let projection = SignatureProjection {
        queries,
        context: (&binders).into(),
        source: &nominal.origin.source,
        binders: &references,
        application: None,
    };
    nominal.parameters.iter().zip(&references).enumerate().map(|(index, (parameter, binder))| {
        projection.type_parameter(parameter, binder,
            json!({"kind": "nominal", "owner": NominalId::from(nominal.declaration), "index": index}))
    }).collect()
}

struct MemberProjection<'a> {
    declaration: SignatureProjection<'a>,
    applied: SignatureProjection<'a>,
}

impl MemberProjection<'_> {
    fn type_use(&self, ty: hir::SemanticTypeUse) -> Result<Value> {
        Ok(
            json!({"declarationType": self.declaration.ty(ty.declaration)?,
            "appliedType": self.applied.ty(ty.declaration)?}),
        )
    }

    fn annotations(&self, target: hir::SemanticAnnotationTarget) -> Annotations<'_> {
        Annotations {
            queries: self.declaration.queries,
            target,
        }
    }

    fn body(
        &self,
        declaration: CallableDeclaration,
        callable: hir::SemanticCallable,
        origin: &hir::SemanticSourceLocation,
    ) -> Result<Value> {
        let availability = match self
            .declaration
            .queries
            .body(callable)
            .ok_or_else(|| error(origin.span, "the described callable has no declaration"))?
        {
            hir::SemanticBodyAvailability::Available(_) => "available",
            hir::SemanticBodyAvailability::PendingGeneration => "pendingGeneration",
            hir::SemanticBodyAvailability::NotPublished => "notPublished",
            hir::SemanticBodyAvailability::NoBody => "noBody",
        };
        Ok(json!({"declaration": declaration, "availability": availability}))
    }
}
