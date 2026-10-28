int scoop_platform_operating_system(void) {
#if defined(_WIN32)
    return 2;
#elif defined(__APPLE__)
    return 0;
#else
    return 1;
#endif
}

int scoop_platform_architecture(void) {
#ifdef __aarch64__
    return 0;
#else
    return 1;
#endif
}
