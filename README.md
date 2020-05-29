# blocked

```toml
// Cargo.toml
blocked = "0.1"
```

This macro takes an issue pattern and an optional 'reason'.

When the `BLOCKED_GITHUB_API_KEY` environment variable is found this macro will attempt to find the status of the referenced issue.
If the issue has been closed blocked will emit a warning containing the optional 'reason'.

Because this requires network access, it is recommended this is only run in CI builds so as to not slow down the edit-run-debug cycle.

```rust
// An attribute-like procedural macro is on the todo-list
#![feature(proc_macro_hygiene)]

use blocked::blocked;

fn hacky_workaround() {}

fn main() {
    blocked!("1", "This code can be removed when the issue is closed");
    hacky_workaround();

    // The reason is optional
    blocked!("1");
}
```

## Issue patterns

The following issue specifiers are supported (Github only for now)
* `#423` or `423`. Repository and organisation are pulled from the upstream or origin remote if they exist.
* `serde#423` or `serde/423` Organisation is pulled from upstream or origin remote if they exist.
* `serde-rs/serde#423` or `serde-rs/serde/423`
* `http(s)://github.com/serde-rs/serde/issues/423`
