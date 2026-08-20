use crate::ConeRelativePath;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeCompileFlag {
    Argument(String),
    Include {
        kind: NativeIncludeFlag,
        path: ConeRelativePath,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
