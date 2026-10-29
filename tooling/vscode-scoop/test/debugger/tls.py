"""Check a prebuilt tls.c through LLDB, including the worker's TLS block."""

import lldb


def __lldb_init_module(debugger, internal_dict):
    debugger.SetAsync(False)
    debugger.HandleCommand("settings set target.disable-aslr false")
    target = debugger.GetSelectedTarget()
    breakpoint = target.BreakpointCreateByName("inspect_tls")
    assert breakpoint.GetNumLocations() == 1
    process = target.LaunchSimple(None, None, None)
    try:
        thread_ids = []
        for expected in (42, 84, 42):
            assert process.GetState() == lldb.eStateStopped, process.GetState()
            stopped = [
                thread for thread in process if thread.GetStopReason() == lldb.eStopReasonBreakpoint
            ]
            assert len(stopped) == 1
            thread = stopped[0]
            thread_ids.append(thread.GetThreadID())
            frame = thread.GetFrameAtIndex(0)
            argument = frame.FindVariable("expected")
            assert argument.GetError().Success() and argument.GetValueAsSigned() == expected
            for name, multiplier in (
                ("tls_initialized", 1),
                ("tls_zero", 2),
                ("tls_aligned", 3),
            ):
                value = frame.GetValueForVariablePath(name)
                assert value.GetError().Success(), (name, str(value.GetError()))
                assert value.GetValueAsSigned() == expected * multiplier, (name, str(value))
            assert process.Continue().Success()
        assert thread_ids[0] == thread_ids[2] and thread_ids[0] != thread_ids[1]
        assert process.GetState() == lldb.eStateExited and process.GetExitStatus() == 0
        print("SCOOP_STATIC_TLS_OK")
    finally:
        if process.IsValid() and process.GetState() != lldb.eStateExited:
            process.Kill()
