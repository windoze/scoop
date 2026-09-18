//! Persistent identities for direct public package bindings.

use std::collections::HashSet;

use la_arena::{Arena, Idx};
use scoop_identity::{
    BindingTarget, BindingTargetError, CborIdentityRecord, DeclarationName, DeclarationScope,
    ExportBindingKey, PersistentExportBindingId, SourceDeclarationKey,
};

use crate::{
    ClassDecl, EnumDecl, Function, HirFunctionIdentities, HirFunctionIdentity,
    HirNominalIdentities, HirObjectValueIdentities, HirPropertyIdentities, HirPropertyIdentity,
    InterfaceDecl, ObjectDecl, Property, PublicSemanticSurface, SingletonValue, StructDecl,
    TypeAliasDecl,
};

mod error;
mod surface;
pub use error::{HirExportBindingEntityKind, HirExportBindingIdentityError};
pub use surface::HirExportBindingSurfaceValidationError;

pub type HirExportBindingIdentity = CborIdentityRecord<PersistentExportBindingId, ExportBindingKey>;

pub struct HirExportBindingIdentityInputs<'a> {
    pub surface: &'a PublicSemanticSurface,
    pub structs: &'a Arena<StructDecl>,
    pub enums: &'a Arena<EnumDecl>,
    pub classes: &'a Arena<ClassDecl>,
    pub interfaces: &'a Arena<InterfaceDecl>,
    pub objects: &'a Arena<ObjectDecl>,
    pub singleton_values: &'a Arena<SingletonValue>,
    pub functions: &'a Arena<Function>,
    pub properties: &'a Arena<Property>,
    pub type_aliases: &'a Arena<TypeAliasDecl>,
    pub nominal_identities: &'a HirNominalIdentities,
    pub object_value_identities: &'a HirObjectValueIdentities,
    pub function_identities: &'a HirFunctionIdentities,
    pub property_identities: &'a HirPropertyIdentities,
    pub type_alias_identities: &'a crate::HirTypeAliasIdentities,
}

/// Canonically ordered direct package bindings for the public HIR surface.
#[derive(Clone, Debug)]
pub struct HirExportBindingIdentities {
    records: Vec<HirExportBindingIdentity>,
}

impl HirExportBindingIdentities {
    pub fn from_public_surface(
        inputs: HirExportBindingIdentityInputs<'_>,
    ) -> Result<Self, HirExportBindingIdentityError> {
        let mut records = Vec::new();

        collect_nominals(
            &inputs.surface.structs,
            inputs.structs,
            inputs.nominal_identities,
            HirExportBindingEntityKind::Struct,
            &mut records,
        )?;
        collect_nominals(
            &inputs.surface.enums,
            inputs.enums,
            inputs.nominal_identities,
            HirExportBindingEntityKind::Enum,
            &mut records,
        )?;
        collect_nominals(
            &inputs.surface.classes,
            inputs.classes,
            inputs.nominal_identities,
            HirExportBindingEntityKind::Class,
            &mut records,
        )?;
        collect_nominals(
            &inputs.surface.interfaces,
            inputs.interfaces,
            inputs.nominal_identities,
            HirExportBindingEntityKind::Interface,
            &mut records,
        )?;

        for &object in &inputs.surface.objects {
            require_index(object, inputs.objects, HirExportBindingEntityKind::Object)?;
            let source = inputs.nominal_identities[object].source().ok_or(
                HirExportBindingIdentityError::NonSourceEntity {
                    kind: HirExportBindingEntityKind::Object,
                    index: raw_index(object),
                },
            )?;
            let declaration = source.declaration();
            require_public_binding_declaration(
                declaration,
                HirExportBindingEntityKind::Object,
                raw_index(object),
            )?;
            push_binding(
                declaration,
                BindingTarget::type_name(declaration),
                HirExportBindingEntityKind::Object,
                raw_index(object),
                &mut records,
            )?;
            let value = inputs.objects[object].singleton_value;
            if local_index(value) >= inputs.singleton_values.len() {
                return Err(HirExportBindingIdentityError::UnknownObjectValue {
                    object: raw_index(object),
                    value: raw_index(value),
                });
            }
            let identity = &inputs.object_value_identities[value];
            if identity.declaration() != object {
                return Err(HirExportBindingIdentityError::ObjectValueRelation {
                    object: raw_index(object),
                    value: raw_index(value),
                });
            }
            push_binding(
                declaration,
                BindingTarget::object_value(declaration),
                HirExportBindingEntityKind::Object,
                raw_index(object),
                &mut records,
            )?;
        }

        for &function in &inputs.surface.functions {
            require_index(
                function,
                inputs.functions,
                HirExportBindingEntityKind::Function,
            )?;
            let HirFunctionIdentity::Source(source) = &inputs.function_identities[function] else {
                return Err(HirExportBindingIdentityError::NonSourceEntity {
                    kind: HirExportBindingEntityKind::Function,
                    index: raw_index(function),
                });
            };
            let declaration = source.declaration();
            if !is_direct_package_declaration(
                declaration,
                HirExportBindingEntityKind::Function,
                raw_index(function),
            )? {
                continue;
            }
            let target = if declaration.duplicate_signature().receiver_is_present() {
                BindingTarget::extension_function(declaration)
            } else {
                BindingTarget::function(declaration)
            };
            push_binding(
                declaration,
                target,
                HirExportBindingEntityKind::Function,
                raw_index(function),
                &mut records,
            )?;
        }

        for &property in &inputs.surface.properties {
            require_index(
                property,
                inputs.properties,
                HirExportBindingEntityKind::Property,
            )?;
            let identity = &inputs.property_identities[property];
            let declaration = identity.declaration();
            if !is_direct_package_declaration(
                declaration,
                HirExportBindingEntityKind::Property,
                raw_index(property),
            )? {
                continue;
            }
            let target = match identity {
                HirPropertyIdentity::Ordinary(_) => BindingTarget::property(declaration),
                HirPropertyIdentity::Extension(_) => BindingTarget::extension_property(declaration),
            };
            push_binding(
                declaration,
                target,
                HirExportBindingEntityKind::Property,
                raw_index(property),
                &mut records,
            )?;
        }

        for &alias in &inputs.surface.type_aliases {
            require_index(
                alias,
                inputs.type_aliases,
                HirExportBindingEntityKind::TypeAlias,
            )?;
            let declaration = inputs.type_alias_identities[alias].key();
            if !is_direct_package_declaration(
                declaration,
                HirExportBindingEntityKind::TypeAlias,
                raw_index(alias),
            )? {
                continue;
            }
            push_binding(
                declaration,
                BindingTarget::type_alias(declaration),
                HirExportBindingEntityKind::TypeAlias,
                raw_index(alias),
                &mut records,
            )?;
        }

        Self::canonicalize(records)
    }

