"""Own a fixture process tree before any child code can run."""

import ctypes
import subprocess
from ctypes import wintypes

kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)


def function(name, result, *arguments):
    value = getattr(kernel32, name)
    value.restype = result
    value.argtypes = arguments
    return value


class BasicLimits(ctypes.Structure):
    _fields_ = [
        ("process_time", ctypes.c_int64),
        ("job_time", ctypes.c_int64),
        ("flags", wintypes.DWORD),
        ("minimum_working_set", ctypes.c_size_t),
        ("maximum_working_set", ctypes.c_size_t),
        ("active_processes", wintypes.DWORD),
        ("affinity", ctypes.c_size_t),
        ("priority", wintypes.DWORD),
        ("scheduling", wintypes.DWORD),
    ]


class ExtendedLimits(ctypes.Structure):
    _fields_ = [
        ("basic", BasicLimits),
        ("io_counters", ctypes.c_uint64 * 6),
        ("process_memory", ctypes.c_size_t),
        ("job_memory", ctypes.c_size_t),
        ("peak_process_memory", ctypes.c_size_t),
        ("peak_job_memory", ctypes.c_size_t),
    ]


class ThreadEntry(ctypes.Structure):
    _fields_ = [
        ("size", wintypes.DWORD),
        ("usage", wintypes.DWORD),
        ("id", wintypes.DWORD),
        ("owner", wintypes.DWORD),
        ("base_priority", wintypes.LONG),
        ("delta_priority", wintypes.LONG),
        ("flags", wintypes.DWORD),
    ]


create_job = function("CreateJobObjectW", wintypes.HANDLE, ctypes.c_void_p, wintypes.LPCWSTR)
set_job = function(
    "SetInformationJobObject",
    wintypes.BOOL,
    wintypes.HANDLE,
    ctypes.c_int,
    ctypes.c_void_p,
    wintypes.DWORD,
)
assign_job = function("AssignProcessToJobObject", wintypes.BOOL, wintypes.HANDLE, wintypes.HANDLE)
close_handle = function("CloseHandle", wintypes.BOOL, wintypes.HANDLE)
snapshot_threads = function(
    "CreateToolhelp32Snapshot", wintypes.HANDLE, wintypes.DWORD, wintypes.DWORD
)
first_thread = function(
    "Thread32First", wintypes.BOOL, wintypes.HANDLE, ctypes.POINTER(ThreadEntry)
)
next_thread = function("Thread32Next", wintypes.BOOL, wintypes.HANDLE, ctypes.POINTER(ThreadEntry))
open_thread = function("OpenThread", wintypes.HANDLE, wintypes.DWORD, wintypes.BOOL, wintypes.DWORD)
resume_thread = function("ResumeThread", wintypes.DWORD, wintypes.HANDLE)


def resume_primary_thread(pid):
    # Popen closes the initial thread handle. While CREATE_SUSPENDED is in
    # effect, the process has only its initial thread and has run no user code.
    snapshot = snapshot_threads(0x00000004, 0)  # TH32CS_SNAPTHREAD
    if snapshot == ctypes.c_void_p(-1).value:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        entry = ThreadEntry(size=ctypes.sizeof(ThreadEntry))
        available = first_thread(snapshot, ctypes.byref(entry))
        while available:
            if entry.owner == pid:
                thread = open_thread(0x0002, False, entry.id)  # THREAD_SUSPEND_RESUME
                if not thread:
                    raise ctypes.WinError(ctypes.get_last_error())
                try:
                    if resume_thread(thread) == 0xFFFFFFFF:
                        raise ctypes.WinError(ctypes.get_last_error())
                    return
                finally:
                    close_handle(thread)
            available = next_thread(snapshot, ctypes.byref(entry))
        raise OSError(f"cannot find initial thread for fixture process {pid}")
    finally:
        close_handle(snapshot)


class Job:
    def __init__(self):
        self.handle = create_job(None, None)
        if not self.handle:
            raise ctypes.WinError(ctypes.get_last_error())
        limits = ExtendedLimits()
        limits.basic.flags = 0x00002000  # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        if not set_job(self.handle, 9, ctypes.byref(limits), ctypes.sizeof(limits)):
            error = ctypes.WinError(ctypes.get_last_error())
            self.close()
            raise error

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()

    def spawn(self, argv, **options):
        child = subprocess.Popen(
            argv, creationflags=0x00000004 | subprocess.CREATE_NEW_PROCESS_GROUP, **options
        )
        try:
            if not assign_job(self.handle, int(child._handle)):
                raise ctypes.WinError(ctypes.get_last_error())
            resume_primary_thread(child.pid)
        except BaseException:
            child.kill()
            child.wait()
            raise
        return child

    def close(self):
        if self.handle:
            if not close_handle(self.handle):
                raise ctypes.WinError(ctypes.get_last_error())
            self.handle = None
