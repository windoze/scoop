use super::*;

pub(super) const FILE_NAME: &str = "objects.rsp";

pub(super) fn write_objects(directory: &Path, paths: &[PathBuf]) -> Result<(), LinkError> {
    let mut contents = Vec::new();
    for path in paths {
        contents.push(b'"');
        for byte in path.as_os_str().as_encoded_bytes() {
            if matches!(byte, b'\\' | b'"') {
                contents.push(b'\\');
            }
            contents.push(*byte);
        }
        contents.extend_from_slice(b"\"\n");
    }
    write_new(&directory.join(FILE_NAME), &contents)
}

pub(super) fn write_pe_objects(
    directory: &Path,
    paths: &[PathBuf],
    microsoft: bool,
) -> Result<(), LinkError> {
    if !cfg!(windows) && !microsoft {
        return write_objects(directory, paths);
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let mut contents = vec![0xff, 0xfe];
        for path in paths {
            let argument: Vec<_> = path.as_os_str().encode_wide().collect();
            for unit in windows_quote(&argument) {
                contents.extend_from_slice(&unit.to_le_bytes());
            }
        }
        write_new(&directory.join(FILE_NAME), &contents)
    }
    #[cfg(not(windows))]
    {
        let contents: Vec<u8> = paths
            .iter()
            .flat_map(|path| windows_quote(path.as_os_str().as_encoded_bytes()))
            .collect();
        write_new(&directory.join(FILE_NAME), &contents)
    }
}

fn windows_quote<T: Copy + Eq + From<u8>>(argument: &[T]) -> Vec<T> {
    let slash = T::from(b'\\');
    let quote = T::from(b'"');
    let mut result = vec![quote];
    let mut slashes = 0;
    for value in argument {
        if *value == slash {
            slashes += 1;
            continue;
        }
        result.extend(std::iter::repeat_n(
            slash,
            if *value == quote {
                slashes * 2 + 1
            } else {
                slashes
            },
        ));
        result.push(*value);
        slashes = 0;
    }
    result.extend(std::iter::repeat_n(slash, slashes * 2));
    result.extend([quote, T::from(b'\n')]);
    result
}
