use super::*;
use scoop_hir::{
    DefaultBodyReferenceOccurrenceV1, DefaultBodyReferenceTargetV1 as Target,
    DefaultBodyReferenceVisitorV1, DefaultClassConstructorIdV1, DefaultConstructorRefV1,
    DefaultConstructorReferenceTargetViewV1 as Constructor, DefaultFieldRefV1,
    DefaultFieldReferenceTargetViewV1 as Field, DefaultSourceNestedCallablesV1,
    ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1,
};

mod callables;
type Nested<'a> = DefaultSourceNestedCallablesV1<'a, ExportDefaultTemplateKeyV1>;

impl Closure<'_, '_> {
    pub(super) fn default(&mut self, template: &ExportDefaultTemplateV1) -> Result<(), Error> {
        self.callable(template.definition_root().declaration())?;
        for ty in template.type_parameters().arguments() {
            self.signature(ty)?;
        }
        let path = WirePath::root().field(7);
        let nested = template.index_nested_callables(&path)?;
        let mut occurrences = Occurrences(Vec::new());
        template.body().visit_direct_references(
            template.locals(),
            template.definition_origin(),
            &mut occurrences,
            &path,
        )?;
        for occurrence in occurrences.0 {
            match occurrence.target {
                Target::Callable(target) => self.default_callable(target, &nested, occurrence)?,
                Target::Type(ty) => self.signature(ty)?,
                Target::Global(id) => self.property(PropertyOwner::Property(id))?,
                Target::Singleton(id) => {
                    let key = self
                        .world
                        .identities
                        .canonical_key::<PersistentObjectValueId, SourceDeclarationKey>(id)
                        .map_err(CrossConeHirNominalAuthorityError::Identity)?;
                    let owner = SourceNominalId::from_source_declaration(&key)
                        .map_err(scoop_hir::DefaultSourceTargetSubjectError::Identity)?;
                    self.nominal(owner)?;
                }
                Target::Constructor(constructor) => self.default_constructor(constructor)?,
                Target::Field(field) => match field {
                    Field::Struct { owner_type, .. }
                    | Field::Field(
                        DefaultFieldRefV1::Struct { owner_type, .. }
                        | DefaultFieldRefV1::Class { owner_type, .. },
                    ) => self.signature(owner_type)?,
                    // A tuple coordinate has no nominal source declaration.
                    Field::Field(DefaultFieldRefV1::Tuple { .. }) => continue,
                },
            }
        }
        Ok(())
    }

    fn default_constructor(&mut self, constructor: Constructor<'_>) -> Result<(), Error> {
        let (owner_type, source) = match constructor {
            Constructor::Constructor(constructor) => (
                constructor.owner_type(),
                match constructor {
                    DefaultConstructorRefV1::Struct { declaration, .. }
                    | DefaultConstructorRefV1::Class {
                        declaration: DefaultClassConstructorIdV1::Source(declaration),
                        ..
                    } => Some(CallableTemplateOrigin::Constructor(*declaration)),
                    DefaultConstructorRefV1::Variant { declaration, .. } => {
                        Some(CallableTemplateOrigin::VariantConstructor(*declaration))
                    }
                    DefaultConstructorRefV1::Class {
                        declaration: DefaultClassConstructorIdV1::Generated(_),
                        ..
                    } => None,
                },
            ),
            Constructor::Variant(variant) => (
                variant.owner_type(),
                Some(CallableTemplateOrigin::VariantConstructor(
                    variant.declaration(),
                )),
            ),
        };
        self.signature(owner_type)?;
        if let Some(source) = source {
            self.callable(source)?;
        }
        Ok(())
    }
}

struct Occurrences<'a>(Vec<DefaultBodyReferenceOccurrenceV1<'a>>);

impl<'a> DefaultBodyReferenceVisitorV1<'a> for Occurrences<'a> {
    type Error = Error;

    fn expression(
        &mut self,
        _: u32,
        _: &'a scoop_hir::DefaultExpressionV1,

        _: &WirePath,
    ) -> Result<(), Error> {
        // Dependency edges are emitted by the reference callback, including
        // metadata types and generated descriptors attached to expressions.
        Ok(())
    }

    fn reference(
        &mut self,
        occurrence: DefaultBodyReferenceOccurrenceV1<'a>,

        path: &WirePath,
    ) -> Result<(), Error> {
        scoop_wire::allocation::try_reserve(&mut self.0, 1, path)?;
        self.0.push(occurrence);
        Ok(())
    }
}
