use glob::{MatchOptions, Pattern};
use scoop_hir::{CompileTimeDirectoryError as Error, CompileTimeDirectoryFilter};

pub(super) struct Patterns {
    include: Vec<Pattern>,
    exclude: Vec<Pattern>,
}

impl Patterns {
    pub fn new(filter: &CompileTimeDirectoryFilter) -> Result<Self, Error> {
        let include = filter
            .include
            .iter()
            .enumerate()
            .map(|(index, pattern)| {
                compile(pattern).map_err(|message| Error::IncludePattern { index, message })
            })
            .collect::<Result<_, _>>()?;
        let exclude = filter
            .exclude
            .iter()
            .enumerate()
            .map(|(index, pattern)| {
                compile(pattern).map_err(|message| Error::ExcludePattern { index, message })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { include, exclude })
    }

    pub fn matches(&self, path: &str) -> bool {
        let options = MatchOptions {
            case_sensitive: true,
            require_literal_separator: true,
            require_literal_leading_dot: false,
        };
        self.include
            .iter()
            .any(|pattern| pattern.matches_with(path, options))
            && !self
                .exclude
                .iter()
                .any(|pattern| pattern.matches_with(path, options))
    }
}

fn compile(pattern: &str) -> Result<Pattern, String> {
    let invalid_path = pattern.is_empty()
        || pattern.contains(['\\', '\0'])
        || pattern
            .split('/')
            .any(|part| matches!(part, "" | "." | ".."))
        || (pattern.as_bytes().get(1) == Some(&b':')
            && pattern.as_bytes()[0].is_ascii_alphabetic());
    if invalid_path {
        return Err(format!(
            "invalid includeDir glob `{pattern}`: expected a nonempty relative slash-separated path without backslashes, NUL, empty, . or .. components"
        ));
    }
    Pattern::new(pattern).map_err(|error| format!("invalid includeDir glob `{pattern}`: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_globs_use_unicode_components_and_explicit_recursive_matching() {
        for (pattern, path, matches) in [
            ("**", "file", true),
            ("**", ".hidden/file", true),
            ("**/*.png", "root.png", true),
            ("**/*.png", "deep/a.png", true),
            ("*.png", "deep/a.png", false),
            ("icons/**", "icons/deep/a.png", true),
            ("?.txt", "雪.txt", true),
            ("?.txt", "ab.txt", false),
            ("[a-z].txt", "c.txt", true),
            ("[!a-z].txt", "雪.txt", true),
            ("[雨-雷].txt", "雪.txt", true),
            ("A.txt", "a.txt", false),
            ("[*][?].txt", "*?.txt", true),
            ("{a,b}", "{a,b}", true),
        ] {
            let patterns = Patterns::new(&CompileTimeDirectoryFilter {
                include: vec![pattern.into()],
                exclude: vec![],
            })
            .unwrap();
            assert_eq!(patterns.matches(path), matches, "{pattern} against {path}");
        }
    }

    #[test]
    fn directory_globs_reject_invalid_forms_and_locate_the_pattern() {
        for pattern in [
            "", "/root", "C:root", "C:/root", "a\\b", "a\0b", "a//b", "./a", "../a", "a/../b",
            "a/", "[abc", "a**", "**a", "***",
        ] {
            let error = Patterns::new(&CompileTimeDirectoryFilter {
                include: vec!["**".into()],
                exclude: vec!["ok".into(), pattern.into()],
            })
            .err()
            .unwrap();
            assert!(
                matches!(error, Error::ExcludePattern { index: 1, .. }),
                "{pattern}: {error:?}"
            );
        }
        let patterns = Patterns::new(&CompileTimeDirectoryFilter {
            include: vec!["**".into(), "**/*.txt".into()],
            exclude: vec!["draft/**".into()],
        })
        .unwrap();
        assert!(patterns.matches("root.txt"));
        assert!(!patterns.matches("draft/root.txt"));
    }
}
