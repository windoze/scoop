static int initialized;

__attribute__((constructor)) static void setup(void) {
    initialized = 1;
}

int m23_add(int x, int y) {
    return x + y + initialized;
}
