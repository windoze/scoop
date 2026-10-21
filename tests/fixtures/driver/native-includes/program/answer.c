#include <target.h>

int m51_target_header_answer(void) {
#if defined(SCOOP_TARGET_OS_DARWIN)
    return TARGET_NUMBER + 35;
#elif defined(SCOOP_TARGET_ENV_GNU)
    return TARGET_NUMBER + 29;
#else
    return TARGET_NUMBER + 21;
#endif
}
