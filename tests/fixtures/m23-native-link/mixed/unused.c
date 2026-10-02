extern int missing_provider(void);
__attribute__((constructor)) static void unused_initializer(void) {}
int m23_unused(void) { return missing_provider(); }
