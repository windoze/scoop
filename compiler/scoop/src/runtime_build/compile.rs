use super::*;
use scoop_process::CommandExt;

pub(super) fn compile(
    request: &RuntimeBuildRequest<'_>,
    inputs: &inputs::Inputs,
    staging: &Path,
) -> Result<(Vec<Vec<u8>>, dependencies::Dependencies), RuntimeBuildError> {
    let root = staging.join("runtime");
    inputs.materialize(&root)?;
    let root = std::fs::canonicalize(root).map_err(error)?;
    let mut objects = Vec::with_capacity(inputs.sources.len());
    let mut files = Vec::new();
    for (index, source) in inputs.sources.iter().enumerate() {
        let object = staging.join(format!("runtime-{index}.o"));
        let depfile = staging.join(format!("runtime-{index}.d"));
        let mut command = request
            .target
            .c_bridge_toolchain()
            .object_compilation_command(&root.join(source), &object);
        command
            .args(request.target.runtime_build().runtime_c_flags())
            .arg(match request.optimization {
                RuntimeOptimization::None => "-O0",
                RuntimeOptimization::Optimized => "-O2",
            })
            .args([
                "-funwind-tables",
                "-fasynchronous-unwind-tables",
                "-fno-lto",
            ])
            .arg(format!("-ffile-prefix-map={}=runtime", root.display()))
            .arg(format!("-fmacro-prefix-map={}=runtime", root.display()))
            .args(["-MD", "-MT", "runtime.o", "-MF"])
            .arg(&depfile);
        for include in request.target.runtime_build().include_directories() {
            command.arg("-I").arg(root.join(include));
        }
        let output = command.scoop_output().map_err(error)?;
        if !output.status.success() {
            return Err(error(format!(
                "{source}: C compiler failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        objects.push(std::fs::read(object).map_err(error)?);
        files.push(std::fs::read_to_string(depfile).ok());
    }
    let dependencies = dependencies::Dependencies::collect(&root, files);
    Ok((objects, dependencies))
}
