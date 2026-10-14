use super::*;
use scoop_process::CommandExt;
use std::collections::BTreeSet;
use std::process::Command;

pub(super) fn compile(
    request: &RuntimeBuildRequest<'_>,
    inputs: &inputs::Inputs,
    staging: &Path,
) -> Result<(Vec<PathBuf>, dependencies::Dependencies), RuntimeBuildError> {
    let mut objects = Vec::with_capacity(inputs.sources.len());
    let mut observed = BTreeSet::new();
    let temporary = tempfile::tempdir_in(staging).map_err(error)?;
    for (index, source) in inputs.sources.iter().enumerate() {
        let object = staging.join(format!("runtime-{index}.o"));
        let depfile = temporary.path().join("unit.d");
        let assembly = source.ends_with(".S");
        let preprocessed = temporary
            .path()
            .join(if assembly { "unit.s" } else { "unit.i" });
        let mut preprocess = request
            .target
            .c_bridge_toolchain()
            .object_compilation_command(&inputs.root.join(source), &preprocessed);
        configure(&mut preprocess, inputs);
        preprocess
            .args(["-E", "-dI", "-v", "-MD", "-MT", "runtime.o", "-MF"])
            .arg(&depfile);
        for include in &inputs.includes {
            preprocess.arg("-I").arg(include);
        }
        let output = run(&mut preprocess, source)?;
        let mut search = inputs.includes.clone();
        search.extend(scoop_toolchain::native_include_search_paths(&output.stderr));
        let bytes = scoop_toolchain::strip_native_include_observations(
            &std::fs::read(&preprocessed).map_err(error)?,
            &inputs.root,
            &search,
            &mut observed,
        )
        .map_err(error)?;
        std::fs::write(&preprocessed, bytes).map_err(error)?;
        let dependencies = std::fs::read_to_string(&depfile).map_err(error)?;
        let paths = scoop_process::parse_make_dependencies(&dependencies, "runtime.o")
            .ok_or_else(|| error("C compiler returned an invalid runtime dependency file"))?;
        observed.extend(paths.into_iter().map(|p| inputs.root.join(p)));
        let mut compile = request
            .target
            .c_bridge_toolchain()
            .object_compilation_command(&preprocessed, &object);
        configure(&mut compile, inputs);
        run(&mut compile, source)?;
        objects.push(object);
    }
    Ok((objects, dependencies::Dependencies::collect(observed)?))
}

fn configure(command: &mut Command, inputs: &inputs::Inputs) {
    command
        .current_dir(&inputs.root)
        .args(&inputs.flags)
        .arg(format!(
            "-ffile-prefix-map={}=runtime",
            inputs.root.display()
        ))
        .arg(format!(
            "-fmacro-prefix-map={}=runtime",
            inputs.root.display()
        ));
}

fn run(command: &mut Command, source: &str) -> Result<std::process::Output, RuntimeBuildError> {
    let output = command.scoop_output().map_err(error)?;
    if !output.status.success() {
        return Err(error(format!(
            "{source}: C compiler failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(output)
}
