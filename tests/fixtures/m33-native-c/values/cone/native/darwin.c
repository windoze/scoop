#if !defined(SCOOP_TARGET_OS_DARWIN) || !defined(SCOOP_TARGET_ARCH_AARCH64) || \
    !defined(SCOOP_TARGET_ENV_NONE) || !defined(__APPLE__)
#error incorrect target macros
#endif
int m33_native_platform(void) { return 1; }
