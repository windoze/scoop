use super::*;
use hir::{SemanticBodyAvailability as Availability, SemanticConstructor as Constructor};

mod imported;

impl BodyQueries<'_> {
    pub(in crate::semantic_queries) fn constructor(
        &mut self,
        constructor: Constructor,
    ) -> Result<Option<Availability<Arc<hir::SemanticConstructorBody>>>, Diagnostic> {
        if let Some(body) = self.constructors.get(&constructor) {
            return Ok(Some(body.clone()));
        }
        let body = match constructor {
            Constructor::CurrentStruct(id) => Some(self.struct_constructor(id)?),
            Constructor::CurrentClass(id) => self.class_constructor(id)?,
            Constructor::Imported(id) => self.imported_constructor(id),
        };
        if let Some(body) = &body {
            self.constructors.insert(constructor, body.clone());
        }
        Ok(body)
    }

    fn struct_constructor(
        &self,
        id: hir::StructConstructorId,
    ) -> Result<Availability<Arc<hir::SemanticConstructorBody>>, Diagnostic> {
        let owner = self.lowerer.struct_constructors[id].owner;
        if !matches!(
            self.lowerer.structs[owner].representation,
            hir::StructRepresentation::Declared(_)
        ) {
            return Ok(Availability::NoBody);
        }
        let data = self.data()?;
        let declaration = data
            .inputs(self.lowerer)
            .project_struct_constructor(id, Some(&data.imported))
            .map_err(projection_error)?;
        Ok(Availability::Available(Arc::new(
            hir::SemanticConstructorBody {
                declaration: Arc::new(declaration),
                common: Arc::from([]),
            },
        )))
    }

    fn class_constructor(
        &mut self,
        id: hir::ClassConstructorId,
    ) -> Result<Option<Availability<Arc<hir::SemanticConstructorBody>>>, Diagnostic> {
        let data = self.data()?;
        if data.constructors[id].source_record().is_none() {
            return Ok(None);
        }
        let owner = self.lowerer.class_constructors[id].owner;
        if !self.lowerer.classes[owner].has_source_constructors() {
            return Ok(Some(Availability::NoBody));
        }
        let declaration = data
            .inputs(self.lowerer)
            .project_class_constructor(id, Some(&data.imported))
            .map_err(projection_error)?;
        let common = if let Some(common) = self.class_common.get(&owner) {
            Arc::clone(common)
        } else {
            let common: Arc<[_]> = data
                .inputs(self.lowerer)
                .project_class_common_initialization(owner, Some(&data.imported))
                .map_err(projection_error)?
                .into();
            self.class_common.insert(owner, Arc::clone(&common));
            common
        };
        Ok(Some(Availability::Available(Arc::new(
            hir::SemanticConstructorBody {
                declaration: Arc::new(declaration),
                common,
            },
        ))))
    }
}
