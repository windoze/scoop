use std::path::PathBuf;

/// Read one GCC/Clang depfile with an explicitly selected, unescaped target.
pub fn parse_make_dependencies(input: &str, target: &str) -> Option<Vec<PathBuf>> {
    let input = input.strip_prefix(target)?.strip_prefix(':')?;
    let mut paths = Vec::new();
    let mut word = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => match chars.next()? {
                '\n' => {}
                '\r' if chars.peek() == Some(&'\n') => {
                    chars.next();
                }
                escaped => word.push(escaped),
            },
            '$' if chars.peek() == Some(&'$') => {
                chars.next();
                word.push('$');
            }
            ch if ch.is_whitespace() => {
                if !word.is_empty() {
                    paths.push(PathBuf::from(std::mem::take(&mut word)));
                }
            }
            ch => word.push(ch),
        }
    }
    if !word.is_empty() {
        paths.push(PathBuf::from(word));
    }
    (!paths.is_empty()).then_some(paths)
}
