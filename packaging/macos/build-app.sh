#!/bin/sh
# Assemble the application bundle DESIGN.md §8 describes: the executable, the
# property list, and the
# three icons.
#
# The bundle is the unit of everything on macOS. A bare executable can draw a
# window, but it has no bundle identifier, Launch Services files it as a
# nameless foreground process, and nothing can be registered or associated
# with it. `lsappinfo` reports `bundleID=[ NULL ]` for one, which is the whole
# reason this script exists.
#
# It signs the bundle when it is given an identity, because the Mac App Store
# is the chosen channel and an unsigned bundle is not a thing that can be
# tested: the App Sandbox is inert until the entitlement is inside a signature,
# so an unsigned bundle carrying `Filebase.entitlements` is not sandboxed and
# proves nothing. `README.md` beside this file says which certificate is which.
#
# This is slipcase-desktop's script with three icons where it has one and two
# type declarations where it has one. Every refusal in it was measured there
# first, and the comment beside each says what it cost.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)
set -eu

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "${here}/../.." && pwd)
binary=""
outdir="${root}/dist"
# Not on PATH, and README.md says so.
lsregister=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
universal=no
identity=""
store_profile=""

usage() {
    cat <<'USAGE'
usage: build-app.sh [--binary PATH] [--outdir DIR] [--universal] [--sign ID]
                    [--store PROFILE]

  --binary PATH  the executable to bundle (default: the release build)
  --outdir DIR   where to write Filebase.app (default: ./dist)
  --sign ID      sign the finished bundle with this identity and the sandbox
                 entitlements beside this script. `security find-identity -v
                 -p codesigning` lists what this machine holds. An Apple
                 Development identity is enough to test the sandbox; a Store
                 upload needs Apple Distribution.
  --universal    join the two per-architecture release builds with lipo, for
                 a Store build that has to run on Apple silicon and Intel:

                   MACOSX_DEPLOYMENT_TARGET=11.0 \
                     cargo build --release --target x86_64-apple-darwin
                   MACOSX_DEPLOYMENT_TARGET=11.0 \
                     cargo build --release --target aarch64-apple-darwin
                   ./packaging/macos/build-app.sh --universal
  --store PROFILE
                 build what the Mac App Store takes: a universal bundle
                 carrying PROFILE as embedded.provisionprofile, signed for
                 distribution, wrapped by productbuild into the .pkg that is
                 uploaded. It chooses its own identities and refuses rather
                 than producing something subtly wrong: the binary must carry
                 both architectures and must agree with the floor Info.plist
                 declares, however it was built. PROFILE is the
                 .provisionprofile downloaded from the developer portal.

                 The release binary is built on the Apple silicon runner and
                 attached to the release, so signing it here is a download and
                 this command, with no toolchain and nothing compiled:

                   gh release download v0.1.0 -p filebase-universal
                   ./packaging/macos/build-app.sh \
                       --binary ./filebase-universal \
                       --store ~/Downloads/Filebase_Mac_App_Store.provisionprofile

                 Building it here instead is --universal beside --store.
USAGE
}

while [ $# -gt 0 ]; do
    case "$1" in
        --binary) binary="${2:?--binary needs a path}"; shift 2 ;;
        --outdir) outdir="${2:?--outdir needs a directory}"; shift 2 ;;
        --universal) universal=yes; shift ;;
        --sign) identity="${2:?--sign needs an identity}"; shift 2 ;;
        --store) store_profile="${2:?--store needs a .provisionprofile}"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "build-app.sh: unknown argument $1" >&2; usage >&2; exit 2 ;;
    esac
done

# One trap for everything this script makes, set before the first `mktemp` and
# never re-armed. A second `trap ... EXIT` replaces the first rather than
# adding to it, which is how slipcase-desktop left store temporaries behind.
stage=""
store_plist=""
store_ents=""
cleanup() {
    [ -z "$stage" ] || rm -rf "$stage"
    [ -z "$store_plist" ] || rm -f "$store_plist"
    [ -z "$store_ents" ] || rm -f "$store_ents"
}
trap cleanup EXIT INT TERM

