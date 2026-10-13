use super::*;
use std::process::Command;

#[test]
fn many_object_inputs_reach_the_linker_without_driver_expansion() {
    let directory = tempfile::tempdir().unwrap();
    let quoted = directory
        .path()
        .join("objects, spaces 'single' \"double\" \\");
    std::fs::create_dir(&quoted).unwrap();
    let source = quoted.join("main.c");
    let main = quoted.join("main.o");
    let empty = quoted.join("empty.o");
    std::fs::write(&source, "int main(void) { return 0; }\n").unwrap();
    checked(
        Command::new("gcc")
            .arg("-c")
            .arg(&source)
            .arg("-o")
            .arg(&main),
    );
    std::fs::write(&source, "\n").unwrap();
    checked(
        Command::new("gcc")
            .arg("-c")
            .arg(&source)
            .arg("-o")
            .arg(&empty),
    );
    let limit = checked(Command::new("getconf").arg("ARG_MAX"));
    let limit: usize = std::str::from_utf8(&limit.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let count = limit / empty.as_os_str().as_encoded_bytes().len() + 100;
    let mut paths = vec![main];
    paths.extend(std::iter::repeat_n(empty.clone(), count));
    for driver in ["gcc", "g++", "musl-gcc"] {
        let scratch = quoted.join(driver);
        std::fs::create_dir(&scratch).unwrap();
        let binary = scratch.join("program");
        let mut command = Command::new(driver);
        command.arg("-Wl,-t").arg("-o").arg(&binary);
        append_objects(&mut command, &scratch, &paths).unwrap();
        let output = checked(&mut command);
        let trace = std::str::from_utf8(&output.stdout).unwrap();
        assert_eq!(
            trace
                .lines()
                .filter(|line| *line == empty.to_str().unwrap())
                .count(),
            count
        );
        checked(&mut Command::new(binary));
    }
}

fn checked(command: &mut Command) -> std::process::Output {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}
