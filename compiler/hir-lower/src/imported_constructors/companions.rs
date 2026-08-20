use super::*;
use scoop_identity::{
    CborIdentityRecord, DefinitionOwnerAtom, GeneratedCallableKey, InitializationCallableRole,
    InitializationUnitKey, PersistentInitializationUnitId,
};

mod dependencies;

impl Lowerer {
    pub(crate) fn request_imported_companion(
        &mut self,
        owner: scoop_identity::PersistentGenericTypeId,
    ) -> Result<hir::ImportedCompanionTemplateId, String> {
        let nominal = hir::SourceNominalId::GenericTemplate(owner);
        if let Some((id, _)) = self
            .imported_companion_templates
            .iter()
            .find(|(_, template)| template.declaration.owner() == nominal)
        {
            return Ok(id);
        }
        let dependencies = self
            .dependencies
            .as_ref()
            .ok_or("companion has no dependency catalog")?;
        let declaration = dependencies
            .nominal_declaration(nominal)
            .cloned()
            .ok_or("companion declaration is missing")?;
        if !matches!(declaration.interface.source_shape(), hir::NominalSourceShapeV1::Object(shape)
            if shape.object_kind() == hir::ObjectSourceKindV1::Companion)
        {
            return Err("generic singleton declaration is not a companion".into());
        }
        let host = match declaration.identity.declaration().owners().owners().last() {
            Some(DefinitionOwnerAtom::Type(id)) => hir::SourceNominalId::Concrete(*id),
            Some(DefinitionOwnerAtom::GenericType(id)) => {
                hir::SourceNominalId::GenericTemplate(*id)
            }
            _ => return Err("companion has no nominal host".into()),
        };
        let host = dependencies
            .nominal_declaration(host)
            .cloned()
            .ok_or("companion host declaration is missing")?;
        let mut names = Vec::new();
        for owner in declaration.identity.declaration().owners().owners() {
            let owner = match owner {
                DefinitionOwnerAtom::Type(id) => hir::SourceNominalId::Concrete(*id),
                DefinitionOwnerAtom::GenericType(id) => hir::SourceNominalId::GenericTemplate(*id),
                _ => return Err("companion owner chain contains a non-nominal declaration".into()),
            };
            names.push(
                dependencies
                    .nominal_declaration(owner)
                    .ok_or("companion enclosing declaration is missing")?
                    .name()
                    .to_owned(),
            );
        }
        names.push(declaration.name().to_owned());
        let display_name = format!("companion:{}", names.join("."));
        let initialization = dependencies
            .nominal_initialization(owner)
            .ok_or("companion initialization template is missing")?;
        let [constructor] = initialization.initialization().constructors() else {
            return Err("companion requires one hidden initializer".into());
        };
        let constructor = source_constructor(constructor.declaration())
            .ok_or("companion initializer identity is missing")?;
        let source = dependencies
            .callable_declaration(CallableTemplateOrigin::Constructor(constructor))
            .map_err(|e| e.to_string())?;
        let definition = initialization
            .definition_source(&declaration.origin)
            .ok_or("companion definition source is missing")?;
        let mut origin = self
            .import_dependency_definition_origin(&declaration.origin, definition)
            .map_err(|e| e.to_string())?;
        let constructor = self.request_imported_constructor_template(source)?;
        let unit = PersistentInitializationUnitId::from_key(
            &InitializationUnitKey::GenericCompanionTemplate(owner),
        )
        .map_err(|e| e.to_string())?;
        let context = scoop_identity::SourceContextKey::Initialization {
            source: declaration.origin.origin().source().clone(),
            unit,
        };
        let context = self.imported_constructor_templates[constructor]
            .source
            .source_context(&context)
            .ok_or("companion initialization source context is missing")?
            .clone();
        origin.context = self.intern_imported_source_context(
            context,
            hir::SourceContextNames {
                function: String::new(),
                type_name: declaration.name().to_owned(),
            },
        );
        let callable = |role| {
            CborIdentityRecord::from_key(GeneratedCallableKey::Initialization { unit, role })
                .map_err(|e| e.to_string())
        };
        let signature = hir::CallableSignature::from_source_effects(
            format!("{}$initialize", declaration.name()),
            Vec::new(),
            self.unit,
            self.imported_constructor_templates[constructor]
                .signature
                .effects
                .clone(),
            hir::ReleaseCallability::Unavailable,
            origin.span,
            Vec::new(),
        );
        Ok(self
            .imported_companion_templates
            .alloc(hir::ImportedCompanionTemplate {
                declaration,
                host,
                constructor,
                dependencies: Vec::new(),
                signature,
                display_name,
                origin,
                initializer: callable(InitializationCallableRole::Initializer)?,
                ensure: callable(InitializationCallableRole::Ensure)?,
            }))
    }
}
