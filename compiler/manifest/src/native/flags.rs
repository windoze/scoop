use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NativeCompileFlag {
    Argument(String),
    Include {
        kind: NativeIncludeFlag,
        path: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NativeIncludeFlag {
    Search,
    Quote,
    System,
    After,
    ForcedHeader,
    Macros,
}

impl NativeIncludeFlag {
    pub const fn argument(self) -> &'static str {
        match self {
            Self::Search => "-I",
            Self::Quote => "-iquote",
            Self::System => "-isystem",
            Self::After => "-idirafter",
            Self::ForcedHeader => "-include",
            Self::Macros => "-imacros",
        }
    }

    pub(crate) fn split(argument: &str) -> Option<(Self, &str)> {
        if let Some(path) = argument.strip_prefix("/I") {
            return Some((Self::Search, path));
        }
        if let Some(path) = argument
            .strip_prefix("/FI")
            .or_else(|| argument.strip_prefix("-FI"))
        {
            return Some((Self::ForcedHeader, path));
        }
        [
            Self::Search,
            Self::Quote,
            Self::System,
            Self::After,
            Self::ForcedHeader,
            Self::Macros,
        ]
        .into_iter()
        .find_map(|kind| {
            argument
                .strip_prefix(kind.argument())
                .map(|path| (kind, path))
        })
    }
}

pub(super) fn driver_managed(argument: &str) -> bool {
    if let Some(msvc) = argument.strip_prefix('-')
        && driver_managed_microsoft(&format!("/{msvc}"))
    {
        return true;
    }
    driver_managed_microsoft(argument) || driver_managed_gnu(argument)
}

fn driver_managed_microsoft(argument: &str) -> bool {
    if argument.starts_with('/') {
        return [
            "/c",
            "/E",
            "/EP",
            "/P",
            "/X",
            "/TC",
            "/TP",
            "/MD",
            "/MDd",
            "/MT",
            "/MTd",
            "/LD",
            "/LDd",
            "/Zs",
            "/MP",
            "/link",
            "/Zi",
            "/ZI",
            "/showIncludes",
        ]
        .contains(&argument)
            || [
                "/Fo",
                "/Fe",
                "/Fd",
                "/Fi",
                "/Fa",
                "/FA",
                "/Fm",
                "/Fp",
                "/FR",
                "/Fr",
                "/Tc",
                "/Tp",
                "/Y",
                "/GL",
                "/sourceDependencies",
                "/scanDependencies",
                "/ifc",
                "/reference",
                "/headerUnit",
                "/translateInclude",
                "/clang:",
                "/external:I",
                "/external:env:",
                "/machine:",
            ]
            .iter()
            .any(|prefix| argument.starts_with(prefix));
    }
    false
}

fn driver_managed_gnu(argument: &str) -> bool {
    argument.starts_with('@')
        || [
            "-c",
            "-S",
            "-E",
            "-fsyntax-only",
            "-include-pch",
            "-include-pth",
            "-emit-llvm",
            "-Xclang",
            "-Xpreprocessor",
            "-Xassembler",
            "-shared",
            "-static",
            "-r",
            "-arch",
            "-m32",
            "-m64",
            "-mx32",
            "-M",
            "-MM",
            "-MD",
            "-MMD",
            "-MP",
            "-MG",
        ]
        .contains(&argument)
        || [
            "-o",
            "--output",
            "-x",
            "-target",
            "--target",
            "-isysroot",
            "--sysroot",
            "-MF",
            "-MT",
            "-MQ",
            "-MJ",
            "-B",
            "-fplugin",
            "-specs",
            "--specs",
            "-wrapper",
            "-Wl,",
            "-Xlinker",
            "-l",
            "-L",
            "-stdlib",
            "--gcc-toolchain",
            "-gcc-toolchain",
            "-resource-dir",
            "-fuse-ld",
            "-flto",
            "-fmodule",
            "-fpch",
            "-fpreprocessed",
            "-fdirectives-only",
            "-frewrite-includes",
            "-save-temps",
            "-ftime-trace",
            "-serialize-diagnostics",
            "-fprofile",
            "-Wp,",
            "-Wa,",
        ]
        .iter()
        .any(|prefix| argument.starts_with(prefix))
}
