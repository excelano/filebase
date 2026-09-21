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
# **One corpus per language, and the paths sort into the same order in both.**
# A recipe's `FIRST_ROW` and `RICHEST_ROW` are pixel coordinates, which in a
# sorted table are row numbers in disguise: translate a filename carelessly and
# row five becomes row three, the shot of the fullest description photographs
# the emptiest, and it passes. So the German names were chosen to sort where
# their English counterparts do — `rechnung` for `invoice` under `2025/`,
# `aussennotizen` before `q3-bericht` as `field-notes` is before `q3-report`,
# and `rahmenvertrag` at the root where `master-services-agreement` is. Change
# a name and check the order again; the German set needs no coordinates of its
# own only because of this.
#
# **The keys are English in both.** They are the schema a person chose, not the
# interface, and the example query in both store listings is the same SlipQL
# either way — `select @path, title, governance.owner where status = "draft"`
# reads the key `title` whatever language the values are in. Only the values,
# the file names and the folder the set is staged under are translated.
#
# It needs the `slipcase` command, which is `slpc-rust`'s and is on apt as
# `slipcase`. That is a build-time dependency of the screenshots and of nothing
# else: the application itself parses containers through the library.
#
#     ./packaging/demo-corpus.sh                 # writes ./dist/corpus
#     ./packaging/demo-corpus.sh --out DIR
#     ./packaging/demo-corpus.sh --lang de       # the German set
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "${here}/.." && pwd)
out="${root}/dist/corpus"

lang=en

while [ $# -gt 0 ]; do
    case "$1" in
        --out) out="${2:?--out needs a directory}"; shift 2 ;;
        --lang) lang="${2:?--lang needs a language tag}"; shift 2 ;;
        -h|--help) sed -n '2,55p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "demo-corpus.sh: unknown argument $1" >&2; exit 2 ;;
    esac
done

# Refusing an unknown language rather than falling back to English, for the
# reason `shots.sh` refuses one: a German frame of an English corpus is a
# German listing of somebody else's application, and it looks finished.
case "$lang" in
    en|en-US|en-us)
        agreement=master-services-agreement.pdf; agreement_title='Master services agreement'
        renewal=renewal-2026.docx;               renewal_title='Renewal, 2026'
        report=q3-report.xlsx;                   report_title='Q3 report'
        notes=field-notes.txt;                   notes_title='Field notes'
        invoice=invoice-1183.pdf;                invoice_title='Invoice 1183'
        signed=signed; draft=draft; paid=paid; many=many
        t_legal=legal; t_contract=contract; t_finance=finance; t_site=site; t_archive=archive
        say_agreement='The agreement, as a document would be.'
        say_renewal='The renewal, still being written.'
        say_report='Numbers, as a spreadsheet would hold them.'
        say_notes='Notes taken on a visit.'
        say_invoice='An invoice, settled.'
        broken=broken.slpc;  say_broken='This is not a container.'
        plain=README.md;     say_plain='A plain file, which the scan does not mention.' ;;
    de|de-DE|de-de)
        agreement=rahmenvertrag.pdf;             agreement_title='Rahmenvertrag'
        renewal=verlaengerung-2026.docx;         renewal_title='Verlängerung, 2026'
        report=q3-bericht.xlsx;                  report_title='Q3-Bericht'
        notes=aussennotizen.txt;                 notes_title='Außennotizen'
        invoice=rechnung-1183.pdf;               invoice_title='Rechnung 1183'
        signed=unterzeichnet; draft=entwurf; paid=bezahlt; many=viele
        t_legal=recht; t_contract=vertrag; t_finance=finanzen; t_site=vorort; t_archive=archiv
        say_agreement='Der Vertrag, wie ein Dokument es wäre.'
        say_renewal='Die Verlängerung, noch im Entwurf.'
        say_report='Zahlen, wie eine Tabelle sie hielte.'
        say_notes='Notizen von einem Besuch.'
        say_invoice='Eine Rechnung, beglichen.'
        broken=defekt.slpc;  say_broken='Dies ist kein Container.'
        plain=LIESMICH.md;   say_plain='Eine gewöhnliche Datei, die der Durchlauf nicht nennt.' ;;
    *) echo "demo-corpus.sh: no corpus is written for ${lang}" >&2; exit 2 ;;
esac

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

pack "$agreement" "." "title = \"${agreement_title}\"
status = \"${signed}\"
signed = 2026-03-14
pages = 42
tags = [\"${t_legal}\", \"${t_contract}\"]

[governance]
owner = \"Kim\"
privacy_flag = false" "$say_agreement"

pack "$renewal" "2026" "title = \"${renewal_title}\"
status = \"${draft}\"
pages = 6
tags = [\"${t_legal}\"]

[governance]
owner = \"Lee\"
privacy_flag = true" "$say_renewal"

pack "$report" "2026/q3" "title = \"${report_title}\"
status = \"${signed}\"
signed = 2026-09-01
pages = 12
tags = [\"${t_finance}\"]

[governance]
owner = \"Lee\"
privacy_flag = false" "$say_report"

# No `governance` table at all: the frame wants an empty cell in it.
pack "$notes" "2026/q3" "title = \"${notes_title}\"
status = \"${draft}\"
pages = \"${many}\"
tags = [\"${t_site}\"]" "$say_notes"

pack "$invoice" "2025" "title = \"${invoice_title}\"
status = \"${paid}\"
signed = 2025-11-02
pages = 2
tags = [\"${t_finance}\", \"${t_archive}\"]

[governance]
owner = \"Kim\"" "$say_invoice"

# Named like a container and not one, so the scan has something to skip and
# report. Deliberately not a truncated container: what is being shown is the
# notice, and the shortest thing that produces one is text.
printf '%s\n' "$say_broken" > "${out}/2026/${broken}"

# Not named like one, so the scan never looks at it. Its absence from the
# notices is as much a part of the frame as the line above it.
printf '%s\n' "$say_plain" > "${out}/2026/${plain}"

# Counted by asking the library, not by globbing: `broken.slpc` is named like
# a container on purpose and is exactly what a glob would miscount.
packed=0
for f in $(find "$out" -name '*.slpc'); do
    slipcase validate "$f" >/dev/null 2>&1 && packed=$((packed + 1))
done
echo "wrote ${packed} containers into ${out}, beside a file named like one that is not and a file that is not named like one"
find "$out" -type f | sed "s|^${out}|  .|" | sort
