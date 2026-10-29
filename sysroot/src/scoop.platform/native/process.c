#define _GNU_SOURCE 1
#include "process_native.h"
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <spawn.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

struct SpawnDescriptors {
    int child[3];
    int parent[3];
};

static void close_descriptors(struct SpawnDescriptors *descriptors, int success) {
    for (int index = 0; index < 3; ++index) {
        if (descriptors->child[index] >= 0)
            (void)close(descriptors->child[index]);
        if (!success && descriptors->parent[index] >= 0)
            (void)close(descriptors->parent[index]);
    }
}

static int redirect(posix_spawn_file_actions_t *actions, struct SpawnDescriptors *descriptors,
                    int index, ScoopPlatformStdio source) {
    if (source.kind == 0) {
        if (fcntl(index, F_GETFD) >= 0)
            return posix_spawn_file_actions_adddup2(actions, index, index);
        return errno == EBADF ? 0 : errno;
    }
    if (source.kind == 1)
        return posix_spawn_file_actions_addopen(actions, index, "/dev/null",
                                                index == 0 ? O_RDONLY : O_WRONLY, 0);
    if (source.kind == 4)
        return posix_spawn_file_actions_adddup2(actions, 1, 2);
    if (source.kind == 2) {
        int32_t pipes[2];
        if (scoop_platform_pipe_open(pipes) < 0)
            return errno;
        descriptors->child[index] = pipes[index == 0 ? 0 : 1];
        descriptors->parent[index] = pipes[index == 0 ? 1 : 0];
    } else {
        descriptors->child[index] = fcntl(scoop_file_handle(source.handle), F_DUPFD_CLOEXEC, 3);
        if (descriptors->child[index] < 0)
            return errno;
    }
    int error = posix_spawn_file_actions_adddup2(actions, descriptors->child[index], index);
    if (error == 0)
        error = posix_spawn_file_actions_addclose(actions, descriptors->child[index]);
    return error;
}

/* The private caller supplies exactly count checked, NUL-terminated byte strings. */
static char **string_vector(char *data, int64_t count) {
    if (count < 0 || (uint64_t)count >= SIZE_MAX / sizeof(char *)) {
        errno = EOVERFLOW;
        return NULL;
    }
    char **vector = calloc((size_t)count + 1, sizeof(*vector));
    if (vector == NULL)
        return NULL;
    for (int64_t index = 0; index < count; ++index) {
        vector[index] = data;
        data += strlen(data) + 1;
    }
    return vector;
}

static int configure_attributes(posix_spawnattr_t *attributes) {
    sigset_t mask, defaults;
    sigemptyset(&mask);
    sigemptyset(&defaults);
    sigaddset(&defaults, SIGPIPE);
    int error = posix_spawnattr_setsigmask(attributes, &mask);
    if (error == 0)
        error = posix_spawnattr_setsigdefault(attributes, &defaults);
    short flags = POSIX_SPAWN_SETSIGMASK | POSIX_SPAWN_SETSIGDEF;
#ifdef __APPLE__
    flags |= POSIX_SPAWN_CLOEXEC_DEFAULT;
#endif
    return error == 0 ? posix_spawnattr_setflags(attributes, flags) : error;
}

ScoopPlatformSpawnResult scoop_platform_process_spawn(const char *executable, char *arguments,
                                                      int64_t argument_count, char *environment,
                                                      int64_t environment_count,
                                                      const char *directory,
                                                      const ScoopPlatformStdio redirects[3],
                                                      ScoopPlatformFileHandle result[3]) {
    char **argv = string_vector(arguments, argument_count);
    if (argv == NULL)
        return scoop_process_failure(errno);
    char **envp = string_vector(environment, environment_count);
    if (envp == NULL) {
        int error = errno;
        free(argv);
        errno = error;
        return scoop_process_failure(error);
    }
    posix_spawn_file_actions_t actions;
    posix_spawnattr_t attributes;
    struct SpawnDescriptors descriptors = {{-1, -1, -1}, {-1, -1, -1}};
    int error = posix_spawn_file_actions_init(&actions);
    if (error != 0)
        goto finish;
    error = posix_spawnattr_init(&attributes);
    if (error != 0)
        goto destroy_actions;
    error = configure_attributes(&attributes);
    if (error == 0) {
#if defined(__APPLE__) && __MAC_OS_X_VERSION_MIN_REQUIRED >= 260000
        error = posix_spawn_file_actions_addchdir(&actions, directory);
#else
        error = posix_spawn_file_actions_addchdir_np(&actions, directory);
#endif
    }
    for (int index = 0; index < 3 && error == 0; ++index)
        error = redirect(&actions, &descriptors, index, redirects[index]);
    pid_t child = -1;
    if (error == 0)
        error = posix_spawn(&child, executable, &actions, &attributes, argv, envp);
    if (error == 0) {
        for (int index = 0; index < 3; ++index)
            result[index] = scoop_file_wrap(descriptors.parent[index]);
    }
    close_descriptors(&descriptors, error == 0);
    (void)posix_spawnattr_destroy(&attributes);
destroy_actions:
    (void)posix_spawn_file_actions_destroy(&actions);
finish:
    free(envp);
    free(argv);
    errno = error;
    return error == 0 ? (ScoopPlatformSpawnResult){{(uint64_t)child}, 0}
                      : scoop_process_failure(error);
}
