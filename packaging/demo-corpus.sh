#!/bin/sh
# Build the folder of containers the screenshots are taken of, the same way on
# every platform.
#
# The sibling applications open one document, so each carries a
# `demo-container.sh` that writes one file. Filebase opens a *folder*, and a
# folder of one container is a screenshot of an empty product — so this writes
# a small tree instead: containers at three depths, with descriptions that
# differ from one another in the ways a query has to survive.
#
# **What the corpus has to contain, and why each thing is in it.** Every frame
# of this application is rows, and rows of one column of identical values
# demonstrate nothing. So:
#
#   - Keys that are not on every container. `governance.owner` is on three of
#     the five, so the frame shows the empty cell that is the whole reason a
#     missing key does not fail a query.
#   - A value of the wrong type. `field-notes.txt.slpc` records `pages = "many"`
#     where the rest record a number, so a query with `pages > 10` in it
#     produces the cross-type notice under the rows, which is one of the three
#     shots worth taking.
#   - A file that is named like a container and is not one, so the skip notice
#     has something to report, and a file that is not named like one, so the
#     scan can be seen not to mention it.
#   - Three levels, so `recursive` is visibly doing something.
#   - Payloads of several kinds, so the payload card is not five copies of one
#     sentence.
#
# **The subject is invented.** No real person, organisation, matter or date
# appears in it, because these frames go into two store listings and onto a
# website. The names are the two the fleet's other samples use.
#
# It needs the `slipcase` command, which is `slpc-rust`'s and is on apt as
# `slipcase`. That is a build-time dependency of the screenshots and of nothing
# else: the application itself parses containers through the library.
#
#     ./packaging/demo-corpus.sh                 # writes ./dist/corpus
#     ./packaging/demo-corpus.sh --out DIR
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "${here}/.." && pwd)
out="${root}/dist/corpus"

while [ $# -gt 0 ]; do
    case "$1" in
        --out) out="${2:?--out needs a directory}"; shift 2 ;;
        -h|--help) sed -n '2,40p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "demo-corpus.sh: unknown argument $1" >&2; exit 2 ;;
    esac
done

command -v slipcase >/dev/null || {
    echo "demo-corpus.sh: no 'slipcase' on PATH; it is slpc-rust's command" >&2
    echo "demo-corpus.sh: sudo apt install slipcase, or cargo install slipcase" >&2
    exit 1
}

rm -rf "$out"
mkdir -p "$out/2026/q3" "$out/2025"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT INT TERM

# pack <name> <subdirectory> <metadata> <payload text>
pack() {
    printf '%s\n' "$4" > "${work}/$1"
    printf '%s\n' "$3" > "${work}/meta.toml"
    slipcase pack "${work}/$1" --name "$1" --meta "${work}/meta.toml" \
        -o "${out}/$2/$1.slpc" --force >/dev/null
    rm -f "${work}/$1" "${work}/meta.toml"
}

pack "master-services-agreement.pdf" "." 'title = "Master services agreement"
status = "signed"
signed = 2026-03-14
pages = 42
tags = ["legal", "contract"]

[governance]
owner = "Kim"
privacy_flag = false' "The agreement, as a document would be."

pack "renewal-2026.docx" "2026" 'title = "Renewal, 2026"
status = "draft"
pages = 6
tags = ["legal"]

[governance]
owner = "Lee"
privacy_flag = true' "The renewal, still being written."

pack "q3-report.xlsx" "2026/q3" 'title = "Q3 report"
status = "signed"
signed = 2026-09-01
pages = 12
tags = ["finance"]

[governance]
owner = "Lee"
privacy_flag = false' "Numbers, as a spreadsheet would hold them."

# No `governance` table at all: the frame wants an empty cell in it.
pack "field-notes.txt" "2026/q3" 'title = "Field notes"
status = "draft"
pages = "many"
tags = ["site"]' "Notes taken on a visit."

pack "invoice-1183.pdf" "2025" 'title = "Invoice 1183"
status = "paid"
signed = 2025-11-02
pages = 2
tags = ["finance", "archive"]

[governance]
owner = "Kim"' "An invoice, settled."

# Named like a container and not one, so the scan has something to skip and
# report. Deliberately not a truncated container: what is being shown is the
# notice, and the shortest thing that produces one is text.
printf 'This is not a container.\n' > "${out}/2026/broken.slpc"

# Not named like one, so the scan never looks at it. Its absence from the
# notices is as much a part of the frame as the line above it.
printf 'A plain file, which the scan does not mention.\n' > "${out}/2026/README.md"

# Counted by asking the library, not by globbing: `broken.slpc` is named like
# a container on purpose and is exactly what a glob would miscount.
packed=0
for f in $(find "$out" -name '*.slpc'); do
    slipcase validate "$f" >/dev/null 2>&1 && packed=$((packed + 1))
done
echo "wrote ${packed} containers into ${out}, beside a file named like one that is not and a file that is not named like one"
find "$out" -type f | sed "s|^${out}|  .|" | sort
