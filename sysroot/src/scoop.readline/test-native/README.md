# Readline PTY test support

This ordinary test dependency opens a Unix pseudo-terminal and launches selected
ignored cases in the same `scoop.test` executable. It removes the parent runner's
capture environment, so child output cannot overwrite the parent's reports.
Production readline code does not depend on this Cone.

Restoration is checked after readline returns and before the child exits. Darwin
revokes the controlling PTY when its session leader exits. The comparison excludes
PENDIN, a kernel-maintained pending-input state, and includes the actual mode flags
and control characters. The observer's slave descriptor is closed before waiting
for session teardown.

The suspension test catches SIGTSTP and stops with SIGSTOP because the child owns
an orphan process group. The parent checks restored settings while it is stopped,
then sends SIGCONT and checks that editing resumes with the original text.
