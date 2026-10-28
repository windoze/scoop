//! Host environment observations shared by all HIR probes in one compilation.

use std::collections::BTreeMap;
use std::sync::Mutex;

use scoop_hir::{CompileTimeEnvironmentName, CompileTimeInputHost};

type EnvironmentValues = BTreeMap<CompileTimeEnvironmentName, Result<Option<String>, String>>;

#[derive(Default)]
pub struct CompileTimeInputs {
    environment: Mutex<EnvironmentValues>,
}

impl CompileTimeInputHost for CompileTimeInputs {
    fn environment(&self, name: &CompileTimeEnvironmentName) -> Result<Option<String>, String> {
        let mut values = self
            .environment
            .lock()
            .expect("environment input cache is not poisoned");
        values
            .entry(name.clone())
            .or_insert_with(|| match std::env::var(name.as_str()) {
                Ok(value) => Ok(Some(value)),
                Err(std::env::VarError::NotPresent) => Ok(None),
                Err(std::env::VarError::NotUnicode(_)) => Err(format!(
                    "compile-time environment variable `{}` is not valid Unicode",
                    name.as_str(),
                )),
            })
            .clone()
    }
}

impl CompileTimeInputs {
    pub fn environment_inputs(&self) -> Vec<scoop_protocol::EnvironmentInputV1> {
        self.environment
            .lock()
            .expect("environment input cache is not poisoned")
            .iter()
            .filter_map(|(name, value)| {
                value
                    .as_ref()
                    .ok()
                    .map(|value| scoop_protocol::EnvironmentInputV1 {
                        name: name.as_str().to_owned(),
                        value: value.clone(),
                    })
            })
            .collect()
    }
}

pub fn environment_inputs_current(inputs: &[scoop_protocol::EnvironmentInputV1]) -> bool {
    inputs.iter().all(|input| match std::env::var(&input.name) {
        Ok(value) => input.value.as_ref() == Some(&value),
        Err(std::env::VarError::NotPresent) => input.value.is_none(),
        Err(std::env::VarError::NotUnicode(_)) => false,
    })
}

#[cfg(test)]
mod tests;
