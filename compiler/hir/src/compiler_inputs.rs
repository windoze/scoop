//! Request-local host services for compile-time inputs. No host I/O lives in HIR.

/// A valid environment name that may be queried by a compiler intrinsic.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompileTimeEnvironmentName(String);

impl CompileTimeEnvironmentName {
    pub fn parse(name: &str) -> Result<Self, &'static str> {
        if !name.starts_with("SCOOP_") {
            return Err("compile-time environment names must start with `SCOOP_`");
        }
        if name.contains(['=', '\0']) {
            return Err("compile-time environment names must not contain `=` or NUL");
        }
        Ok(Self(name.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Supplied by the compiler or editor host; shared by candidate-local probes.
pub trait CompileTimeInputHost: Send + Sync {
    fn environment(&self, name: &CompileTimeEnvironmentName) -> Result<Option<String>, String>;
}
