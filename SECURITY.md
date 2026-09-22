# Security Policy

## Reporting a vulnerability

Please report suspected vulnerabilities privately through GitHub Security Advisories at https://github.com/excelano/filebase/security/advisories/new. If you would rather not use GitHub, email david.anderson@excelano.com instead. I aim to respond within seven days.

Please do not open public issues for security problems.

## Supported versions

The latest release receives security fixes. Older versions are not supported.

## What Filebase can access

Filebase is a desktop application that runs locally on your machine. It lists the folder you point it at, opens every file there whose extension is `.slpc`, reads the flyleaf member out of each through the `slpc` library, and closes it. It makes no network calls of any kind: there is no server behind it, no account, no analytics, no telemetry and no crash reporting. It can only read files your operating-system user already has access to.

**It never writes a container.** There is no Save in the application and no code path that rewrites one. The flyleaf tree you see is drawn by the shared editor widget under a policy that marks every key read-only, which is what stops the widget from editing a document it is perfectly capable of editing.

The flyleaf member is decompressed before it can be parsed, so a container is a claim on memory before it is known to be one. The `slpc` library bounds that read, and a container over the bound is skipped and reported rather than read.

A scan is also a claim on memory in this application in a way it is not in the `slipql` command, because a window holds its rows where a pipe forgets them. A run stops at five thousand rows and says on screen that it did, rather than filling memory with an answer nobody asked for.

## Handing a content file to another application

Pressing **Open content file** extracts that container's content file and asks the operating system to open it. Two things about that are worth stating plainly.

The content file is written to a directory of this process's own, created with mode 0700 on Unix and removed with its contents when the application exits. Nothing is ever written beside the container you are reading. Where the content file lands is decided by `slpc`, not by the name inside the container, so a member named `../elsewhere` or an absolute path cannot escape that directory.

What happens next is the operating system's decision, not this application's. Filebase carries no table of file types and runs no program of its own choosing; it hands over a path and the platform opens whatever it has registered. **A content file is somebody else's file, and opening one is as safe as opening it from anywhere else, which is to say it depends on what it is and what opens it.** The content file's name is shown escaped on the card, so a name carrying a direction override cannot make an executable read as a document.

## What Filebase stores

Nothing that persists. There is no settings file, no recent-folders list, no cache and no index — every query is a fresh scan of what is on disk now, which is why there is no stale state to go wrong. The only thing written anywhere is the temporary content file described above, and it goes when the application does.

## Verifying releases

Every GitHub release includes the `.deb` packages built by GitHub Actions from the tagged commit. The workflow that builds them is `.github/workflows/linux.yml` in this repository, and it is public and auditable: it builds the package, checks that it declares the architecture its executable actually is, runs lintian at error and warning with no overrides, installs it, and confirms the executable loads.

Packages served from the Excelano apt repository are signed; https://excelano.com/apt/ has the key and the one-line setup.
