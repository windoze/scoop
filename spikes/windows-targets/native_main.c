#include <assert.h>
#include <errno.h>
#include <locale.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <windows.h>

extern int native_probe(void);
extern int seh_probe(void);

static _Thread_local int local = 42;

static DWORD WINAPI thread_probe(void *context) {
    (void)context;
    assert(local == 42);
    local = 9;
    errno = 13;
    return 7;
}

int main(void) {
    _Static_assert(sizeof(long) == 4, "Win64 is LLP64");
    _Static_assert(sizeof(void *) == 8, "Win64 pointer size");
    _Static_assert(sizeof(int64_t) == 8, "Scoop Long uses int64_t");
    local = 17;
    errno = 0;
    HANDLE thread = CreateThread(NULL, 0, thread_probe, NULL, 0, NULL);
    assert(thread != NULL);
    assert(WaitForSingleObject(thread, INFINITE) == WAIT_OBJECT_0);
    DWORD result;
    assert(GetExitCodeThread(thread, &result) && result == 7);
    assert(CloseHandle(thread));
    assert(local == 17 && errno == 0);
    _locale_t locale = _create_locale(LC_ALL, "C");
    assert(locale != NULL);
    char *end = NULL;
    assert(_strtod_l("1.25!", &end, locale) == 1.25 && *end == '!');
    _free_locale(locale);
    assert(native_probe() == 2);
    assert(seh_probe() == 0);
    puts("native C/C++ and Scoop SEH passed");
    return 0;
}
