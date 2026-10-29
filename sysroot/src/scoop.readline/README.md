# Scoop readline

Line editing ported from [rustyline 19.0.0](https://github.com/kkawakam/rustyline/tree/v19.0.0),
with Darwin, Linux and Windows console support.
The MIT license is retained in [LICENSE-rustyline](LICENSE-rustyline).
The implementation uses Scoop, the existing terminal library, ICU4X and unicode-width
Unicode operations through scoop.charset, and platform file locks. The fixed Rust
text archive is prepared with the sysroot; applications consume precompiled
resources. SQLite history is outside this port.

Windows filename completion preserves drive and UNC path separators and quotes
names containing spaces. Interactive tests use real ConPTY sessions and check
input/output modes and code pages after normal and exceptional returns. Ctrl-Z
reports the platform's Unsupported error after restoring the session; Unix keeps
its existing suspend/resume behavior.

The public contract is in [the readline specification](../../../docs/specs/SCOOP-READLINE-SPEC.md).

```toml
[dependencies]
"scoop:scoop.readline" = "0.1.0"
```

```scoop
import scoop.readline.*
import scoop.readline.history.*
import scoop.platform.fs.*

fun main(): Int {
    val history = FileHistory()
    val path = Path.fromString(".history")
    if (path.exists()) history.load(path)
    val editor = Editor(history = history)
    editor.hinter = HistoryHinter()
    editor.completer = FilenameCompleter()
    editor.validator = MatchingBracketValidator()
    while (true) {
        try {
            val line = editor.readLine("> ")
            history.add(line)
            println(line)
        } catch (end: ReadlineEofException) { break }
        catch (interrupted: ReadlineInterruptedException) { continue }
    }
    history.append(path)
    return 0
}
```

Use `MemoryHistory` for transient storage, or implement `History` to supply another
backend. File persistence is a separate `PersistentHistory` capability. FileHistory
reads rustyline V1/V2 files and writes V2, including multiline entries.

`Config(editMode = EditMode.Vi)` selects Vi. `Completer`, `Hinter`, `Highlighter`
and `Validator` can be implemented independently. All public cursor positions are
Unicode scalar indices; editing moves and deletes complete grapheme clusters.
`Editor.start()` exposes the same `EditingSession` used by interactive input.
`createExternalPrinter()` supplies thread-safe messages during editing. Ctrl-Z restores
the terminal before suspension and resumes with the same input.

`Config(completionType = CompletionType.Fuzzy)` enables interactive candidate
filtering. `Config(historyCaseSensitive = false)` selects Unicode caseless history
search. The public API follows Scoop conventions: callbacks are independent
interfaces, persistence is a separate capability, and indices count Unicode scalars
instead of UTF-8 bytes. Fuzzy selection uses the same editor and a stable subsequence
ranking; it does not embed the Rust skim UI.

After preparing the development sysroot, run:

```sh
SCOOP_SYSROOT="$PWD/sysroot" target/release/scoop test sysroot/src/scoop.readline --release
```

Tests use `scoop.test`, including real PTY subprocesses for input, bracketed paste,
resize, suspend/resume, external printing, interrupt/EOF, callback failures and
terminal restoration. The small
`test-native` Cone only owns the PTY and subprocess; it contains no editing logic.
Ignored child cases are selected explicitly by their parent tests.

Validation on 2026-10-22: release tests pass on aarch64-apple-darwin,
x86_64-unknown-linux-gnu and x86_64-unknown-linux-musl: 39 top-level cases and
all six PTY child cases. The child cases are marked ignored in the ordinary test
list and launched by their parents. The musl PTY helper initializes termios before
comparing restored state; its four affected cases pass after that fix.

Unicode operations now use the shared Rust text backend through `scoop.charset`.
API documentation checks pass. No fixtures, dedicated lower-layer gap tests or
upstream Unicode compatibility/completeness tests are added. Raw logs remain
outside the repository.
