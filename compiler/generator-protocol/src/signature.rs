//! Complete generator method signatures use the frontend's declaration scopes.

use crate::{CallableDeclaration, MethodId, NominalId, QueryJson};
use scoop_hir as hir;
use scoop_identity::{SignatureTypeKey, SourceIdentity};
use scoop_source::Diagnostic;
use serde_json::{Value, json};

mod bounds;
mod effects;
mod parameters;

pub(crate) type Result<T> = std::result::Result<T, Diagnostic>;

pub fn pending_method_signature(
    queries: &mut dyn hir::SemanticQueries,
    method: &hir::PendingMethod,
) -> Result<Value> {
    let signature = queries.signature(method.function);
    let declaration = MethodId::from(&queries.method_identity(method.function)?);
    let parameters = queries.function_parameters(method.function)?;
    let binders = signature.declaration_binders();
    let receiver = signature.receiver.ok_or_else(|| {
        error(
            signature.origin.span,
            "the pending method has no nominal receiver",
        )
    })?;
    let owner = queries.describe_type((&binders).into(), receiver)?;
    let hir::SemanticTypeStructure::Nominal(owner) = owner.structure else {
        return Err(error(
            signature.origin.span,
            "the pending receiver is not nominal",
        ));
    };
    let owner = NominalId::from(owner.declaration);
    let binder_references = binders.signature_binders();
    let projection = SignatureProjection {
        queries,
        context: (&binders).into(),
        source: &signature.origin.source,
        binders: &binder_references,
        application: None,
    };
    let parameters = parameters
        .iter()
        .map(|parameter| projection.parameter(parameter))
        .collect::<Result<Vec<_>>>()?;
    let mut type_parameters = Vec::with_capacity(signature.type_parameters.len());
    for (index, (parameter, binder)) in signature
        .type_parameters
        .iter()
        .zip(&binder_references)
        .enumerate()
    {
        let reference = if index < signature.owner_parameter_count {
            json!({ "kind": "nominal", "owner": owner, "index": index })
        } else {
            json!({ "kind": "callable", "owner": declaration, "index": index - signature.owner_parameter_count })
        };
        type_parameters.push(projection.type_parameter(parameter, binder, reference)?);
    }
    let context_parameters = signature
        .context_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            Ok(json!({
                "declaration": { "owner": declaration, "index": index },
                "label": QueryJson(&parameter.label),
                "valueType": projection.ty(parameter.ty)?,
                "origin": projection.origin(parameter.span),
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({
        "method": declaration, "owner": owner, "name": signature.name,
        "receiver": projection.ty(receiver)?, "binders": type_parameters,
        "contextParameters": context_parameters, "parameters": parameters,
        "result": projection.ty(signature.result)?,
        "effects": projection.effects(signature.is_suspend, &signature.modifiers, &signature.attributes,
            &signature.release_callability, signature.origin.span)?, "origin": QueryJson(&signature.origin),
    }))
}

pub(crate) struct SignatureProjection<'a> {
    pub queries: &'a dyn hir::SemanticQueries,
    pub context: hir::SemanticTypeContext<'a>,
    pub source: &'a SourceIdentity,
    pub binders: &'a [hir::HirSignatureBinder],
    pub application: Option<(&'a hir::CanonicalBinderUseListV1, u32)>,
}

impl SignatureProjection<'_> {
    pub(crate) fn signature_type(&self, ty: hir::TypeId) -> Result<SignatureTypeKey> {
        let signature = self.queries.type_signature(self.context, ty)?;
        if let Some((arguments, introduced)) = self.application {
            let provider = hir::DefaultTemplateProviderShapeV1::try_new(arguments.len_u32(), 0)
                .map_err(|cause| error(hir::Span::new(0, 0), cause))?;
            arguments
                .substitute_provider_type_in_scope(provider, &signature, introduced)
                .map_err(|cause| error(hir::Span::new(0, 0), cause))
        } else {
            Ok(signature)
        }
    }

    pub(crate) fn ty(&self, ty: hir::TypeId) -> Result<Value> {
        Ok(json!(QueryJson(&self.signature_type(ty)?)))
    }

    fn origin(&self, span: hir::Span) -> Value {
        json!({ "source": QueryJson(self.source), "span": QueryJson(&span) })
    }
}

pub(crate) fn error(span: hir::Span, cause: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::at(
        span,
        format!("cannot project the semantic description: {cause}"),
    )
}
