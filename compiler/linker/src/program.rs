//! The resolved object and symbol inputs of one executable.
use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{ConeIdentity, PersistentSymbolRequest};
use scoop_slib::{LinkDefinitionOwnerV1, ProgramLinkClosure, SlibMemberId};
use scoop_toolchain::ValidatedFinalLinkProfile;

use crate::native_input::{NativeInputId, NativeInputs};
use crate::{LinkError, RuntimeObjectId, RuntimeObjectSet, error};

pub(crate) mod native;
mod object;
pub(crate) use object::{InputObject, ObjectBytes};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum DefinitionOwner {
    Scoop(LinkDefinitionOwnerV1),
    Runtime(RuntimeObjectId),
    Native(NativeInputId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ObjectOrigin {
    Cone {
        cone: ConeIdentity,
        member: SlibMemberId,
    },
    Runtime(RuntimeObjectId),
    Native(NativeInputId),
}
impl std::fmt::Display for ObjectOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cone { cone, member } => write!(f, "cone-{cone}-{member}"),
            Self::Runtime(id) => write!(f, "runtime-{id}"),
            Self::Native(id) => write!(f, "native-{id}"),
        }
    }
}

pub(crate) struct ProgramInputs<'a> {
    pub objects: Vec<InputObject<'a>>,
    pub definitions: BTreeMap<String, DefinitionOwner>,
    pub requirements: BTreeSet<String>,
    pub dynamic: BTreeSet<String>,
    pub images: Vec<String>,
    pub root: String,
    pub string_target: String,
    pub strong_relocations: Vec<&'a scoop_slib::VerifiedCurrentConeStrongRelocationClosureV1>,
    pub native: NativeInputs,
}

impl<'a> ProgramInputs<'a> {
    pub fn new(
        closure: &'a ProgramLinkClosure,
        runtime: &'a RuntimeObjectSet,
        profile: &ValidatedFinalLinkProfile,
        library_paths: &[std::path::PathBuf],
    ) -> Result<Self, LinkError> {
        if runtime.target() != profile.target() {
            return Err(error("runtime target differs from final-link target"));
        }
        let mut objects = Vec::new();
        let mut definitions = BTreeMap::new();
        let mut requirements = BTreeSet::new();
        let mut images = Vec::new();
        let mut root = None;
        let mut string_target = None;
        let mut strong_relocations = Vec::new();
        for (artifact, symbols) in closure.artifacts() {
            let cone = artifact.identity();
            strong_relocations.push(
                symbols
                    .object_contents()
                    .patch_sites()
                    .builtins()
                    .strong_relocations(),
            );
            images.push(object_symbol(
                artifact.production().image_plan().symbol(),
                profile,
            ));
            if let scoop_lir::EntryProductionPlanV1::Executable(entry) =
                artifact.production().entry_plan()
            {
                if cone != closure.root() || root.is_some() {
                    return Err(error(format!("multiple executable entries at Cone {cone}")));
                }
                root = Some(object_symbol(entry.root_descriptor_symbol(), profile));
            }
            let mut members = BTreeMap::new();
            for object in symbols.final_objects().objects() {
                members.insert(object.member(), object.bytes());
            }
            for object in symbols.object_contents().generated_objects() {
                members.insert(object.member(), object.bytes());
            }
            objects.extend(members.into_iter().map(|(member, bytes)| InputObject {
                origin: ObjectOrigin::Cone { cone, member },
                bytes: ObjectBytes::Borrowed(bytes),
            }));
            let odr_symbols: BTreeSet<_> = symbols
                .object_contents()
                .patch_sites()
                .builtins()
                .strong_relocations()
                .members()
                .iter()
                .flat_map(|member| member.definitions().symbols())
                .filter(|symbol| {
                    matches!(
                        symbol.definition_owner(),
                        scoop_identity::ObjectDefinitionPlanOwner::Odr { .. }
                    )
                })
                .map(|symbol| symbol.macho_name())
                .collect();
            for definition in symbols.defined_symbols().owners() {
                let name = String::from_utf8(definition.symbol().to_vec()).map_err(error)?;
                let owner = DefinitionOwner::Scoop(definition.owner());
                if let Some(previous) = definitions.insert(name.clone(), owner)
                    && (previous != owner || !odr_symbols.contains(name.as_bytes()))
                {
                    return Err(error(format!(
                        "multiple or conflicting Strong owners for {name}: {previous:?}, {owner:?}; Cone {cone}"
                    )));
                }
            }
            for alias in symbols.link_support().runtime_data_aliases() {
                let record = symbols
                    .defined_symbols()
                    .owners()
                    .iter()
                    .find(|record| {
                        record.owner() == LinkDefinitionOwnerV1::StrongDefinition(alias.owner())
                    })
                    .ok_or_else(|| {
                        error(format!(
                            "Cone {cone}: runtime String alias has no physical TD definition"
                        ))
                    })?;
                let target = String::from_utf8(record.symbol().to_vec()).map_err(error)?;
                if string_target.replace(target).is_some() {
                    return Err(error("runtime String alias has more than one provider"));
                }
            }
            for requirement in symbols.undefined_partitions().legacy().requirements() {
                requirements.insert(
                    String::from_utf8(requirement.use_site().symbol().to_vec()).map_err(error)?,
                );
            }
        }
        for object in runtime.objects() {
            for symbol in object.info().definitions.keys() {
                if let Some(previous) =
                    definitions.insert(symbol.clone(), DefinitionOwner::Runtime(object.id()))
                {
                    return Err(error(format!(
                        "runtime object {} conflicts with {previous:?} at {symbol}",
                        object.id()
                    )));
                }
            }
            requirements.extend(object.info().requirements.iter().cloned());
            objects.push(InputObject {
                origin: ObjectOrigin::Runtime(object.id()),
                bytes: ObjectBytes::Borrowed(object.bytes()),
            });
        }
        let root = root.ok_or_else(|| error("executable closure has no root entry"))?;
        let string_target = string_target
            .ok_or_else(|| error("executable closure has no typed runtime String alias"))?;
        requirements.extend(images.iter().cloned());
        requirements.extend([root.clone(), "_scoop_rt_run_program".to_owned()]);
        requirements.extend(
            profile
                .linker_system_requirements()
                .iter()
                .map(|symbol| (*symbol).to_owned()),
        );
        let mut result = Self {
            objects,
            definitions,
            requirements,
            dynamic: BTreeSet::new(),
            images,
            root,
            string_target,
            strong_relocations,
            native: NativeInputs::default(),
        };
        native::resolve(closure, runtime, profile, library_paths, &mut result)?;
        Ok(result)
    }
}

fn object_symbol(request: PersistentSymbolRequest, profile: &ValidatedFinalLinkProfile) -> String {
    profile
        .target()
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(request.symbol().as_str())
}
