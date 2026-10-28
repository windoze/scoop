#ifndef SCOOP_PLATFORM_ERRORS_WINDOWS_H
#define SCOOP_PLATFORM_ERRORS_WINDOWS_H

#define WIN32_NO_STATUS
#ifdef _MSC_VER
#include <WinSock2.h>
#else
#include <winsock2.h>
#endif
#include "windows_api.h"
#undef WIN32_NO_STATUS
#include <winternl.h>
#include <ntstatus.h>
#include <stdint.h>

static int32_t ntstatus_error_kind(int32_t code) {
    switch ((NTSTATUS)code) {
    case STATUS_INVALID_HANDLE:
    case STATUS_INVALID_PARAMETER:
        return 6;
    case STATUS_NO_MEMORY:
        return 7;
    case STATUS_NOT_SUPPORTED:
        return 17;
    default:
        return 0;
    }
}

static int32_t win32_error_kind(int32_t code) {
    switch ((uint32_t)code) {
    case ERROR_FILE_NOT_FOUND:
    case ERROR_PATH_NOT_FOUND:
    case ERROR_INVALID_DRIVE:
    case ERROR_BAD_NETPATH:
    case ERROR_BAD_NET_NAME:
        return 1;
    case ERROR_FILE_EXISTS:
    case ERROR_ALREADY_EXISTS:
        return 2;
    case ERROR_ACCESS_DENIED:
    case ERROR_PRIVILEGE_NOT_HELD:
    case ERROR_WRITE_PROTECT:
        return 3;
    case ERROR_DIRECTORY:
        return 4;
    case ERROR_INVALID_PARAMETER:
    case ERROR_INVALID_HANDLE:
    case ERROR_INVALID_NAME:
    case ERROR_BAD_PATHNAME:
    case ERROR_FILENAME_EXCED_RANGE:
    case ERROR_NO_UNICODE_TRANSLATION:
        return 6;
    case ERROR_NOT_ENOUGH_MEMORY:
    case ERROR_OUTOFMEMORY:
    case ERROR_COMMITMENT_LIMIT:
        return 7;
    case ERROR_DISK_FULL:
    case ERROR_HANDLE_DISK_FULL:
        return 8;
    case ERROR_BROKEN_PIPE:
    case ERROR_NO_DATA:
    case ERROR_PIPE_NOT_CONNECTED:
        return 9;
    case ERROR_SEM_TIMEOUT:
    case ERROR_TIMEOUT:
    case WAIT_TIMEOUT:
        return 14;
    case ERROR_OPERATION_ABORTED:
        return 15;
    case ERROR_IO_PENDING:
    case ERROR_IO_INCOMPLETE:
    case ERROR_LOCK_VIOLATION:
        return 16;
    case ERROR_NOT_SUPPORTED:
    case ERROR_CALL_NOT_IMPLEMENTED:
    case ERROR_INVALID_FUNCTION:
        return 17;
    default:
        return 0;
    }
}

static int32_t winsock_error_kind(int32_t code) {
    switch (code) {
    case WSAHOST_NOT_FOUND:
    case WSANO_DATA:
        return 1;
    case WSAEACCES:
        return 3;
    case WSA_INVALID_HANDLE:
    case WSA_INVALID_PARAMETER:
    case WSAEBADF:
    case WSAEFAULT:
    case WSAEINVAL:
    case WSAENOTSOCK:
        return 6;
    case WSA_NOT_ENOUGH_MEMORY:
    case WSAENOBUFS:
        return 7;
    case WSAESHUTDOWN:
        return 9;
    case WSAECONNREFUSED:
        return 10;
    case WSAECONNRESET:
    case WSAECONNABORTED:
    case WSAENETRESET:
        return 11;
    case WSAEADDRINUSE:
        return 12;
    case WSAEADDRNOTAVAIL:
        return 13;
    case WSAETIMEDOUT:
        return 14;
    case WSAEINTR:
    case WSA_OPERATION_ABORTED:
        return 15;
    case WSAEWOULDBLOCK:
    case WSAEINPROGRESS:
    case WSAEALREADY:
    case WSA_IO_PENDING:
    case WSA_IO_INCOMPLETE:
    case WSATRY_AGAIN:
        return 16;
    case WSAEAFNOSUPPORT:
    case WSAEPROTONOSUPPORT:
    case WSAESOCKTNOSUPPORT:
    case WSAEOPNOTSUPP:
    case WSATYPE_NOT_FOUND:
        return 17;
    default:
        return 0;
    }
}

#endif
