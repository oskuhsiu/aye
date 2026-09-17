# aye tools

Each tool lives in its own directory. The existing Rust CLI is in [aye/](aye/);
future graphical tools can be added alongside it.
All tools share the repository's root `.git/`, source history and aye task state.

| Directory | Contents |
| --- | --- |
| [aye/](aye/README.md) | Git-native task manager CLI, Cargo package and development commands |
| [aye/skill/](aye/skill/README.md) | Portable agent skill and human installation guide |
| [aye/verification/](aye/verification/CASES.md) | Behavior cases and executable verification scripts |

From this repository root:

```sh
cargo install --path aye --force --locked
make -C aye test
```

Cargo uses its configured/default installation location, normally `~/.cargo/bin`.
Generated `aye/target/` build output and `aye/test/` fixtures are ignored and may
be deleted; builds and tests recreate them. Keep the verification scripts.
