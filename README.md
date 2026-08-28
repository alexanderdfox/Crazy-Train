# Train – Bi-directional Process Pipeline

A lightweight Rust “train” that chains external programs (binaries, shell commands, or compiled Rust filters) together.  
Each line in `cars.txt` becomes a car. Data travels forward through the cars; status / results can travel backward.  
Supports both **straight-line** and **loop** (circular) topologies.

## Features

- One external command per line in `cars.txt`
- Forward data flow + backward feedback
- Optional circular mode (`loop`)
- Clean shutdown with `Terminate` message
- Works with any stdin/stdout program (Rust binaries, Python, shell tools, etc.)

## Quick Start

```bash
# 1. Create cars.txt
cat > cars.txt << 'EOF'
# Comments and blank lines are ignored
cat
tr 'a-z' 'A-Z'
sed 's/BINARY/PROCESSED/'
EOF

# 2. Run in straight-line mode (default)
cargo run

# 3. Run in loop mode
cargo run -- loop
```

## Command-line Usage

```text
cargo run [-- loop] [cars_file]
```

| Argument       | Meaning                                      |
|----------------|----------------------------------------------|
| *(none)*       | Straight-line mode, use `cars.txt`           |
| `loop`         | Circular mode (last car feeds back to engine)|
| `mycars.txt`   | Use a different cars file                    |
| `loop mycars.txt` | Loop mode + custom file                   |

Any argument containing the substring `loop` (case-insensitive) enables circular mode.

## cars.txt Format

```text
# This is a comment
cat
tr 'a-z' 'A-Z'

# Full path to a binary
./target/release/myfilter

# Shell one-liner
python3 -c 'import sys; print(sys.stdin.read().upper())'

# Another Rust binary
/usr/local/bin/my_rust_tool
```

- Empty lines and lines starting with `#` are ignored.
- Each remaining line is executed with `sh -c`.
- The program must read from **stdin** and write to **stdout**.

## Using Rust Code as a Car

Rust source is **not** executed directly. Compile it first:

```bash
# filter.rs
use std::io::{self, Read, Write};

fn main() {
    let mut data = Vec::new();
    io::stdin().read_to_end(&mut data).unwrap();
    let upper = String::from_utf8_lossy(&data).to_uppercase();
    io::stdout().write_all(upper.as_bytes()).unwrap();
}
```

```bash
rustc -O filter.rs -o filter
```

Then add to `cars.txt`:

```text
./filter
```

## Architecture

```
Engine  →  Car 1  →  Car 2  →  …  →  Car N
   ↑                                   │
   └────────── (loop mode only) ───────┘
```

- **Straight mode**: data flows 1 → N, last car sends a `Backward` status message.
- **Loop mode**: last car sends its output back to the engine as a `Forward` message.

Each car is an independent OS process kept alive for the lifetime of the train.  
Data is written to its stdin; the result is read from its stdout.

## Example Session

```bash
$ cargo run
Loading cars from 'cars.txt' – mode: STRAIGHT
Train has 3 cars:
   1. cat
   2. tr 'a-z' 'A-Z'
   3. sed 's/BINARY/PROCESSED/'
Car 1 (cat) received 27 bytes
Car 2 (tr 'a-z' 'A-Z') received 27 bytes
Car 3 (sed 's/BINARY/PROCESSED/') received 27 bytes
Feedback: Car 3 (sed 's/BINARY/PROCESSED/') finished – 30 bytes
Train stopped.
```

## Building

```bash
cargo build --release
./target/release/train          # straight
./target/release/train loop     # circular
```

## License

MIT
```
