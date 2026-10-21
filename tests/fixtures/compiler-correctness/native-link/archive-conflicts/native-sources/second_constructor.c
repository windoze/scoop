static int initialized;

__attribute__((constructor)) static void setup(void) {
    initialized = 1;
}

int helper(void) {
    return initialized;
}
