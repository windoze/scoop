use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

pub(crate) fn write(path: &Path, script: &[u8]) {
    // Open the executable for writing only in this child. Parallel test spawns
    // cannot inherit its writable descriptor and cause ETXTBSY on Linux.
    let mut writer = Command::new("/bin/sh")
        .args(["-c", "/bin/cat > \"$1\"", "write-test-compiler"])
        .arg(path)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    writer.stdin.take().unwrap().write_all(script).unwrap();
    assert!(writer.wait().unwrap().success());
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}
