# ⚗️ Oxide Shell

Oxide is a custom, high-performance shell written entirely in Rust. 

Instead of just wrapping existing system calls, Oxide implements its own complete pipeline from scratch: it reads raw text, tokenizes it through a custom Lexer, builds an Abstract Syntax Tree (AST) via its Parser, and routes commands through a dedicated Execution Engine.

## 🏗️ Architecture
This project is structured as a Cargo Workspace to keep components strictly decoupled and modular:

* **`oxide-cli`**: The entry point and terminal UI.
* **`oxide-core`**: The main execution engine and state manager (REPL loop).
* **`oxide-parser`**: The brain of the shell. Contains the Lexer (tokenization) and Parser (grammar and AST generation).
* **`oxide-builtins`**: Internal commands that must be executed by the shell itself rather than the OS (e.g., `cd`, `echo`).

## ✨ Features (So Far)
- [x] **Custom Lexing & Parsing:** Accurately parses strings, arguments, and shell operators.
- [x] **Process Execution:** Spawns and manages OS-level child processes.
- [x] **Built-in Commands:** Native interception for commands that modify shell state (like `cd` and `echo`).
- [x] **Output Redirection:** Route standard output to files using the `>` operator (e.g., `echo "hello" > file.txt`).
- [x] **Dynamic Prompt:** Real-time tracking of the current working directory.

## 🚀 Getting Started

Ensure you have the Rust toolchain installed (and MSVC build tools if on Windows).

```bash
# Clone the repo
git clone https://github.com/AmmaarBakshi/oxide.git (https://github.com/AmmaarBakshi/oxide.git)
cd oxide

# Build and run the shell
cargo run -p oxide-cli