#include <string.h>
#include <unistd.h>

int main(int argc, char **argv) {
    if (argc != 3)
        return 90;
    char invalid[] = {'r', 'a', 'w', (char)0xff, 0};
    char *bad[] = {invalid, "valid", "valid", NULL};
    char *unit[] = {"../literal app name", "two words", "", "中文🙂", "--literal", NULL};
    char *integer[] = {"../literal app name", "normal", NULL};
    char **arguments = strcmp(argv[2], "unit") == 0  ? unit
                       : strcmp(argv[2], "int") == 0 ? integer
                                                     : bad;
    execv(argv[1], arguments);
    return 91;
}
