#!/bin/sh
# Take back what install.sh put down.
#
# Two files and two cache refreshes. There is no media type to think about:
# Filebase declares none, because it opens a folder rather than a document.
# See install.sh.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)
set -eu

prefix=${XDG_DATA_HOME:-${HOME}/.local/share}
case "${1:-}" in
    --prefix) [ $# -ge 2 ] || { echo "usage: uninstall.sh [--prefix DIR]" >&2; exit 2; }; prefix=$2 ;;
    "") ;;
    -h|--help) echo "usage: uninstall.sh [--prefix DIR]"; exit 0 ;;
    *) echo "uninstall.sh: unknown argument $1" >&2; exit 2 ;;
esac

rm -f "${prefix}/applications/filebase.desktop"
rm -f "${prefix}/icons/hicolor/scalable/apps/filebase.svg"
update-desktop-database "${prefix}/applications" 2>/dev/null || true
gtk-update-icon-cache -f -t "${prefix}/icons/hicolor" 2>/dev/null || true

echo "removed the Filebase desktop entry and icon from ${prefix}"
