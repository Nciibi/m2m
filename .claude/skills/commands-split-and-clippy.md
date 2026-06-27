# Skill: Split a Rust Monolith + Clean Clippy Warnings

## When to use
- A single `commands.rs` (or similar handler file) has grown past 1000+ lines
- The file handles multiple domains (vault, chat, files, network, settings)
- The project has accumulated clippy warnings across the codebase

## Process

### Step 1: Map the module boundaries
1. Read the monolith and classify every function by domain
2. Count Tauri `#[tauri::command]` annotations to know the register surface
3. Identify shared types (response structs, events) and helpers (utility functions)
4. Identify cross-file references (e.g. `crate::commands::resolve_local_ip`)

### Step 2: Create the module directory structure
```
src-tauri/src/commands/
    mod.rs    — shared types + pub use re-exports
    vault.rs
    chat.rs
    files.rs
    network.rs
    settings.rs
    forwards.rs
    util.rs    — pure helpers with no Tauri dependency
```

### Step 3: File creation order (critical)
Create files in dependency order to avoid broken intermediates:
1. `mod.rs` (types only — no imports needed)
2. `util.rs` (pure functions — no crate imports beyond std/lib)
3. Domain modules (`vault.rs`, `chat.rs`, `files.rs`, `settings.rs`, `forwards.rs`)
4. Hub module (`network.rs` — contains the receive loop that references `files::send_file_chunks`)
5. Update `lib.rs` with new module paths
6. Update any remaining cross-references (e.g. `port_mapping.rs` calling `commands::resolve_local_ip`)
7. Delete the old monolith
8. Build and fix compilation errors iteratively
9. Run `cargo clippy -- -D warnings` and fix every error

### Step 4: Cross-module visibility
- Functions called from the receive loop (e.g. `send_file_chunks`) need `pub(super)` visibility
- Sub-modules access shared types via `use super::*` or `use super::TypeName`

### Step 5: Clippy pass patterns
When `cargo clippy -- -D warnings` fires, these are the common fixes:

| Clippy lint | Fix |
|---|---|
| `needless_borrow` | `Some(ref x)` → `Some(x)` when pattern is already a reference |
| `redundant_closure` | `.map(\|r\| f(r))` → `.map(f)` |
| `manual_div_ceil` | `(a + b - 1) / b` → `a.div_ceil(b)` |
| `manual_split_once` | `s.splitn(2, ':').nth(1)` → `s.split_once(':').map(\|x\| x.1)` |
| `unwrap_or_default` | `map.entry(k).or_insert_with(VecDeque::new)` → `or_default()` |
| `empty_line_after_doc_comments` | Remove blank line between `///` comment and the item it documents |
| `op_ref` | `&a != &b` → `a != b` |
| `let_unit_value` | `let _x = expr?;` → `expr?;` |
| `needless_question_mark` | `Some(expr?)` → `expr` (if expr returns Option) |
| `type_complexity` | Add `#[allow(clippy::type_complexity)]` on the function |
| `dead_code` | Add `#[allow(dead_code)]` with rationale comment, OR remove the code |

### Dead code philosophy
- Code that is **architecturally designed but not yet wired** → `#[allow(dead_code)]` with inline comment referencing the future phase
- Code that is **actually unused and not planned** → delete it
- Code that is **used only in tests** → gate with `#[cfg(test)]`
