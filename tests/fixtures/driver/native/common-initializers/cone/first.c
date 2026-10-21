int counter;
__attribute__((weak)) int override = 99;
int common_over_weak;
__attribute__((constructor)) static void initialize(void) { counter += 2; }
int first(void) { return ++counter; }