    pub fn canonicalize(
        mut records: Vec<HirExportBindingIdentity>,
    ) -> Result<Self, HirExportBindingIdentityError> {
        records.sort_unstable_by_key(CborIdentityRecord::id);
        let mut seen = HashSet::with_capacity(records.len());
        for record in &records {
            if !seen.insert(record.id()) {
                return Err(HirExportBindingIdentityError::DuplicateIdentity {
                    identity: record.id(),
                });
            }
        }
        Ok(Self { records })
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &HirExportBindingIdentity> {
        self.records.iter()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

fn collect_nominals<T>(
    surface: &[Idx<T>],
    declarations: &Arena<T>,
    identities: &HirNominalIdentities,
    kind: HirExportBindingEntityKind,
    records: &mut Vec<HirExportBindingIdentity>,
) -> Result<(), HirExportBindingIdentityError>
where
    HirNominalIdentities: std::ops::Index<Idx<T>, Output = crate::HirNominalIdentity>,
{
    for &id in surface {
        require_index(id, declarations, kind)?;
        let source =
            identities[id]
                .source()
                .ok_or(HirExportBindingIdentityError::NonSourceEntity {
                    kind,
                    index: raw_index(id),
                })?;
        let declaration = source.declaration();
        require_public_binding_declaration(declaration, kind, raw_index(id))?;
        push_binding(
            declaration,
            BindingTarget::type_name(declaration),
            kind,
            raw_index(id),
            records,
        )?;
    }
    Ok(())
}

fn push_binding(
    declaration: &SourceDeclarationKey,
    target: Result<BindingTarget, BindingTargetError>,
    kind: HirExportBindingEntityKind,
    index: u32,
    records: &mut Vec<HirExportBindingIdentity>,
) -> Result<(), HirExportBindingIdentityError> {
    let target = target.map_err(|error| HirExportBindingIdentityError::InvalidTarget {
        kind,
        index,
        error,
    })?;
    let DeclarationName::Named(name) = declaration.name() else {
        return Err(HirExportBindingIdentityError::UnnamedDeclaration { kind, index });
    };
    let key = ExportBindingKey::new(
        declaration.origin(),
        declaration.package().clone(),
        name.clone(),
        target,
    );
    let record = CborIdentityRecord::from_key(key)
        .map_err(|error| HirExportBindingIdentityError::InvalidIdentity { kind, index, error })?;
    records.push(record);
    Ok(())
}

fn is_direct_package_declaration(
    declaration: &SourceDeclarationKey,
    kind: HirExportBindingEntityKind,
    index: u32,
) -> Result<bool, HirExportBindingIdentityError> {
    if !declaration.owners().owners().is_empty() {
        return Ok(false);
    }
    require_public_binding_declaration(declaration, kind, index).map(|()| true)
}

fn require_public_binding_declaration(
    declaration: &SourceDeclarationKey,
    kind: HirExportBindingEntityKind,
    index: u32,
) -> Result<(), HirExportBindingIdentityError> {
    if matches!(declaration.scope(), DeclarationScope::ConeWide) {
        Ok(())
    } else {
        Err(HirExportBindingIdentityError::NonPublicPackageDeclaration { kind, index })
    }
}

fn require_index<T>(
    id: Idx<T>,
    arena: &Arena<T>,
    kind: HirExportBindingEntityKind,
) -> Result<(), HirExportBindingIdentityError> {
    if local_index(id) < arena.len() {
        Ok(())
    } else {
        Err(HirExportBindingIdentityError::UnknownSurfaceEntity {
            kind,
            index: raw_index(id),
        })
    }
}

fn local_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

fn raw_index<T>(id: Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