# Everything --store needs is checked before anything is built, because the
# failures here are cheap to see now and expensive to see after an upload: a
# profile for the wrong bundle identifier, an expired one, or a certificate
# this machine does not hold all produce a package that assembles perfectly
# and is refused by App Store Connect.
if [ -n "$store_profile" ]; then
    [ -z "$identity" ] || {
        echo "build-app.sh: --store chooses its own identities; drop --sign" >&2
        exit 2
    }
    [ -f "$store_profile" ] || {
        echo "build-app.sh: no provisioning profile at ${store_profile}" >&2
        exit 1
    }
    # A Store binary runs on both architectures or half the machines that
    # bought it cannot run it, so this is not a flag a person should have to
    # remember.
    universal=yes

    # The profile is a CMS-signed property list. Decoding it is also the check
    # that it is one.
    store_plist=$(mktemp -t filebase-profile)
    security cms -D -i "$store_profile" > "$store_plist" 2>/dev/null || {
        echo "build-app.sh: ${store_profile} is not a provisioning profile this can read" >&2
        exit 1
    }

    # ISO 8601 rather than PlistBuddy's rendering, which is locale-dependent
    # and would make this check pass or fail by what language the machine is
    # in.
    store_expiry=$(plutil -extract ExpirationDate raw -o - "$store_plist" 2>/dev/null)
    store_expiry_at=$(date -j -u -f "%Y-%m-%dT%H:%M:%SZ" "$store_expiry" +%s 2>/dev/null || echo "")
    [ -n "$store_expiry_at" ] || {
        echo "build-app.sh: cannot read the profile's expiry date (${store_expiry:-none})" >&2
        exit 1
    }
    [ "$store_expiry_at" -gt "$(date +%s)" ] || {
        echo "build-app.sh: the profile expired on ${store_expiry}" >&2
        exit 1
    }

    # The team and the application identifier come out of the profile rather
    # than being written down here. The profile is the thing App Store Connect
    # validates against, so it is the only copy that cannot drift.
    store_app_id=$(/usr/libexec/PlistBuddy -c \
        'Print Entitlements:com.apple.application-identifier' "$store_plist" 2>/dev/null || echo "")
    store_team=$(/usr/libexec/PlistBuddy -c \
        'Print Entitlements:com.apple.developer.team-identifier' "$store_plist" 2>/dev/null || echo "")
    [ -n "$store_app_id" ] && [ -n "$store_team" ] || {
        echo "build-app.sh: the profile carries no application-identifier or team-identifier" >&2
        exit 1
    }
fi

