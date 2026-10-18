"""Fixed CMake inputs shared by compression preparation and packaging."""

PACKAGES = (
    {
        "name": "zstd",
        "version": "1.5.7",
        "source": "build/cmake",
        "targets": ["libzstd_static"],
        "libraries": ["libzstd.a"],
        "header": "zstd.h",
        "options": {
            "ZSTD_BUILD_SHARED": "OFF",
            "ZSTD_BUILD_STATIC": "ON",
            "ZSTD_BUILD_PROGRAMS": "OFF",
            "ZSTD_BUILD_TESTS": "OFF",
            "ZSTD_BUILD_CONTRIB": "OFF",
            "ZSTD_MULTITHREAD_SUPPORT": "OFF",
            "ZSTD_LEGACY_SUPPORT": "OFF",
        },
    },
    {
        "name": "zlib-ng",
        "version": "2.3.3",
        "source": ".",
        "targets": ["zlib-ng"],
        "libraries": ["libz-ng.a"],
        "header": "zlib-ng.h",
        "options": {
            "ZLIB_COMPAT": "OFF",
            "WITH_GZFILEOP": "OFF",
            "WITH_NATIVE_INSTRUCTIONS": "OFF",
            "WITH_RUNTIME_CPU_DETECTION": "ON",
            "WITH_INFLATE_STRICT": "ON",
        },
    },
    {
        "name": "brotli",
        "version": "1.2.0",
        "source": ".",
        "targets": ["brotlicommon", "brotlidec", "brotlienc"],
        "libraries": ["libbrotlicommon.a", "libbrotlidec.a", "libbrotlienc.a"],
        "header": "brotli/encode.h",
        "options": {
            "BROTLI_BUILD_TOOLS": "OFF",
            "BROTLI_DISABLE_TESTS": "ON",
            "BROTLI_BUILD_FOR_PACKAGE": "OFF",
        },
    },
)
