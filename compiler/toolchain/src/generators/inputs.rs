use super::*;
use scoop_protocol::GeneratorToolInputV1;

pub fn record_generator_input(
    name: &str,
    program: &Path,
    version: u32,
) -> Result<GeneratorToolInputV1, ToolchainError> {
    let metadata = program.metadata().map_err(|source| {
        error(format!(
            "cannot inspect generator {}: {source}",
            program.display()
        ))
    })?;
    let registration = GeneratorRegistrationV1::new(
        name.to_owned(),
        HostPathCarrier::from_path(program).map_err(error)?,
    )
    .map_err(error)?;
    Ok(GeneratorToolInputV1::new(
        registration,
        version,
        metadata.len(),
        metadata.modified().map_err(error)?,
    ))
}

impl GeneratorPrograms {
    pub fn inputs_current(
        &self,
        inputs: &[GeneratorToolInputV1],
        compiler: &Path,
    ) -> Result<bool, ToolchainError> {
        for input in inputs {
            let registration = input.registration();
            let Some(program) = self.program_for_compiler(registration.name(), compiler) else {
                return Ok(false);
            };
            if program != registration.program().to_path_buf().map_err(error)? {
                return Ok(false);
            }
            let metadata = match program.metadata() {
                Ok(metadata) => metadata,
                Err(source)
                    if matches!(
                        source.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                    ) =>
                {
                    return Ok(false);
                }
                Err(source) => {
                    return Err(error(format!(
                        "cannot inspect generator {}: {source}",
                        program.display()
                    )));
                }
            };
            if metadata.len() != input.byte_length()
                || metadata.modified().map_err(error)? != input.modified()
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_used_bindings_and_program_updates_invalidate_results() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let tool = root.join("tool");
        std::fs::write(&tool, "first").unwrap();
        let inputs = [record_generator_input("used", &tool, 1).unwrap()];
        let programs = |unused: &str| {
            GeneratorPrograms::from_cli(
                &["used=tool".into(), format!("unused={unused}").into()],
                root,
            )
            .unwrap()
        };
        let compiler = root.join("scoopc");
        assert!(
            programs("missing")
                .inputs_current(&inputs, &compiler)
                .unwrap()
        );
        assert!(
            programs("different-missing")
                .inputs_current(&inputs, &compiler)
                .unwrap()
        );
        assert!(
            !GeneratorPrograms::default()
                .inputs_current(&inputs, &compiler)
                .unwrap()
        );
        std::fs::write(&tool, "changed program").unwrap();
        assert!(
            !programs("missing")
                .inputs_current(&inputs, &compiler)
                .unwrap()
        );
        assert!(programs("missing").inputs_current(&[], &compiler).unwrap());
    }

    #[test]
    fn default_program_is_paired_with_the_selected_compiler() {
        let root = std::env::temp_dir().join("selected-toolchain");
        assert_eq!(
            GeneratorPrograms::default().program_for_compiler("scoop.codec", &root.join("scoopc")),
            Some(root.join("scoop-codec-generator"))
        );
    }
}
