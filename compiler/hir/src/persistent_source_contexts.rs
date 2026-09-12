//! Persistent identities aligned with the export HIR source-context arena.

use std::collections::{HashMap, HashSet};
use std::ops::Index;

use la_arena::{Arena, Idx};
use scoop_identity::{CborIdentityRecord, PersistentSourceContextId, SourceContextKey};

use crate::{
    AnonymousFunction, ClassConstructor, ClassDecl, EnumDecl, Function, HirConstructorIdentities,
    HirFunctionIdentities, HirInitializationUnitIdentities, HirNominalIdentities,
    HirPropertyAccessorIdentities, HirPropertyIdentities, InitializationUnit, InterfaceDecl,
    Lambda, ObjectDecl, Property, SingletonValue, SourceContext, SourceContextId,
    SourceFileMetadata, StructConstructor, StructDecl,
};

mod error;
mod resolution;
pub use error::{HirSourceContextIdentityError, HirSourceContextReferenceKind};
use resolution::context_key;

pub type HirSourceContextIdentity = CborIdentityRecord<PersistentSourceContextId, SourceContextKey>;

#[derive(Clone, Copy)]
pub struct HirSourceContextIdentityInputs<'a> {
    pub source_files: &'a [SourceFileMetadata],
    pub source_contexts: &'a Arena<SourceContext>,
    pub structs: &'a Arena<StructDecl>,
    pub enums: &'a Arena<EnumDecl>,
    pub classes: &'a Arena<ClassDecl>,
    pub interfaces: &'a Arena<InterfaceDecl>,
    pub objects: &'a Arena<ObjectDecl>,
    pub functions: &'a Arena<Function>,
    pub struct_constructors: &'a Arena<StructConstructor>,
    pub class_constructors: &'a Arena<ClassConstructor>,
    pub properties: &'a Arena<Property>,
    pub initialization_units: &'a Arena<InitializationUnit>,
    pub singleton_values: &'a Arena<SingletonValue>,
    pub lambdas: &'a Arena<Lambda>,
    pub anonymous_functions: &'a Arena<AnonymousFunction>,
    pub nominal_identities: &'a HirNominalIdentities,
    pub function_identities: &'a HirFunctionIdentities,
    pub property_accessor_identities: &'a HirPropertyAccessorIdentities,
    pub constructor_identities: &'a HirConstructorIdentities,
    pub property_identities: &'a HirPropertyIdentities,
    pub initialization_unit_identities: &'a HirInitializationUnitIdentities,
}

/// Total persistent identity relation for every export HIR source context.
#[derive(Clone, Debug)]
pub struct HirSourceContextIdentities {
    identities: Vec<HirSourceContextIdentity>,
}

impl HirSourceContextIdentities {
    pub fn from_contexts(
        inputs: HirSourceContextIdentityInputs<'_>,
    ) -> Result<Self, HirSourceContextIdentityError> {
        let identities = inputs
            .source_contexts
            .iter()
            .map(|(context, value)| {
                let key = context_key(&inputs, context, value)?;
                CborIdentityRecord::from_key(key).map_err(|error| {
                    HirSourceContextIdentityError::InvalidIdentity {
                        context: raw_index(context),
                        error,
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::checked(inputs, identities)
    }

    pub fn checked(
        inputs: HirSourceContextIdentityInputs<'_>,
        identities: Vec<HirSourceContextIdentity>,
    ) -> Result<Self, HirSourceContextIdentityError> {
        if identities.len() != inputs.source_contexts.len() {
            return Err(HirSourceContextIdentityError::Length {
                expected: inputs.source_contexts.len(),
                actual: identities.len(),
            });
        }
        validate_sources(&inputs)?;

        let mut seen = HashSet::with_capacity(identities.len());
        for (context, value) in inputs.source_contexts.iter() {
            let expected = context_key(&inputs, context, value)?;
            let identity = &identities[local_index(context)];
            if identity.key() != &expected {
                return Err(HirSourceContextIdentityError::IdentityMismatch {
                    context: raw_index(context),
                });
            }
            if !seen.insert(identity.id()) {
                return Err(HirSourceContextIdentityError::DuplicateIdentity {
                    context: raw_index(context),
                    identity: identity.id(),
                });
            }
        }
        Ok(Self { identities })
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &HirSourceContextIdentity> {
        self.identities.iter()
    }

    pub fn get(&self, context: SourceContextId) -> Option<&HirSourceContextIdentity> {
        self.identities.get(local_index(context))
    }

    pub fn len(&self) -> usize {
        self.identities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.identities.is_empty()
    }
}

impl Index<SourceContextId> for HirSourceContextIdentities {
    type Output = HirSourceContextIdentity;

    fn index(&self, id: SourceContextId) -> &Self::Output {
        &self.identities[local_index(id)]
    }
}

fn validate_sources(
    inputs: &HirSourceContextIdentityInputs<'_>,
) -> Result<(), HirSourceContextIdentityError> {
    let mut sources = HashMap::with_capacity(inputs.source_files.len());
    for (index, source) in inputs.source_files.iter().enumerate() {
        if let Some(first) = sources.insert(&source.identity, index) {
            return Err(HirSourceContextIdentityError::DuplicateSource {
                first,
                duplicate: index,
            });
        }
    }
    let mut file_contexts = vec![false; inputs.source_files.len()];
    for (context, value) in inputs.source_contexts.iter() {
        let Some(&source) = sources.get(value.source()) else {
            return Err(HirSourceContextIdentityError::UnknownSource {
                context: raw_index(context),
            });
        };
        if matches!(value.subject(), crate::SourceContextSubject::File) {
            file_contexts[source] = true;
        }
    }
    match file_contexts.iter().position(|present| !present) {
        Some(source) => Err(HirSourceContextIdentityError::MissingFileContext { source }),
        None => Ok(()),
    }
}

fn local_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

fn raw_index<T>(id: Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
