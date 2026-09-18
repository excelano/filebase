#!/bin/sh
# Put the desktop entry and the application icon where a desktop will find
# them, for a build made from this checkout.
#
# This is for somebody running from source. The `.deb` installs the same two
# files under `/usr` and needs none of this; `packaging/debian/build-deb.sh`
# is that path.
#
# **There is no media type here, and that is the difference from the rest of
# the family.** Slipcase Desktop and Slipcase Open open a document, so they
# associate themselves with `.slpc` and depend on `slipcase-common` to declare
# the type once for every product that reads it. Filebase opens a *folder*: it
# is handed a directory and asks it a question, and there is no file extension
# in that. So nothing here declares a type, nothing registers a handler, and
# this package depends on no other.
#
# The entry does declare `inode/directory`, which is what puts Filebase in a
# file manager's "Open With" for a folder. That is the same path the binary's
# one positional argument serves, and it is the only reason the argument
# exists.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "${here}/../.." && pwd)

prefix=${XDG_DATA_HOME:-${HOME}/.local/share}

usage() {
    cat <<'USAGE'
usage: install.sh [--prefix DIR]

  --prefix DIR   where to install, default $XDG_DATA_HOME or ~/.local/share
USAGE
}

case "${1:-}" in
    --prefix) [ $# -ge 2 ] || { usage >&2; exit 2; }; prefix=$2 ;;
    -h|--help) usage; exit 0 ;;
    "") ;;
    *) echo "install.sh: unknown argument $1" >&2; usage >&2; exit 2 ;;
esac

install -d "${prefix}/applications" "${prefix}/icons/hicolor/scalable/apps"
install -m 0644 "${here}/filebase.desktop" "${prefix}/applications/filebase.desktop"
install -m 0644 "${here}/icons/filebase.svg" \
    "${prefix}/icons/hicolor/scalable/apps/filebase.svg"

# Both caches, and neither failure is fatal: a desktop that has no such tool
# rebuilds its own on a schedule, and a machine without them is not a machine
# this can fix.
update-desktop-database "${prefix}/applications" 2>/dev/null || true
gtk-update-icon-cache -f -t "${prefix}/icons/hicolor" 2>/dev/null || true

echo "installed the Filebase desktop entry and application icon under ${prefix}"
echo
echo "The entry runs whatever is on PATH. For a build from this checkout:"
echo "  cargo build --release && cp \"\$(cargo metadata --format-version 1 --no-deps \\"
echo "    | sed -n 's/.*\"target_directory\":\"\\([^\"]*\\)\".*/\\1/p')/release/filebase\" ~/.local/bin/"
echo
echo "Then, in a file manager, a folder's Open With should offer Filebase."
