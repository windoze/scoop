//! The resolved object and symbol inputs of one executable.
use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{ConeIdentity, PersistentSymbolRequest};
use scoop_slib::{LinkDefinitionOwnerV1, ProgramLinkClosure, SlibMemberId};
use scoop_toolchain::ValidatedFinalLinkProfile;

use crate::native_input::{NativeInputs, NativeObjectId};
use crate::{LinkError, RuntimeObjectId, RuntimeObjectSet, error};

pub(crate) mod image;
pub(crate) mod native;
mod selection;
use selection::SelectedMembers;
mod object;
pub(crate) use object::{InputObject, ObjectBytes};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum DefinitionOwner {
    Scoop(LinkDefinitionOwnerV1),
    Runtime(RuntimeObjectId),
    Native(NativeObjectId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ObjectOrigin {
    Cone {
        cone: ConeIdentity,
        member: SlibMemberId,
    },
    Runtime(RuntimeObjectId),
    Native(NativeObjectId),
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
    pub requirement_origins: BTreeMap<String, Vec<String>>,
    pub namespace: crate::namespace::NativeNamespace,
    pub target: scoop_lir::LirTargetProfile,
    pub images: Vec<String>,
    pub final_images: Vec<image::SelectedImage>,
    pub selected: SelectedMembers,
    pub root: String,
    pub string_target: String,
    pub native: NativeInputs,
    pub windows_subsystem: scoop_identity::WindowsSubsystem,
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
        let (declarations, libraries) = native::declarations::read(closure)?;
        let mut native = NativeInputs::read(libraries, library_paths, profile)?;
        native.include_embedded(closure, profile)?;
        let namespace =
            crate::namespace::NativeNamespace::read(&mut native, library_paths, profile)?;
        // These indexes were already read for native symbol selection. Candidate
        // archive references conservatively retain Scoop entries without pulling
        // the native members themselves into the final link.
        let native_references = native
            .files
            .values()
            .flat_map(|file| file.objects())
            .flat_map(|(_, index, _, _)| &index.requirements)
            .chain(
                runtime
                    .objects()
                    .iter()
                    .flat_map(|object| &object.info().requirements),
            )
            .map(String::as_str);
        let selected = SelectedMembers::new(closure, native_references)?;
        let mut final_images = Vec::new();
        let mut objects = Vec::new();
        let mut definitions = BTreeMap::new();
        let mut requirements = BTreeSet::new();
        let mut requirement_origins: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut images = Vec::new();
        let mut root = None;
        let mut windows_subsystem = scoop_identity::WindowsSubsystem::Console;
        let mut string_target = None;
        for (artifact, symbols) in closure.artifacts() {
            let cone = artifact.identity();
            let image = image::SelectedImage::new(artifact, selected.definitions(cone), profile)?;
            images.push(image.name.clone());
            requirements.extend(image.references().cloned());
            for name in image.defined_names() {
                let owner = symbols
                    .defined_symbols()
                    .owners()
                    .iter()
                    .find(|record| record.symbol() == name.as_bytes())
                    .expect("verified image symbol owner");
                definitions.insert(name.to_owned(), DefinitionOwner::Scoop(owner.owner()));
            }
            final_images.push(image);
            if let Some(entry) = artifact.production().entry_symbol {
                if cone != closure.root() || root.is_some() {
                    return Err(error(format!("multiple executable entries at Cone {cone}")));
                }
                root = Some(object_symbol(entry, profile));
                windows_subsystem = artifact
                    .manifest()
                    .cone()
                    .windows_subsystem()
                    .unwrap_or_default();
            }
            for object in symbols.members() {
                let member = object.member();
                if !selected.contains(cone, member) {
                    continue;
                }
                let directory = artifact.manifest().members();
                let position = directory
                    .binary_search_by_key(&member, |record| record.id())
                    .map_err(|_| error(format!("missing object member {member} in Cone {cone}")))?;
                let record = &directory[position];
                objects.push(InputObject {
                    origin: ObjectOrigin::Cone { cone, member },
                    bytes: ObjectBytes::Artifact { artifact, member },
                    digest: record.sha256(),
                });
            }
            for definition in symbols.defined_symbols().owners() {
                if !selected.contains(cone, definition.member()) {
                    continue;
                }
                let name = String::from_utf8(definition.symbol().to_vec()).map_err(error)?;
                let owner = DefinitionOwner::Scoop(definition.owner());
                if let Some(previous) = definitions.insert(name.clone(), owner) {
                    return Err(error(format!(
                        "multiple or conflicting selected owners for {name}: {previous:?}, {owner:?}; Cone {cone}"
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
            for requirement in symbols.requirements() {
                if !selected.contains(cone, requirement.member()) {
                    continue;
                }
                let symbol = String::from_utf8(requirement.symbol().to_vec()).map_err(error)?;
                requirement_origins
                    .entry(symbol.clone())
                    .or_default()
                    .push(format!(
                        "Cone {} member {}",
                        artifact.manifest().cone().coordinate(),
                        requirement.member()
                    ));
                requirements.insert(symbol);
            }
        }
        for object in runtime.objects() {
            for symbol in object.info().definitions.keys() {
                match definitions.entry(symbol.clone()) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(DefinitionOwner::Runtime(object.id()));
                    }
                    std::collections::btree_map::Entry::Occupied(entry) => {
                        // RuntimeObjectSet already resolved coalescible definitions.
                        if !matches!(entry.get(), DefinitionOwner::Runtime(_)) {
                            return Err(error(format!(
                                "runtime object {} conflicts with {:?} at {symbol}",
                                object.id(),
                                entry.get()
                            )));
                        }
                    }
                }
            }
            requirements.extend(object.info().requirements.iter().cloned());
            for symbol in &object.info().requirements {
                requirement_origins
                    .entry(symbol.clone())
                    .or_default()
                    .push(format!("runtime object {}", object.id()));
            }
            objects.push(InputObject {
                origin: ObjectOrigin::Runtime(object.id()),
                bytes: ObjectBytes::File {
                    path: object.path().to_owned(),
                },
                digest: object.digest(),
            });
        }
        let root = root.ok_or_else(|| error("executable closure has no root entry"))?;
        let string_target = string_target
            .ok_or_else(|| error("executable closure has no typed runtime String alias"))?;
        requirements.extend([
            root.clone(),
            profile
                .target()
                .contract()
                .native_symbol_normalization()
                .compiler_generated_object_symbol(
                    if profile.target().native_object_format()
                        == scoop_lir::NativeObjectFormat::Coff64
                    {
                        "scoop_rt_run_program_utf16"
                    } else {
                        "scoop_rt_run_program"
                    },
                ),
        ]);
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
            requirement_origins,
            namespace,
            target: profile.target(),
            images,
            final_images,
            selected,
            root,
            string_target,
            native,
            windows_subsystem,
        };
        native::resolve(
            closure,
            &declarations,
            runtime,
            profile,
            library_paths,
            &mut result,
        )?;
        Ok(result)
    }

    pub fn symbol(&self, logical: &str) -> String {
        self.target
            .contract()
            .native_symbol_normalization()
            .compiler_generated_object_symbol(logical)
    }

    pub fn compiler_owned(&self, symbol: &str) -> bool {
        symbol == self.symbol("main")
            || (self.target.native_object_format() == scoop_lir::NativeObjectFormat::Coff64
                && symbol == "wmain")
            || symbol == self.symbol("scoop_td_String")
            || symbol.starts_with(&self.symbol("scoop$"))
    }

    pub fn linker_defined(&self, symbol: &str) -> bool {
        symbol == self.symbol("scoop_td_String")
            || symbol == self.symbol("__dso_handle")
            || (self.target.id() != scoop_target_data::DARWIN_AARCH64_ID
                && matches!(
                    symbol,
                    "_GLOBAL_OFFSET_TABLE_" | "__scoop_stackmaps_start" | "__scoop_stackmaps_end"
                ))
    }
}

fn object_symbol(request: PersistentSymbolRequest, profile: &ValidatedFinalLinkProfile) -> String {
    profile
        .target()
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(request.symbol().as_str())
}
