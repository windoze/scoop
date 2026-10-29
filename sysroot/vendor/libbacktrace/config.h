/* Fixed configuration for Scoop's 64-bit Darwin and Linux runtime targets. */
#ifndef SCOOP_LIBBACKTRACE_CONFIG_H
#define SCOOP_LIBBACKTRACE_CONFIG_H
#ifndef _GNU_SOURCE
#define _GNU_SOURCE 1
#endif
#ifndef _DARWIN_C_SOURCE
#define _DARWIN_C_SOURCE 1
#endif
#define HAVE_ATOMIC_FUNCTIONS 1
#define HAVE_SYNC_FUNCTIONS 1
#if !defined(__APPLE__) || !defined(_POSIX_C_SOURCE) || _POSIX_C_SOURCE < 200112L
#define HAVE_DECL_GETPAGESIZE 1
#endif
#define HAVE_DECL_STRNLEN 1
#define HAVE_FCNTL 1
#define HAVE_LSTAT 1
#define HAVE_READLINK 1
#define HAVE_GETIPINFO 1
#define HAVE_STDINT_H 1
#define HAVE_INTTYPES_H 1
#define HAVE_STDLIB_H 1
#define HAVE_STRING_H 1
#define HAVE_STRINGS_H 1
#define HAVE_SYS_MMAN_H 1
#define HAVE_SYS_STAT_H 1
#define HAVE_SYS_TYPES_H 1
#define HAVE_UNISTD_H 1
#if defined(__APPLE__)
#define HAVE_MACH_O_DYLD_H 1
#else
#define HAVE_DL_ITERATE_PHDR 1
#define HAVE_LINK_H 1
#define BACKTRACE_ELF_SIZE 64
#endif
#endif
