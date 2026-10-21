#include <stdint.h>
#include <stdlib.h>

int32_t main_code(void) {
    const char *code = getenv("M33_MAIN_CODE");
    return code == NULL ? 37 : (int32_t)strtoll(code, NULL, 10);
}
