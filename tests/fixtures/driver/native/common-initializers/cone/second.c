int counter;
int override;
__attribute__((weak)) int common_over_weak = 100;
int second(void) { return counter + override + common_over_weak; }
