# Typecheck harness

Compiles all 50 modules of the M2M app — including the entire Tauri command
layer — with `cargo check`, on a machine that cannot build the real `tauri`
crate.

```bash
./check.sh --run
```

## Why

`cargo check` in `src-tauri/` fails before it reaches a single line of this
project's code:

```
glib-sys / gio-sys / gdk-sys build scripts → need GTK development packages
                                          → `.pc` files absent
                                          → only the GTK *runtime* is in the nix store
```

So without this harness, the backend is not typechecked at all in this
environment. Changes go in unverified, and mistakes only surface at release
time — or never.

## How

`check.sh` copies the live `src-tauri/src/` into `build/`, strips what a plain
crate cannot provide, and points the result at `tauri_stub/`:

| Real tauri | Handled by |
|---|---|
| `tauri` crate | `tauri_stub/` — same API surface, no GTK |
| `tauri-plugin-{dialog,notification,updater}` | rewritten to `tauri::plugin_*` modules in the stub |
| `#[tauri::command]` / `#[tauri::main]` | attribute macros — needs a proc-macro crate, so the lines are deleted |
| `include_bytes!("../icons/icon.png")` | the icon is copied alongside |

`build/Cargo.toml` is **generated** from `src-tauri/Cargo.toml` on every run,
minus the four GTK-coupled crates. There is no hand-maintained dependency list
to drift. `build/` is gitignored and can be deleted at any time; `tauri_stub/`
and `check.sh` are the only source of truth.

## What it can and cannot tell you

**Reliable.** Type errors, trait resolution, exhaustiveness, borrow checking —
all of it, across the whole backend. Several real bugs have been caught by this
harness that `tsc` and the frontend tests could not see.

**Three known limits.** Each has already produced a wrong answer once, so check
the real crate when it matters:

1. **`#[tauri::command]` bodies are not analysed the way Tauri analyses them.**
   The stub's `generate_handler!` emits `let _ = <path>;` for each command, which
   marks them used so dead-code analysis stays honest. If a command is added to
   `generate_handler!` in `lib.rs` it will typecheck; if you *forget* to add it,
   the harness will not notice that the command is unreachable. Tauri would
   still build fine — the command is simply never called.

2. **`Manager` vs inherent methods must match the real crate exactly.** The
   stub deliberately gives `AppHandle` *no* inherent `state` / `try_state` /
   `get_webview_window`, because in tauri 2.11.4 those are `Manager` trait
   methods (`src/lib.rs:729, 744, 576`). If they were inherent, every module
   using them without `use tauri::Manager` would typecheck here and fail against
   the real crate. `exit` *is* inherent (`src/app.rs:574`) and is stubbed as
   such. When upgrading tauri, re-verify these against the real source rather
   than trusting the stub.

3. **Nothing here can be executed.** `Manager::state` fabricates a
   `NonNull::dangling` reference, because the harness has nowhere to put a
   managed value. It is never dereferenced — `check.sh` only ever runs
   `cargo check`, which does not execute code. This is also why the harness
   cannot run the app's own `#[cfg(test)]` tests: the 378 backend tests still
   need a GTK-capable machine.

## ⚠️ Never symlink the live sources into here

An earlier rlib-based harness created symlinks from `src/*.rs` to the live
`src-tauri/src/*.rs`. A later `sed ... >` redirect wrote *through* those
symlinks and truncated all 20 live source files to 0 bytes. They were
recoverable from git only because nothing had been committed since.

`check.sh` copies real files and refuses to run if it finds a symlink under
`src-tauri/src`. Keep it that way.
