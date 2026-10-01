#include "startup.h"

SCOOP_FIXTURE_IMAGES

static int run(void) {
    return scoop_rt_run_program(
        fixture_images, sizeof fixture_images / sizeof *fixture_images, &fixture_root);
}
void m23_reenter(void) { (void)run(); }
int main(void) {
    int result = run();
    if (SCOOP_FIXTURE_REPEAT) {
        result = run();
    }
    return result;
}
