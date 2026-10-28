# Readline PTY test support

This ordinary test dependency opens a Unix pseudo-terminal or Windows ConPTY and launches selected
ignored cases in the same `scoop.test` executable. It removes the parent runner's
capture environment, so child output cannot overwrite the parent's reports.
Production readline code does not depend on this Cone.

Each child captures its terminal settings before entering readline and checks
restoration after readline returns, before exiting. Windows compares input/output
console modes and code pages. ConPTY output is drained on a separate reader thread,
including during shutdown, following the [Windows pseudoconsole lifecycle](https://learn.microsoft.com/en-us/windows/console/creating-a-pseudoconsole-session).
Darwin
revokes the controlling PTY when its session leader exits. The comparison excludes
PENDIN, a kernel-maintained pending-input state, and includes the actual mode flags
and control characters. The observer's slave descriptor is closed before waiting
for session teardown.

The Unix suspension test catches SIGTSTP and stops with SIGSTOP because the child owns
an orphan process group. The parent checks restored settings while it is stopped,
then sends SIGCONT and checks that editing resumes with the original text.
Windows instead checks that Ctrl-Z reports the platform's Unsupported error after
restoring the console. Its test dependency does not expose Unix job-control operations.
