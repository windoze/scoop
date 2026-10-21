extern int m23_missing_unused(void);

__attribute__((constructor)) static void unused_constructor(void) {
    (void)m23_missing_unused();
}

#if defined(__APPLE__)
__asm__(".linker_option \"-lnot_a_link_input\"");
#endif

int m23_unused(void) {
    return m23_missing_unused();
}
