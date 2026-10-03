//! Conditional release-value facts over the finite nominal template graph.

use std::collections::{HashMap, HashSet};

use scoop_hir as hir;

use crate::{CoreLoweringAuthority, Lowerer, Owner};

type Requirements = Option<HashSet<hir::TypeParamId>>;

pub(crate) struct ReleaseValueFacts {
    templates: HashMap<hir::SourceNominalId, Requirements>,
}

impl ReleaseValueFacts {
    pub(super) fn infer(lowerer: &Lowerer) -> Self {
        let forbidden = match &lowerer.core {
            CoreLoweringAuthority::Defined => [
                lowerer.ffi_pinned_ptr,
                lowerer.ffi_gc_handle,
                lowerer.ffi_foreign_callback,
            ]
            .into_iter()
            .flatten()
            .map(|id| lowerer.nominal_identity(Owner::Struct(id)).declaration_id())
            .collect::<Vec<_>>(),
            CoreLoweringAuthority::Imported(core) => vec![
                hir::SourceNominalId::GenericTemplate(core.ffi().pinned_ptr().persistent()),
                hir::SourceNominalId::GenericTemplate(core.ffi().gc_handle().persistent()),
                hir::SourceNominalId::GenericTemplate(
                    core.foreign_callbacks().callback().persistent(),
                ),
            ],
        };
        let mut fields = HashMap::new();
        for (id, definition) in lowerer
            .structs
            .iter()
            .map(|(id, declaration)| {
                (
                    lowerer.nominal_identity(Owner::Struct(id)).declaration_id(),
                    &declaration.definition,
                )
            })
            .chain(
                lowerer
                    .loaded_struct_definitions
                    .iter()
                    .map(|(&id, declaration)| (id, &declaration.definition)),
            )
        {
            fields.insert(
                id,
                definition
                    .semantic_fields()
                    .iter()
                    .map(|field| field.ty)
                    .collect::<Vec<_>>(),
            );
        }
        for (id, definition) in lowerer
            .enums
            .iter()
            .map(|(id, declaration)| {
                (
                    lowerer.nominal_identity(Owner::Enum(id)).declaration_id(),
                    &declaration.definition,
                )
            })
            .chain(
                lowerer
                    .loaded_enum_definitions
                    .iter()
                    .map(|(&id, declaration)| (id, &declaration.definition)),
            )
        {
            fields.insert(
                id,
                definition
                    .variants
                    .iter()
                    .flat_map(|variant| variant.fields.iter().map(|field| field.ty))
                    .collect(),
            );
        }
        let mut facts = Self {
            templates: fields
                .keys()
                .map(|&id| (id, (!forbidden.contains(&id)).then(HashSet::new)))
                .collect(),
        };
        // Conditions can only gain one of the declaration's own parameters,
        // or become impossible. Following a pointer consults a template fact
        // rather than expanding an ever-growing recursive type application.
        loop {
            let mut changed = false;
            for (&id, fields) in &fields {
                if facts.templates[&id].is_none() {
                    continue;
                }
                let next = facts.values(lowerer, fields.iter().copied());
                if next != facts.templates[&id] {
                    facts.templates.insert(id, next);
                    changed = true;
                }
            }
            if !changed {
                return facts;
            }
        }
    }

    pub(super) fn values(
        &self,
        lowerer: &Lowerer,
        types: impl IntoIterator<Item = hir::TypeId>,
    ) -> Requirements {
        let mut required = HashSet::new();
        for ty in types {
            required.extend(self.requirements(lowerer, ty)?);
        }
        Some(required)
    }

    pub(super) fn requirements(&self, lowerer: &Lowerer, ty: hir::TypeId) -> Requirements {
        match &lowerer.types[ty] {
            hir::Type::Unit | hir::Type::Integer(_) | hir::Type::Boolean => Some(HashSet::new()),
            hir::Type::Param(parameter) => Some(HashSet::from([*parameter])),
            hir::Type::Ptr(pointee) => self.requirements(lowerer, *pointee),
            hir::Type::Tuple(fields) => self.values(lowerer, fields.iter().copied()),
            hir::Type::Struct(application) => {
                let application = &lowerer.struct_applications[*application];
                self.application(
                    lowerer,
                    application.template,
                    &lowerer.struct_definition(application.template).type_params,
                    &application.arguments,
                )
            }
            hir::Type::Enum(application) => {
                let application = &lowerer.enum_applications[*application];
                self.application(
                    lowerer,
                    application.template,
                    &lowerer.enum_definition(application.template).type_params,
                    &application.arguments,
                )
            }
            hir::Type::String
            | hir::Type::Class(_)
            | hir::Type::Interface(_)
            | hir::Type::Any
            | hir::Type::Function(_)
            | hir::Type::FunPtr(_) => None,
        }
    }

    fn application(
        &self,
        lowerer: &Lowerer,
        template: hir::SourceNominalId,
        parameters: &[hir::TypeParamDecl],
        arguments: &[hir::TypeId],
    ) -> Requirements {
        let required = self.templates[&template].as_ref()?;
        self.values(
            lowerer,
            parameters
                .iter()
                .zip(arguments)
                .filter_map(|(parameter, &argument)| {
                    required.contains(&parameter.id).then_some(argument)
                }),
        )
    }
}
