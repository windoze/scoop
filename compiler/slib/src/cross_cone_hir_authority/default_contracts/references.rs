//! Checks reference roles against the already resolved declaration identities.

use scoop_hir::{
    DefaultClassConstructorIdV1, DefaultConstructorRefV1, DefaultFieldRefV1,
    ExportDefaultReferenceKindV1 as Kind, ExportDefaultReferenceSetV1, SourceNominalId,
};
use scoop_identity::{
    DeclarationScope, DefinitionOwnerAtom, EnumVariantIdentityKey, FieldIdentityKey,
    FieldIdentityView, GeneratedCallableKey, GeneratedNominalKey, IdentityReferenceError,
    PersistentConstructorId, SignatureTypeKey, SourceDeclarationKey, SourceDeclarationKind,
    ValidatedIdentityGraph,
};

use super::{CanonicalCrossConeHirSurfaceAuthority, Error};

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(crate) fn validate_default_reference_targets(
        &self,
        references: &ExportDefaultReferenceSetV1,
    ) -> Result<(), Error> {
        for (index, reference) in references.fields().iter().enumerate() {
            field(self.identities, reference.target())
                .map_err(|error| error.at(Kind::Field, index))?;
        }
        for (index, reference) in references.constructors().iter().enumerate() {
            constructor(self.identities, reference.target())
                .map_err(|error| error.at(Kind::Constructor, index))?;
        }
        for (index, reference) in references.globals().iter().enumerate() {
            let key = self
                .identities
                .canonical_key::<_, SourceDeclarationKey>(*reference.target())
                .map_err(Error::Identity)?;
            if !key.owners().owners().is_empty()
                || matches!(key.scope(), DeclarationScope::LexicalScoped { .. })
            {
                return Err(Error::ReferenceTarget {
                    kind: Kind::Global,
                    index,
                    reason: "global reference requires a top-level property",
                });
            }
        }
        Ok(())
    }
}

fn field(graph: &ValidatedIdentityGraph, target: &DefaultFieldRefV1) -> Result<(), TargetError> {
    let (declaration, owner_type) = match target {
        DefaultFieldRefV1::Struct {
            declaration,
            owner_type,
        }
        | DefaultFieldRefV1::Class {
            declaration,
            owner_type,
        } => (*declaration, owner_type),
        DefaultFieldRefV1::Tuple { .. } => return Ok(()),
    };
    let key = graph.canonical_key::<_, FieldIdentityKey>(declaration)?;
    let (owner, kind) = match (target, key.view()) {
        (DefaultFieldRefV1::Struct { .. }, FieldIdentityView::SourceDeclared { owner, .. }) => {
            (owner, SourceDeclarationKind::Struct)
        }
        (
            DefaultFieldRefV1::Class { .. },
            FieldIdentityView::SourcePropertyBacking { owner, .. }
            | FieldIdentityView::SourcePropertyDelegate { owner, .. },
        ) => (owner, SourceDeclarationKind::Class),
        (DefaultFieldRefV1::Class { .. }, FieldIdentityView::Generated { owner, key })
            if key.object_backing_property().is_some() =>
        {
            let backing = graph.canonical_key::<_, GeneratedNominalKey>(owner)?;
            let GeneratedNominalKey::ObjectBackingClass { object } = backing.as_ref() else {
                return Err(TargetError::Invalid("field is not an object backing field"));
            };
            (
                SourceNominalId::Concrete(*object),
                SourceDeclarationKind::Object,
            )
        }
        _ => {
            return Err(TargetError::Invalid(
                "field declaration has a different storage role",
            ));
        }
    };
    check_owner(graph, owner, owner_type, kind)
}

fn constructor(
    graph: &ValidatedIdentityGraph,
    target: &DefaultConstructorRefV1,
) -> Result<(), TargetError> {
    let (owner, kind) = match target {
        DefaultConstructorRefV1::Struct { declaration, .. } => (
            constructor_owner(graph, *declaration)?,
            SourceDeclarationKind::Struct,
        ),
        DefaultConstructorRefV1::Class { declaration, .. } => {
            let declaration = match declaration {
                DefaultClassConstructorIdV1::Source(id) => *id,
                DefaultClassConstructorIdV1::Generated(id) => {
                    let key = graph.canonical_key::<_, GeneratedCallableKey>(*id)?;
                    let GeneratedCallableKey::ZeroArgumentConstructorAdapter { constructor } =
                        key.as_ref()
                    else {
                        return Err(TargetError::Invalid(
                            "generated callable is not a constructor adapter",
                        ));
                    };
                    *constructor
                }
            };
            (
                constructor_owner(graph, declaration)?,
                SourceDeclarationKind::Class,
            )
        }
        DefaultConstructorRefV1::Variant { declaration, .. } => {
            let key = graph.canonical_key::<_, EnumVariantIdentityKey>(*declaration)?;
            let owner = key
                .source_owner()
                .ok_or(TargetError::Invalid("variant has no source enum owner"))?;
            (owner, SourceDeclarationKind::Enum)
        }
    };
    check_owner(graph, owner, target.owner_type(), kind)
}

fn constructor_owner(
    graph: &ValidatedIdentityGraph,
    declaration: PersistentConstructorId,
) -> Result<SourceNominalId, TargetError> {
    let key = graph.canonical_key::<_, SourceDeclarationKey>(declaration)?;
    match key.owners().owners().last() {
        Some(DefinitionOwnerAtom::Type(id)) => Ok(SourceNominalId::Concrete(*id)),
        Some(DefinitionOwnerAtom::GenericType(id)) => Ok(SourceNominalId::GenericTemplate(*id)),
        _ => Err(TargetError::Invalid("constructor has no nominal owner")),
    }
}

fn check_owner(
    graph: &ValidatedIdentityGraph,
    owner: SourceNominalId,
    owner_type: &SignatureTypeKey,
    kind: SourceDeclarationKind,
) -> Result<(), TargetError> {
    let actual = match owner_type {
        SignatureTypeKey::Nominal(id) => SourceNominalId::Concrete(*id),
        SignatureTypeKey::NominalApplication { origin, .. } => {
            SourceNominalId::GenericTemplate(*origin)
        }
        _ => {
            return Err(TargetError::Invalid(
                "reference owner is not a nominal type",
            ));
        }
    };
    if actual != owner {
        return Err(TargetError::Invalid(
            "reference owner does not match its declaration",
        ));
    }
    let key = match owner {
        SourceNominalId::Concrete(id) => graph.canonical_key::<_, SourceDeclarationKey>(id)?,
        SourceNominalId::GenericTemplate(id) => {
            graph.canonical_key::<_, SourceDeclarationKey>(id)?
        }
    };
    if key.declaration_kind() != kind {
        return Err(TargetError::Invalid(
            "reference requires a different nominal kind",
        ));
    }
    Ok(())
}

enum TargetError {
    Identity(IdentityReferenceError),
    Invalid(&'static str),
}

impl From<IdentityReferenceError> for TargetError {
    fn from(error: IdentityReferenceError) -> Self {
        Self::Identity(error)
    }
}

impl TargetError {
    fn at(self, kind: Kind, index: usize) -> Error {
        match self {
            Self::Identity(error) => Error::Identity(error),
            Self::Invalid(reason) => Error::ReferenceTarget {
                kind,
                index,
                reason,
            },
        }
    }
}
