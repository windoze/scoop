#!/bin/sh
# Build the installed system libraries and their offline API documentation.
set -eu

scoop_root=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
scoop_plan="$scoop_root/share/scoop/install"
scoop_targets=''
scoop_all_targets=false
scoop_jobs=6
scoop_work="$scoop_root/build/post-install"
scoop_cc=''
scoop_native_sysroot=''

scoop_error() {
    printf 'post-install: %s\n' "$*" >&2
    exit 2
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --target|--jobs|--work-dir|--cc|--native-sysroot)
            [ "$#" -ge 2 ] && [ -n "$2" ] || scoop_error "$1 requires a value"
            case "$1" in
                --target)
                    grep -Fxq -- "$2" "$scoop_plan/targets.list" ||
                        scoop_error "target $2 is not included in this installation"
                    case " $scoop_targets " in
                        *" $2 "*) ;;
                        *) scoop_targets="${scoop_targets:+$scoop_targets }$2" ;;
                    esac
                    ;;
                --jobs) scoop_jobs=$2 ;;
                --work-dir) scoop_work=$2 ;;
                --cc) scoop_cc=$2 ;;
                --native-sysroot) scoop_native_sysroot=$2 ;;
            esac
            shift 2
            ;;
        --all-targets) scoop_all_targets=true; shift ;;
        --help|-h)
            cat <<'HELP'
Usage: sh post-install.sh [options]
Build release system libraries and offline API documentation after extraction.

  --target TARGET        Prepare a bundled target (repeatable)
  --all-targets          Prepare every bundled target; requires their SDKs
  --jobs N               Maximum parallel build jobs (default: 6)
  --work-dir PATH        Build cache and intermediates (default: build/post-install)
  --cc PATH              C compiler for the selected targets
  --native-sysroot PATH  Native SDK for the selected targets

With no target option, prepare the package's default target.
HELP
            exit 0
            ;;
        *) scoop_error "unknown option: $1 (see --help)" ;;
    esac
done

case "$scoop_jobs" in ''|*[!0-9]*) scoop_error '--jobs must be positive' ;; esac
[ "$scoop_jobs" -gt 0 ] || scoop_error '--jobs must be positive'
if [ "$scoop_all_targets" = true ]; then
    [ -z "$scoop_targets" ] || scoop_error '--target and --all-targets are mutually exclusive'
    scoop_targets=$(cat "$scoop_plan/targets.list")
elif [ -z "$scoop_targets" ]; then
    IFS= read -r scoop_targets < "$scoop_plan/targets.list"
fi

mkdir -p -- "$scoop_work"
scoop_work=$(CDPATH='' cd -- "$scoop_work" && pwd)
mkdir -p "$scoop_work/scratch"
export TMPDIR="$scoop_work/scratch" TMP="$scoop_work/scratch" TEMP="$scoop_work/scratch"

scoop_run() {
    if [ -n "$scoop_cc" ]; then set -- "$@" --cc "$scoop_cc"; fi
    if [ -n "$scoop_native_sysroot" ]; then
        set -- "$@" --native-sysroot "$scoop_native_sysroot"
    fi
    "$scoop_root/bin/scoop" "$@"
}

# Target triples contain no whitespace; Cone paths are read one per line.
for scoop_target in $scoop_targets; do
    mkdir -p "$scoop_root/lib/$scoop_target"
    while IFS= read -r scoop_cone; do
        printf 'Building %s for %s\n' "$scoop_cone" "$scoop_target"
        set -- build "$scoop_root/src/$scoop_cone" \
            --release --jobs "$scoop_jobs" --target "$scoop_target" \
            --sysroot "$scoop_root" --scoopc "$scoop_root/bin/scoopc" \
            --cache-dir "$scoop_work/cache" \
            --target-dir "$scoop_work/$scoop_target/target/$scoop_cone" \
            --lockfile "$scoop_work/$scoop_target/locks/$scoop_cone.lock" \
            -o "$scoop_root/lib/$scoop_target/$scoop_cone.slib"
        case "$scoop_target" in
            *-apple-darwin) ;;
            *) set -- "$@" --unwind-prefix "$scoop_root/lib/$scoop_target/unwind" ;;
        esac
        scoop_run "$@"
    done < "$scoop_plan/$scoop_target.list"

    printf 'Generating API documentation for %s\n' "$scoop_target"
    set -- doc
    while IFS= read -r scoop_cone; do
        set -- "$@" "$scoop_root/src/$scoop_cone/Cone.toml"
    done < "$scoop_plan/$scoop_target.list"
    scoop_run "$@" --jobs "$scoop_jobs" --target "$scoop_target" \
        --sysroot "$scoop_root" --scoopc "$scoop_root/bin/scoopc" \
        --no-deps --deny-doc-warnings --cache-dir "$scoop_work/cache" \
        --target-dir "$scoop_work/doc-target" --output-dir "$scoop_root/docs/$scoop_target"
done

{
    cat <<'HTML'
<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Scoop built-in API documentation</title></head>
<body><main><h1>Scoop built-in API documentation</h1>
<p>Select the target used to compile your application.
All pages, source text, and search resources are included locally.</p><ul>
HTML
    while IFS= read -r scoop_target; do
        if [ -f "$scoop_root/docs/$scoop_target/index.html" ]; then
            printf '<li><a href="%s/index.html">%s</a></li>\n' "$scoop_target" "$scoop_target"
        fi
    done < "$scoop_plan/targets.list"
    printf '</ul></main></body></html>\n'
} > "$scoop_root/docs/index.html"
printf 'System libraries and API documentation are ready in %s\n' "$scoop_root"
