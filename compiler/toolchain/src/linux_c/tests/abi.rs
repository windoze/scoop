use super::*;

#[test]
fn gcc_and_clang_interoperate_for_sysv_signatures_in_both_directions() {
    let target = scoop_target_data::LINUX_X86_64_GNU;
    let gcc = resolve_linux_c_toolchain(target, Some(Path::new("gcc")), None).unwrap();
    let clang = resolve_linux_c_toolchain(target, Some(Path::new("clang")), None).unwrap();
    for optimization in [
        scoop_lir::OptimizationMode::Debug,
        scoop_lir::OptimizationMode::Release,
    ] {
        for (caller, callee) in [(&gcc, &clang), (&clang, &gcc)] {
            let caller = caller.clone().with_optimization(optimization);
            let callee = callee.clone().with_optimization(optimization);
            let directory = tempfile::tempdir().unwrap();
            let header = directory.path().join("abi.h");
            std::fs::write(header, HEADER).unwrap();
            let mut objects = Vec::new();
            for (name, body, compiler) in [("caller", CALLER, &caller), ("callee", CALLEE, &callee)]
            {
                let source = directory.path().join(format!("{name}.c"));
                let object = directory.path().join(format!("{name}.o"));
                std::fs::write(&source, body).unwrap();
                compiler.compile_object(&source, &object).unwrap();
                objects.push(object);
            }
            let executable = directory.path().join("abi");
            run(caller
                .driver_command()
                .args(objects)
                .arg("-o")
                .arg(&executable))
            .unwrap();
            run(&mut Command::new(executable)).unwrap();
        }
    }
}

const HEADER: &str = r#"
#include <stdint.h>
typedef struct { double d; int64_t i; } Mixed;
typedef struct { double x, y; } Floats;
typedef struct { int64_t x, y, z; } Large;
int8_t narrow(int8_t value, _Bool flag);
Mixed mixed(Mixed value);
Floats floats(Floats value);
Large large(Large value);
Mixed exhausted(int64_t a, int64_t b, int64_t c, int64_t d, int64_t e, int64_t f, Mixed value);
Mixed callback(Mixed (*function)(Mixed), Mixed value);
"#;

const CALLEE: &str = r#"
#include "abi.h"
int8_t narrow(int8_t value, _Bool flag) { return flag ? value : 0; }
Mixed mixed(Mixed value) { return (Mixed){value.d + 1.5, value.i + 7}; }
Floats floats(Floats value) { return (Floats){value.y + 2, value.x + 3}; }
Large large(Large value) { return (Large){value.z + 1, value.x + 2, value.y + 3}; }
Mixed exhausted(int64_t a, int64_t b, int64_t c, int64_t d, int64_t e, int64_t f, Mixed value) {
    return (Mixed){value.d, value.i + a + b + c + d + e + f};
}
Mixed callback(Mixed (*function)(Mixed), Mixed value) { return function(value); }
"#;

const CALLER: &str = r#"
#include "abi.h"
static Mixed local(Mixed value) { return (Mixed){value.d + 4, value.i + 5}; }
int main(void) {
    Mixed m = mixed((Mixed){2.5, 11});
    Floats f = floats((Floats){1.5, 2.5});
    Large l = large((Large){10, 20, 30});
    Mixed e = exhausted(1, 2, 3, 4, 5, 6, (Mixed){9.5, 100});
    Mixed c = callback(local, (Mixed){3.5, 17});
    return narrow(-109, 1) != -109 || narrow(-109, 0) != 0 ||
        m.d != 4 || m.i != 18 || f.x != 4.5 || f.y != 4.5 ||
        l.x != 31 || l.y != 12 || l.z != 23 || e.d != 9.5 || e.i != 121 ||
        c.d != 7.5 || c.i != 22;
}
"#;
