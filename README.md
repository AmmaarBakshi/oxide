# ⚗️ Oxide Shell

A custom, high-performance shell written entirely in **Rust**. Oxide builds a complete command pipeline from scratch—lexing, parsing, and execution—without relying on existing system shell implementations.

> **Why Oxide?** Instead of wrapping existing system calls, Oxide implements its own complete pipeline from scratch: it reads raw text, tokenizes it through a custom Lexer, builds an Abstract Syntax Tree (AST) via its Parser, and routes commands through a dedicated Execution Engine.

---

## 📖 Table of Contents

- [Features](#-features)
- [Architecture](#-architecture)
- [Getting Started](#-getting-started)
- [Usage Examples](#-usage-examples)
- [Project Structure](#-project-structure)
- [Documentation](#-documentation)
- [Contributing](#-contributing)
- [Roadmap](#-roadmap)
- [License](#-license)

---

## ✨ Features

### Implemented
- ✅ **Custom Lexing & Parsing** — Accurately tokenizes and parses strings, arguments, and shell operators
- ✅ **Process Execution** — Spawns and manages OS-level child processes efficiently
- ✅ **Built-in Commands** — Native interception for commands that modify shell state (`cd`, `echo`, etc.)
- ✅ **Output Redirection** — Route standard output to files using the `>` operator (e.g., `echo "hello" > file.txt`)
- ✅ **Dynamic Prompt** — Real-time tracking and display of the current working directory
- ✅ **Modular Architecture** — Cleanly decoupled components for easy extension

### Planned
- 🔄 Piping (`|`) support
- 🔄 Input redirection (`<`) and append (`>>`)
- 🔄 Environment variable expansion
- 🔄 Command history and autocompletion
- 🔄 Script execution

---

## 🏗️ Architecture

Oxide is organized as a **Cargo Workspace** to maintain strict modularity and clear separation of concerns:

| Component | Purpose |
|-----------|---------|
| **`oxide-cli`** | Entry point and terminal UI layer |
| **`oxide-core`** | Main execution engine and state manager (REPL loop) |
| **`oxide-parser`** | Lexer (tokenization) and Parser (grammar and AST generation) |
| **`oxide-builtins`** | Built-in commands (`cd`, `echo`, etc.) |
| **`oxide-exec`** | Command execution and process management |
| **`oxide-config`** | Configuration and settings handling |
| **`oxide-utils`** | Shared utilities and helpers |

For detailed architecture insights, see [docs/architecture.md](docs/architecture.md).

---

## 🚀 Getting Started

### Prerequisites

- **Rust 1.70+** ([Install Rust](https://rustup.rs/))
- **Cargo** (included with Rust)
- **On Windows:** MSVC build tools

### Installation

```bash
# Clone the repository
git clone https://github.com/AmmaarBakshi/oxide.git
cd oxide

# Build and run the shell
cargo run -p oxide-cli

# Or build a release binary (optimized)
cargo build --release -p oxide-cli
./target/release/oxide-cli
```

### Platform-Specific Notes

**Windows:** Ensure you have MSVC build tools installed. You can install them via Visual Studio or the [Build Tools](https://visualstudio.microsoft.com/downloads/).

**Linux/macOS:** Standard Rust installation is sufficient.

---

## 💡 Usage Examples

### Basic Commands
```bash
oxide> echo "Hello, Oxide!"
Hello, Oxide!

oxide> pwd
/home/user

oxide> cd /tmp
oxide> pwd
/tmp
```

### Output Redirection
```bash
oxide> echo "Project Status" > status.txt
oxide> cat status.txt
Project Status
```

### Command Execution
```bash
oxide> ls -la
oxide> whoami
oxide> date
```

---

## 📁 Project Structure

```
oxide/
├── crates/                    # Cargo workspace members
│   ├── oxide-cli/            # Terminal interface
│   ├── oxide-core/           # Core execution engine
│   ├── oxide-parser/         # Lexer and parser
│   ├── oxide-builtins/       # Built-in commands
│   ├── oxide-exec/           # Process execution
│   ├── oxide-config/         # Configuration
│   └── ...                   # Other specialized crates
├── docs/                      # Documentation
│   ├── architecture.md       # System design
│   ├── grammar.md            # Shell grammar specification
│   └── ...
├── tests/                     # Integration and unit tests
├── assets/                    # Configuration templates and themes
└── Cargo.toml                # Workspace configuration
```

---

## 📚 Documentation

Comprehensive documentation is available in the [`docs/`](docs/) directory:

- **[Architecture](docs/architecture.md)** — System design and component interaction
- **[Grammar](docs/grammar.md)** — Shell syntax specification
- **[Configuration](docs/config.md)** — Configuration options and customization
- **[Performance](docs/performance.md)** — Benchmarks and optimization notes
- **[Scripting](docs/scripting.md)** — Script execution guide
- **[Security](docs/security.md)** — Security considerations
- **[Plugin API](docs/plugin-api.md)** — Extending Oxide with plugins

---

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on:

- Reporting bugs
- Submitting feature requests
- Code style and standards
- Pull request process

---

## 🗺️ Roadmap

For planned features, milestones, and project direction, see [ROADMAP.md](ROADMAP.md).

---

## 📋 License

Oxide Shell is licensed under the [LICENSE](LICENSE) file. See it for details.

---

**Made with ⚗️ by Oxide contributors**