use super::*;
use std::collections::BTreeMap;

use scoop_wire::{Encoder, WireEncode, domain_separated_cbor_hash, sha256};

pub(super) struct Inputs {
    pub files: BTreeMap<String, Vec<u8>>,
    pub sources: Vec<String>,
    pub flags: Vec<String>,
    pub compiler_digest: Digest256,
    pub base_key: Digest256,
    pub input_paths: Vec<PathBuf>,
    pub unwind_include: Option<&'static str>,
}

impl Inputs {
    pub fn read(request: &RuntimeBuildRequest<'_>) -> Result<Self, RuntimeBuildError> {
        let root = std::fs::canonicalize(request.runtime_root).map_err(error)?;
        let mut files = BTreeMap::new();
        let mut sources = Vec::new();
        for path in request.target.runtime_build().runtime_sources() {
            let relative = path
                .strip_prefix("runtime/")
                .ok_or_else(|| error(format!("invalid runtime source rule {path}")))?;
            files.insert(
                relative.to_owned(),
                std::fs::read(root.join(relative)).map_err(error)?,
            );
            sources.push(relative.to_owned());
        }
        let include_dirs = request.target.runtime_build().include_directories();
        for (index, path) in include_dirs.iter().enumerate() {
            if include_dirs[..index]
                .iter()
                .any(|parent| Path::new(path).starts_with(parent))
            {
                continue;
            }
            read_headers(&root, Path::new(path), *path == "include", &mut files)?;
        }
        let mut input_paths: Vec<_> = files
            .keys()
            .map(|path| request.runtime_root.join(path))
            .collect();
        let unwind =
            scoop_toolchain::runtime_unwind_include(request.target.id(), request.unwind_prefix)
                .map_err(error)?;
        if let Some(include) = &unwind {
            let mut headers = BTreeMap::new();
            read_headers(include, Path::new(""), true, &mut headers)?;
            input_paths.extend(headers.keys().map(|path| include.join(path)));
            files.extend(
                headers
                    .into_iter()
                    .map(|(path, bytes)| (format!(".scoop-unwind/{path}"), bytes)),
            );
        }
        let invocation = request.target.c_bridge_toolchain();
        let compiler_digest = sha256(&std::fs::read(invocation.compiler_driver()).map_err(error)?);
        let mut flags: Vec<String> = request
            .target
            .runtime_build()
            .runtime_c_flags()
            .iter()
            .map(|flag| (*flag).to_owned())
            .collect();
        flags.extend(
            request
                .optimization
                .c_flags()
                .iter()
                .map(|flag| (*flag).to_owned()),
        );
        flags.extend(
            [
                "-funwind-tables",
                "-fasynchronous-unwind-tables",
                "-fno-lto",
                "-ffile-prefix-map=<runtime>=runtime",
                "-fmacro-prefix-map=<runtime>=runtime",
                "-MD",
                "-MT",
                "runtime.o",
                "-MF",
                "<depfile>",
            ]
            .into_iter()
            .map(str::to_owned),
        );
        flags.extend(include_dirs.iter().map(|path| format!("-I{path}")));
        let unwind_include = unwind.as_ref().map(|_| ".scoop-unwind");
        if let Some(include) = unwind_include {
            flags.push(format!("-I{include}"));
        }
        let toolchain = encode(invocation.profile().contract()).map_err(error)?;
        let base_key = domain_separated_cbor_hash(
            "scoop-runtime-build-inputs-v1",
            &InputKey {
                files: &files,
                sources: &sources,
                flags: &flags,
                compiler_digest,
                toolchain: &toolchain,
            },
        )
        .map_err(error)?;
        Ok(Self {
            files,
            sources,
            flags,
            compiler_digest,
            base_key,
            input_paths,
            unwind_include,
        })
    }

    pub fn materialize(&self, root: &Path) -> Result<(), RuntimeBuildError> {
        for (relative, bytes) in &self.files {
            let path = root.join(relative);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(error)?;
            }
            std::fs::write(path, bytes).map_err(error)?;
        }
        Ok(())
    }
}

fn read_headers(
    root: &Path,
    relative: &Path,
    all_files: bool,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), RuntimeBuildError> {
    for entry in std::fs::read_dir(root.join(relative)).map_err(error)? {
        let entry = entry.map_err(error)?;
        let path = relative.join(entry.file_name());
        if entry.file_type().map_err(error)?.is_dir() {
            read_headers(root, &path, all_files, files)?;
        } else if all_files
            || !matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("c" | "S")
            )
        {
            let name = path
                .to_str()
                .ok_or_else(|| error("runtime header path is not UTF-8"))?;
            files.insert(
                name.to_owned(),
                std::fs::read(root.join(&path)).map_err(error)?,
            );
        }
    }
    Ok(())
}

struct InputKey<'a> {
    files: &'a BTreeMap<String, Vec<u8>>,
    sources: &'a [String],
    flags: &'a [String],
    compiler_digest: Digest256,
    toolchain: &'a [u8],
}
impl WireEncode for InputKey<'_> {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(6)?;
        e.field(1)?;
        e.bytes(self.toolchain)?;
        e.field(2)?;
        self.compiler_digest.encode(e)?;
        e.field(3)?;
        scoop_lir::RuntimeAbiContract.encode(e)?;
        e.field(4)?;
        e.array(self.flags.len() as u64)?;
        for flag in self.flags {
            e.text(flag)?;
        }
        e.field(5)?;
        e.array(self.sources.len() as u64)?;
        for source in self.sources {
            e.text(source)?;
        }
        e.field(6)?;
        e.array(self.files.len() as u64)?;
        for (path, bytes) in self.files {
            e.array(2)?;
            e.text(path)?;
            sha256(bytes).encode(e)?;
        }
        Ok(())
    }
}
