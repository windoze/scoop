#ifndef SCOOP_PLATFORM_WINDOWS_API_H
#define SCOOP_PLATFORM_WINDOWS_API_H
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifdef _MSC_VER
#include <Windows.h>
#else
#include <windows.h>
#endif
#endif