# Cargo is asked where its target directory is. `[build] target-dir` in a
# Cargo configuration file moves it and no environment variable then says so.
target_dir=$(cd "$root" && cargo metadata --format-version 1 --no-deps |
    sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')

# A Store build has to run on both architectures, and Rosetta is not a plan
# Apple is keeping. `cargo build --release` writes to `release/`; asking for a
# target explicitly writes to `<triple>/release/`, so the two slices are built
# separately and joined here. `lipo` is the only step: nothing is compiled
# twice by this script and nothing is compiled at all.
# Only when nothing was handed over. `--binary` takes an executable built
# elsewhere and already joined - the Apple silicon runner attaches a universal
# one to the release, and that is the file this is given for a Store build, so
# there is nothing here to compile or to join. The architecture check below is
# what holds in either case, and it is asked of whatever is about to be
# packaged rather than only of what `lipo` wrote.
if [ "$universal" = yes ] && [ -z "$binary" ]; then
    slices=""
    for triple in x86_64-apple-darwin aarch64-apple-darwin; do
        slice="${target_dir}/${triple}/release/filebase"
        [ -x "$slice" ] || {
            echo "build-app.sh: no executable at $slice — run 'cargo build --release --target ${triple}' first" >&2
            exit 1
        }
        slices="${slices} ${slice}"
    done
    binary="${target_dir}/release/filebase-universal"
    # shellcheck disable=SC2086
    lipo -create ${slices} -output "$binary"
fi

if [ -z "$binary" ]; then
    binary="${target_dir}/release/filebase"
fi
[ -x "$binary" ] || {
    echo "build-app.sh: no executable at $binary — run 'cargo build --release' first" >&2
    exit 1
}

# A binary with one architecture would be a Store upload rejected days later,
# or worse, accepted and unrunnable on half the machines that bought it. Asked
# of whatever is about to be packaged rather than only of what `lipo` just
# wrote, because `--binary` takes one built elsewhere: the release binary is
# built on the runner and signed here, so the joining is no longer the only
# way a universal binary arrives.
if [ "$universal" = yes ] || [ -n "$store_profile" ]; then
    for triple in x86_64 arm64; do
        lipo -info "$binary" | grep -q "$triple" || {
            echo "build-app.sh: ${binary} has no ${triple} slice" >&2
            exit 1
        }
    done
fi

# **A private symbol in the binary is a rejection, and one cost slipcase-desktop
# a review cycle.** Its 0.1.1 was refused on 2026-08-31 for referencing
# `_CGSSetWindowBackgroundBlurRadius`, which arrived through `winit` and which
# neither application calls. Review scans the symbol table rather than the
# call graph, so *unreachable* is not *absent*. The workspace `Cargo.toml`'s
# `[patch.crates-io]` is what removes it, and this is what notices if it or
# anything like it comes back; before that patch this tree's own release
# binary carried it and `_CGSMainConnectionID` beside it.
#
# **The question it asks is a real one rather than a list of names.** For
# every undefined symbol the executable imports from a system *framework*,
# does that framework's own public headers declare it? That is exactly the
# line Apple draws: `CGShieldingWindowLevel` is in `CGDirectDisplay.h` and is
# fine, while the two `CGS` symbols appear in no header and only in
# `CoreGraphics.tbd`.
#
# Frameworks only. libSystem, libobjc and the rest are the compiler's own
# runtime, emitted rather than named by any source here and declared in no
# header by design; asking about them produced a dozen findings that were all
# noise. The whole `.framework` directory is searched and not just its
# `Headers`, because Carbon and CoreServices are umbrellas whose declarations
# live in sub-frameworks beneath them.
#
# **And the search follows symlinks**, because an umbrella's sub-frameworks are
# not all real directories. `winit` links `CGDisplayCreateUUIDFromDisplayID` and
# `CGDisplayGetDisplayIDFromUUID` through ApplicationServices, the public header
# declaring them is ColorSync's, and in every SDK on the Mac (14, 15.5, 26,
# 26.2) `ApplicationServices.framework/Versions/A/Frameworks/ColorSync.framework`
# is a symlink up to the top-level framework, which a plain `find` does not
# enter. Without `-L` the umbrella yields 59 headers and no `ColorSyncDevice.h`;
# with `-L`, 1281 headers and the declaration. Both symbols are public and are
# in the accepted Store bundle of Slipcase Desktop.
private_symbols() {
    exe="$1"
    sdk=$(xcrun --sdk macosx --show-sdk-path 2>/dev/null) || sdk=""
    # Not being able to ask is not the same as a clean answer, and a check
    # that goes quiet on the machine that lacks a tool is the one that lets a
    # build through. Refuse instead.
    [ -n "$sdk" ] && [ -d "$sdk" ] || {
        echo "build-app.sh: no macOS SDK, so the private-symbol check cannot run" >&2
        echo "  install the Xcode command line tools: xcode-select --install" >&2
        exit 1
    }
    scratch=$(mktemp -d)
    # `nm -m` names the library each undefined symbol is expected to come
    # from, which is what makes the per-framework question askable at all.
    # Both slices of a universal binary are listed, and a symbol in either is
    # a finding.
    nm -mu "$exe" 2>/dev/null |
        sed -n 's/.*(undefined) external _\{0,1\}\([A-Za-z0-9_]*\) (from \([A-Za-z0-9_+]*\)).*/\2 \1/p' |
        sort -u > "${scratch}/pairs"
    # An executable that imports nothing is not a clean answer, it is `nm`
    # having failed to read the file, and an empty list walks through every
    # check below it without a word. Refuse that rather than pass it.
    [ -s "${scratch}/pairs" ] || {
        echo "build-app.sh: nm read no imported symbols from ${exe}" >&2
        echo "  a Mach-O executable always imports some; this is not a pass" >&2
        rm -rf "$scratch"
        exit 1
    }
    : > "${scratch}/flagged"
    for framework in $(cut -d' ' -f1 "${scratch}/pairs" | sort -u); do
        dir="${sdk}/System/Library/Frameworks/${framework}.framework"
        [ -d "$dir" ] || continue
        awk -v f="$framework" '$1 == f { print $2 }' "${scratch}/pairs" |
            sort -u > "${scratch}/wanted"
        find -L "$dir" -name '*.h' -print0 2>/dev/null |
            xargs -0 grep -hoFw -f "${scratch}/wanted" 2>/dev/null |
            sort -u > "${scratch}/declared"
        comm -23 "${scratch}/wanted" "${scratch}/declared" |
            sed "s/^/${framework} /" >> "${scratch}/flagged"
    done
    if [ -s "${scratch}/flagged" ]; then
        echo "build-app.sh: the executable imports symbols no public header declares:" >&2
        sed 's/^/  /' "${scratch}/flagged" >&2
        echo "  App Store review refuses these as Guideline 2.5.1." >&2
        echo "  Find the crate with: grep -rn SYMBOL ~/.cargo/registry/src/*/" >&2
        rm -rf "$scratch"
        exit 1
    fi
    rm -rf "$scratch"
}
private_symbols "$binary"

# Two numbers, not one, and that is the whole reason `version.sh` takes an
# argument. `CFBundleShortVersionString` is what a person sees in the About
# box and is the release version. `CFBundleVersion` is what App Store Connect
# deduplicates uploads by: it must increase on *every* upload, including a
# rejected one resubmitted with no change, so it cannot be the release
# version.
version=$("${here}/../version.sh" --short)
build=$("${here}/../version.sh" --build)

app="${outdir}/Filebase.app"
rm -rf "$app"
mkdir -p "${app}/Contents/MacOS" "${app}/Contents/Resources"

# The application icon, rendered on Linux and committed.
#
# The fleet's older trees build the `.icns` here with `sips` and `iconutil`,
# which exist only on a Mac, so the icon cannot exist until the build reaches
# this machine. `packaging/make-icons` renders it with the `icns` crate
# instead, on whatever machine runs `cargo`, and the result is committed — so
# by the time anything gets here the icon is a file to check rather than a
# step to perform. ~/notes/mac_icons_on_linux.md measured that in odox.
#
# **One icon, where the document applications carry three.** A file type draws
# its icon by its own mechanism, so an application opening two kinds of file
# needs a drawing for each or one picture ends up on both. Filebase opens a
# folder and declares no file type, so there is nothing to draw but the
# application.
icns="${here}/../icons/filebase.icns"
[ -f "$icns" ] || {
    echo "build-app.sh: no icon at ${icns}; run 'cargo run --manifest-path packaging/make-icons/Cargo.toml'" >&2
    exit 1
}
# A bundle whose `.icns` is unreadable draws the generic application icon and
# says nothing about it, so the magic is checked rather than trusted.
magic=$(dd if="$icns" bs=1 count=4 2>/dev/null)
[ "$magic" = "icns" ] || {
    echo "build-app.sh: ${icns} is not an icns file" >&2
    exit 1
}
install -m 0644 "$icns" "${app}/Contents/Resources/filebase.icns"

sed -e "s/@VERSION@/${version}/g" -e "s/@BUILD@/${build}/g" \
    "${here}/Info.plist.in" > "${app}/Contents/Info.plist"
# A malformed property list is not an error Finder reports; it is a bundle
# that quietly does not associate. Parsed here so the failure is loud.
plutil -lint "${app}/Contents/Info.plist" >/dev/null

install -m 0755 "$binary" "${app}/Contents/MacOS/filebase"

# A released bundle's executable has to agree with the floor its property
# list declares, and Cargo's default does not: measured on this tree's first
# release build, the x86_64 slice said 10.12 while the bundle says 11.0.
# Finder would refuse to launch it below 11 and the binary would claim to run
# there. `MACOSX_DEPLOYMENT_TARGET` is what moves it, and this is the check
# that catches forgetting to set it.
#
# For every release path, which is `--universal` and anything bound for the
# Store. A plain `cargo build --release` for the local test loop is left alone,
# because failing the everyday bundle over a floor that only matters on
# somebody else's machine would be theatre.
if [ "$universal" = yes ] || [ -n "$store_profile" ]; then
    floor=$(plutil -extract LSMinimumSystemVersion raw "${app}/Contents/Info.plist")
    for arch in x86_64 arm64; do
        # Two shapes: a modern build emits LC_BUILD_VERSION with `minos`, and
        # an old enough deployment target emits LC_VERSION_MIN_MACOSX with
        # `version`. Both are read, so this cannot pass by finding neither.
        got=$(otool -arch "$arch" -l "${app}/Contents/MacOS/filebase" |
            awk '/LC_BUILD_VERSION|LC_VERSION_MIN_MACOSX/ {want=1; next}
                 want && ($1 == "minos" || $1 == "version") {print $2; exit}')
        [ "$got" = "$floor" ] || {
            echo "build-app.sh: the ${arch} slice was built for ${got:-nothing} and Info.plist declares ${floor} — rebuild with MACOSX_DEPLOYMENT_TARGET=${floor}" >&2
            exit 1
        }
    done
fi

# The Store path. Everything it needs was validated before the build; what is
# left is to put the profile inside the bundle, sign what a submission is
# signed with, and wrap it.
if [ -n "$store_profile" ]; then
    # The profile has to match the bundle it goes into. `application-identifier`
    # is `TEAMID.bundle-identifier`, so the tail of it is what Info.plist must
    # say; a profile for a neighbouring identifier signs perfectly and is
    # refused at upload.
    bundle_id=$(/usr/libexec/PlistBuddy -c 'Print CFBundleIdentifier' "${app}/Contents/Info.plist")
    [ "$store_app_id" = "${store_team}.${bundle_id}" ] || {
        echo "build-app.sh: the profile is for ${store_app_id} and this bundle is ${bundle_id}" >&2
        exit 1
    }

    # One identity or none, never a guess. Two certificates of the same kind
    # in one keychain is an ordinary state, an expiring one beside its
    # replacement, and picking whichever `grep` found first is how a package
    # gets signed with the wrong one.
    #
    # Counted by the certificate and not by the line. One certificate in two
    # keychains is listed once for each, and on a machine whose search list
    # has two it is listed once per pair - the same certificate four times,
    # which is not four certificates. `find-identity` prints the SHA-1 first
    # and that is the certificate, so two of a kind are still caught and two
    # sightings of one are not.
    find_identity() {
        matches=$(security find-identity -v 2>/dev/null |
            grep "$1: .*(${store_team})" |
            sed 's/^ *[0-9]*) *\([0-9A-Fa-f]*\) *"\(.*\)"$/\1 \2/' |
            sort -u)
        count=$(printf '%s' "$matches" | grep -c . || true)
        [ "$count" = 1 ] || {
            echo "build-app.sh: expected one \"$1\" certificate for team ${store_team}, found ${count}" >&2
            [ "$count" = 0 ] || echo "$matches" | sed 's/^/  /' >&2
            return 1
        }
        printf '%s' "$matches" | sed 's/^[0-9A-Fa-f]* //'
    }
    app_identity=$(find_identity "Apple Distribution") || exit 1
    # Apple's portal calls this Mac Installer Distribution; the certificate
    # calls itself something else, and the certificate is what `security`
    # reports. It also never appears under `-p codesigning`, because it signs
    # a package rather than code, which is why nothing here filters by that
    # policy.
    pkg_identity=$(find_identity "3rd Party Mac Developer Installer") || exit 1

    # Before the signature, because a signature covers what is in the bundle
    # when it is made and this is part of what gets covered.
    cp "$store_profile" "${app}/Contents/embedded.provisionprofile"

    # **Then strip every extended attribute off the bundle, and this is not
    # tidying.** App Store Connect refuses a package containing any file
    # marked `com.apple.quarantine`, ITMS-91109, and a profile is downloaded
    # from the developer portal in a browser, so it arrives marked. macOS `cp`
    # preserves extended attributes, so the mark rides into the bundle,
    # through the signature, through `productbuild`, and past `altool
    # --validate-app`. The upload is then accepted, ingestion rejects it hours
    # later by email, and nothing appears in App Store Connect at all.
    # slipcase-desktop measured it on 2026-08-29. The profile also carries
    # `kMDItemWhereFroms`, holding the portal URL with the team and profile
    # identifiers in it, which would otherwise ship inside the application.
    xattr -cr "$app"

    # The entitlements a Store build is signed with are not the ones a
    # development build is signed with, and this is generated rather than
    # committed so the team identifier has exactly one source: the profile.
    #
    # `keychain-access-groups` is deliberately absent. The profile grants it
    # and this application touches no keychain, and a capability asked for
    # and unused is a question at review with no good answer, the same rule
    # `AppxManifest.xml` follows about declaring only `runFullTrust`.
    #
    # The file access is **read-only**, which is where this diverges from the
    # rest of the family and has to stay in step with `Filebase.entitlements`
    # beside this script. Filebase never writes a container, so read-write
    # would be a capability the product does not have.
    store_ents=$(mktemp -t filebase-entitlements)
    cat > "$store_ents" <<ENTITLEMENTS
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>com.apple.security.app-sandbox</key>
	<true/>
	<key>com.apple.security.files.user-selected.read-only</key>
	<true/>
	<key>com.apple.application-identifier</key>
	<string>${store_app_id}</string>
	<key>com.apple.developer.team-identifier</key>
	<string>${store_team}</string>
</dict>
</plist>
ENTITLEMENTS

    codesign --force --timestamp --options runtime \
        --sign "$app_identity" \
        --entitlements "$store_ents" \
        "$app"

    # Read back rather than trusted, for both of them. The sandbox one is the
    # failure that costs a day; the identifier one is the failure that costs
    # an upload, and neither is visible by looking at the bundle.
    granted=$(codesign -d --entitlements - --xml "$app" 2>/dev/null |
        plutil -extract 'com\.apple\.security\.app-sandbox' raw - 2>/dev/null)
    [ "$granted" = true ] || {
        echo "build-app.sh: the Store signature carries no app-sandbox entitlement" >&2
        exit 1
    }
    # And refuse if anything is still marked, because the cost of finding out
    # later is an upload, a wait, and an email.
    #
    # `find -exec` rather than `xargs`: xargs answers 123 when the command it
    # ran was false, which is the *normal* case here, and under `set -eu` that
    # kills the substitution and the script with it, silently.
    marked=$(find "$app" -type f -exec sh -c \
        'xattr -p com.apple.quarantine "$1" >/dev/null 2>&1 && echo "$1"' _ {} \;)
    [ -z "$marked" ] || {
        echo "build-app.sh: files in the bundle carry com.apple.quarantine, which" >&2
        echo "  App Store Connect refuses as ITMS-91109:" >&2
        printf '  %s\n' $marked >&2
        exit 1
    }

    granted=$(codesign -d --entitlements - --xml "$app" 2>/dev/null |
        plutil -extract 'com\.apple\.application-identifier' raw - 2>/dev/null)
    [ "$granted" = "$store_app_id" ] || {
        echo "build-app.sh: the Store signature says application-identifier ${granted:-nothing}, not ${store_app_id}" >&2
        exit 1
    }
    [ -f "${app}/Contents/embedded.provisionprofile" ] || {
        echo "build-app.sh: the signed bundle carries no embedded.provisionprofile" >&2
        exit 1
    }
    codesign --verify --deep --strict "$app" || {
        echo "build-app.sh: the signed bundle does not verify" >&2
        exit 1
    }
    echo "signed ${app} for the Store with ${app_identity}"

    # `--component … /Applications` is where the Store installs it.
    # `productbuild` rather than `pkgbuild`: the first makes a distribution
    # package, which is what the upload takes, and the second makes a
    # component package, which it does not.
    pkg="${outdir}/Filebase.pkg"
    productbuild --component "$app" /Applications --sign "$pkg_identity" "$pkg" >/dev/null
    pkgutil --check-signature "$pkg" | sed -n '1,3p'
    echo "built ${pkg} signed with ${pkg_identity}"

    # Launch Services must not know this bundle. It cannot run on this
    # machine: AMFI refuses its restricted entitlements without a profile
    # covering the Mac, and a Store profile covers none (README.md). Launch
    # Services does not ask whether a bundle can launch before choosing it,
    # and among copies of one identifier it prefers the newer version, so a
    # submission build sitting here is a handler candidate. slipcase-desktop
    # measured it on 2026-09-04: every double-click launched the Store build
    # and the kernel killed it, one crash report per attempt and no window.
    # What registers a bundle is a hand-off, `lsregister -f` on a development
    # build at this same path being the usual one, and the claim survives
    # `rm -rf` and a rebuild. This withdraws it.
    #
    # `-u` exits 1 with -10814 when the bundle was never registered, which is
    # the usual state straight after a build, so its status is not the
    # script's.
    "$lsregister" -u "$app" >/dev/null 2>&1 || true

    echo
    echo "validate without uploading, then upload; both need the App Store Connect"
    echo "API key in ~/.appstoreconnect/private_keys and its issuer id:"
    echo "  xcrun altool --validate-app -f ${pkg} -t macos --apiKey KEY_ID --apiIssuer ISSUER_ID"
    echo "  xcrun altool --upload-app   -f ${pkg} -t macos --apiKey KEY_ID --apiIssuer ISSUER_ID"
    exit 0
fi

# Last, so that nothing this script writes lands inside the bundle after it
# has been sealed. A signature covers what is there when it is made, and
# adding a file afterwards is how a bundle becomes one macOS reports as
# damaged.
if [ -n "$identity" ]; then
    codesign --force --timestamp=none \
        --sign "$identity" \
        --entitlements "${here}/Filebase.entitlements" \
        "$app"
    # A signature that did not carry the entitlements is the failure that
    # costs a day: the bundle launches, behaves exactly as an unsigned one
    # does, and every sandbox measurement made against it is quietly
    # meaningless.
    #
    # The dots in the key are escaped because `plutil -extract` reads an
    # unescaped one as a key path separator, so the plain spelling looks for
    # five nested dictionaries, fails, and reports a correctly signed bundle
    # as unsigned.
    granted=$(codesign -d --entitlements - --xml "$app" 2>/dev/null |
        plutil -extract 'com\.apple\.security\.app-sandbox' raw - 2>/dev/null)
    [ "$granted" = true ] || {
        echo "build-app.sh: the signature carries no app-sandbox entitlement" >&2
        exit 1
    }
    echo "signed ${app} with ${identity}"
fi

echo "built ${app} from ${binary}"
echo
echo "run it against a folder of containers:"
echo "  open -a ${app} --args /path/to/a/folder"
echo
echo "Filebase declares no document type, so there is nothing to register with"
echo "Launch Services and nothing for a double-click to find. It is handed a"
echo "folder as an argument, which is what the Store screenshot driver does"
echo "through AS_ARGS in packaging/macos/shots.sh."
