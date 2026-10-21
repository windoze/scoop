#if !defined(SCOOP_TARGET_OS_LINUX) || !defined(SCOOP_TARGET_ARCH_X86_64) ||   \
    !defined(SCOOP_TARGET_ENV_GNU) || !defined(__linux__)
#error incorrect target macros
#endif
int m33_native_platform(void) { return 2; }
