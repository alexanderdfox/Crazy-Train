# stage – Recursive Cross-User File Stager

Stages files and directories into a world-readable/writable shared workspace (`/tmp/fox_shared_space`) so any user on the machine can access them.

Control-flow shape intentionally mirrors [aip.py](https://github.com/alexanderdfox/AIP/blob/main/aip.py) / sshp:

| Concept (aip / sshp)     | This tool                          |
|--------------------------|------------------------------------|
| Target list              | `Target { id, source, rel }`       |
| Child + State            | `Child` + `State` enum             |
| max-jobs concurrency     | `tokio::sync::Semaphore`           |
| Modes                    | `Files` / `Tree`                   |
| Dry-run / debug          | `-n` / `-d`                        |
| Aggregate exit status    | `0` if all ok, else `1`            |

## Features

- Recursive directory staging (default)
- Concurrent copies with hard job limit (`-m`)
- Preserves relative tree structure under the shared root
- World permissions: directories `0777`, files `0666`
- Dry-run mode
- File list input (`-f`) or stdin (`-f -`)
- Defaults to staging the current directory (works “on itself”)

## Build

Requires **Rust 1.70+** (edition 2021):

```bash
cargo build --release
# binary: target/release/stage
```

## Usage

```
stage [OPTIONS] [PATHS]...
```

### Options

| Flag | Description |
|------|-------------|
| `-f, --file <FILE>` | Read paths from file (one per line). Use `-` for stdin |
| `-m, --max-jobs <N>` | Max concurrent copy jobs (default: 8) |
| `-r, --recursive` | Recurse into directories (default: true) |
| `--no-recursive` | Disable recursion |
| `--mode <MODE>` | `tree` (default) or `files` |
| `-n, --dry-run` | Show what would be staged, do not copy |
| `-d, --debug` | Verbose progress on stderr |
| `-h, --help` | Print help |
| `-V, --version` | Print version |

### Modes

- **`tree`** (default) – preserves directory structure under `/tmp/fox_shared_space`
- **`files`** – stages only regular files (directories are skipped as targets)

## Examples

```bash
# Stage the current directory recursively (default behaviour)
./target/release/stage

# Stage specific paths with higher concurrency
./target/release/stage -m 16 src/ Cargo.toml README.md

# Dry-run
./target/release/stage -n -d

# Paths from a file
./target/release/stage -f paths.txt

# Paths from stdin
find . -name '*.rs' | ./target/release/stage -f -

# Files-only mode
./target/release/stage --mode files -r /some/project
```

## Output

On success each staged destination path is printed to stdout:

```
/tmp/fox_shared_space/src/main.rs
/tmp/fox_shared_space/Cargo.toml
...
```

Exit codes:

- `0` – everything staged successfully  
- `1` – one or more failures  
- `2` – usage / no targets  

## Layout

```
src/
  main.rs          # clap CLI + main_loop + staging logic
Cargo.toml
.gitignore
README.md
```

## Notes

- The shared root `/tmp/fox_shared_space` is created automatically and given `0777` permissions.
- Intermediate directories created during staging also receive `0777`.
- Files receive `0666`.
- Symlinks are not followed (`WalkDir::follow_links(false)`).
- The tool is intentionally single-purpose and dependency-light (clap, tokio, walkdir).
```