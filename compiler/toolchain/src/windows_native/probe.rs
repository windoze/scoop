use scoop_lir::TargetProfileId;
use scoop_manifest::NativeSourceLanguage;

pub(super) fn source(target: TargetProfileId, language: NativeSourceLanguage) -> String {
    let microsoft = target == scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID;
    let ucrt = target != scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_ID;
    let cxx = language == NativeSourceLanguage::Cxx;
    let mut source = String::from(
        r#"
#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <windows.h>
#if !defined(_WIN64) || (!defined(_M_X64) && !defined(__x86_64__))
#error Win64 compiler required
#endif
#ifdef __cplusplus
#define SCOOP_ASSERT static_assert
extern "C" {
#else
#define SCOOP_ASSERT _Static_assert
#endif
SCOOP_ASSERT(sizeof(void *) == 8 && sizeof(long) == 4 && sizeof(wchar_t) == 2, "Windows LLP64 required");
#ifdef __cplusplus
}
#endif
"#,
    );
    source.push_str(&format!("\n#if defined(__MINGW32__) == {}\n#error wrong Windows native environment\n#endif\n#if defined(_UCRT) != {}\n#error wrong Windows CRT headers\n#endif\n", u8::from(microsoft), u8::from(ucrt)));
    if microsoft {
        source.push_str("#if !defined(_DLL) || !defined(_MT) || defined(_DEBUG)\n#error release dynamic Microsoft CRT required\n#endif\n");
    } else {
        source.push_str("#ifndef __clang__\n#error LLVM-MinGW requires Clang\n#endif\n");
    }
    if cxx {
        source.push_str(
            r#"
#include <string>
#include <stdexcept>
#include <typeinfo>
struct Base { virtual ~Base() = default; };
struct Derived : Base {};
extern "C" int scoop_native_probe(Base *value) {
    try {
        if (dynamic_cast<Derived *>(value)) throw std::runtime_error("native");
        return int(std::string(typeid(*value).name()).size());
    } catch (const std::exception &error) { return int(std::string(error.what()).size()); }
}
"#,
        );
        source.push_str(if microsoft {
            "#ifndef _MSVC_STL_VERSION\n#error Microsoft STL required\n#endif\n#if _ITERATOR_DEBUG_LEVEL != 0\n#error release Microsoft STL required\n#endif\n#define SCOOP_LIBRARY_VERSION _MSVC_STL_VERSION\n"
        } else {
            "#ifndef _LIBCPP_VERSION\n#error libc++ required\n#endif\n#define SCOOP_LIBRARY_VERSION _LIBCPP_VERSION\n"
        });
    } else {
        source.push_str("#define SCOOP_LIBRARY_VERSION 0\n");
    }
    source.push_str(
        r#"
#ifdef __cplusplus
extern "C"
#endif
uint32_t scoop_native_configuration[4] = {
#ifdef __clang__
    __clang_major__, __clang_minor__, __clang_patchlevel__,
#else
    _MSC_VER, _MSC_FULL_VER, _MSC_BUILD,
#endif
    SCOOP_LIBRARY_VERSION
};
"#,
    );
    source
}
