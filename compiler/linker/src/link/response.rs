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
