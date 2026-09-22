# Filebase

A desktop window over a directory of [Slipcase](https://slipcaseformat.org) containers. Point it at a folder, ask a question in [SlipQL](https://github.com/excelano/slipql), and the containers that answer come back as rows — one row per container, one column per flyleaf key. Select a row and the container's content file and its full flyleaf are beside it; press Open and the content file goes to whatever the system opens that kind of file with.

It reads. It never writes a container, and there is no Save in it: [Slipcase Desktop](https://github.com/excelano/slipcase-desktop) is the editor, and [Tommy Flyleaf](https://github.com/excelano/flyleaf) is the flyleaf editor both of them draw with.

## Why

A container carries a TOML document describing its content file, and that description travels with the file. Once a folder holds a few hundred of them, the question stops being *what is in this file* and becomes *which of these files say this* — and a file manager cannot answer it, because to a file manager they are all ZIP archives with the same icon.

Unpacking every container to find out, or keeping an index that goes stale the moment somebody copies a file in, both defeat the point of a flyleaf that lives with the document. Filebase keeps no index and no state: every query is a fresh scan, so the answer is what is on disk now, and the flyleaf is read in place without unpacking a content file.

## The query

The language is SlipQL, and Filebase embeds the same crate the `slipql` command runs, rather than reimplementing it — which is why the language was written first. Its `GRAMMAR.md` is the reference, and the short version is that clauses come from SQL and literals come from TOML:

```text
select @path, title, governance.owner where status = "draft" or tags contains "legal"
```

The chosen folder supplies `from`, so a query in the window can leave it out. Tick **recursive** to descend into subdirectories; a scan does not descend unless asked. `select *` gives `@path` and then every flyleaf key across the containers that answered, which is the quickest way to learn what a folder has to query against.

A key a container does not have renders as an empty cell rather than failing the query, because flyleaf keys are ad hoc by design. A file that cannot be read is skipped and reported under the rows, and so is a comparison that crossed types — `pages > 10` against a container whose `pages` is a word is neither true nor false, and the count of them is on screen rather than swallowed.

## The window

The folder and the query are at the top, the rows fill the window, and the selected container is on the right: its content file's name and size, a button that hands the content file to the system, and its flyleaf as a tree. Rows appear as the scan finds them rather than when it finishes, and starting another query stops the one before it.

A folder can also be named on the command line, which is what a file manager passes when it is asked to open a folder with this. There are no flags: `slipql` is the command-line interface, over the same engine.

## Building

`cargo build --release`. It needs a Rust toolchain and nothing else — nothing here compiles C, and `Cargo.toml` says what each dependency is for and what was measured about it.

## License

MIT. See [LICENSE](LICENSE).
