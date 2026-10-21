int m23_unused = 7;
extern int missing_provider(void);
int unused_helper(void) { return missing_provider(); }
__attribute__((constructor)) static void unused_initializer(void) {}
#if defined(__APPLE__)
__asm__(".linker_option \"-lUnused\"");
#endif
