# Qi Language Server

Language Server Protocol (LSP) implementation for the Qi programming language.

## Overview

The Qi Language Server provides intelligent language features for Qi source code in editors and IDEs that support the Language Server Protocol. It integrates with the Qi compiler to provide:

- **Syntax highlighting** and **code completion** for Chinese keywords
- **Real-time error checking** with diagnostic messages
- **Go to definition** and **find references** functionality
- **Hover information** for symbols and keywords
- **Code formatting** according to Qi style conventions
- **Full UTF-8 support** for Chinese characters

## Features

### 🚀 Language Support
- 100% Chinese keyword support (函数, 变量, 如果, 当, 等)
- Chinese type system (整数, 浮点数, 字符串, 布尔, 等)
- Mixed Chinese/English identifier support
- UTF-8 text handling and positioning

### 🔧 Editor Features
- **Code Completion**: Intelligent suggestions for keywords, types, and symbols
- **Diagnostics**: Real-time syntax and semantic error checking
- **Hover**: Documentation and type information on hover
- **Go to Definition**: Navigate to symbol definitions
- **Find References**: Locate all uses of a symbol
- **Formatting**: Automatic code formatting with configurable style

### 🏗️ Architecture
- Built on Rust's async runtime for high performance
- Integrates directly with the Qi compiler's parser and AST
- Efficient document management with Rope data structure
- Comprehensive error handling and logging

## Installation

### Building from Source

```bash
# Clone the repository
git clone https://github.com/qi-lang/qi-compiler.git
cd qi-compiler/qi-lsp

# Build the language server
cargo build --release

# The binary will be available at target/release/qi-lsp
```

### Requirements

- Rust 1.75 or later
- Qi compiler (for dependency resolution)

## Configuration

### Editor Setup

#### Visual Studio Code

Add to your `settings.json`:

```json
{
  "languageServers": [
    {
      "name": "qi-lsp",
      "command": "/path/to/qi-lsp",
      "filetypes": ["qi"]
    }
  ]
}
```

Or use the official Qi VS Code extension (when available).

#### Neovim

```lua
lspconfig.qi_lsp.setup {
  cmd = { "/path/to/qi-lsp" },
  filetypes = { "qi" },
  root_dir = lspconfig.util.root_pattern(".git", "Cargo.toml"),
}
```

#### Emacs

```elisp
(use-package lsp-mode
  :config
  (lsp-register-client
    (make-lsp-client :new-connection (lsp-stdio-connection "/path/to/qi-lsp")
                    :major-modes '(qi-mode)
                    :server-id 'qi-lsp)))
```

### Configuration Options

The language server can be configured via environment variables:

- `QI_LSP_DEBUG=1`: Enable debug logging
- `QI_LSP_LOG=info`: Set log level (debug, info, warn, error)

## Usage

### Basic Workflow

1. **Open a Qi file** (`.qi` extension)
2. **Language features activate automatically**
3. **Write code with real-time feedback**
4. **Use keyboard shortcuts for navigation**:
   - `Ctrl+Click` (or `Cmd+Click`): Go to definition
   - `F12`: Go to definition
   - `Shift+F12`: Find all references
   - `Ctrl+Space`: Code completion
   - `Hover`: View documentation

### Supported File Types

- `.qi` - Qi source files
- Chinese and English file names
- UTF-8 encoded files

### Example Features

#### Code Completion
Type `函` and get suggestions like:
- `函数` - Function declaration
- `异步函数` - Async function declaration

#### Hover Information
Hover over `整数` to see:
```
**Type: i32**
32-bit signed integer

范围: -2,147,483,648 到 2,147,483,647
```

#### Diagnostics
```qi
变量 x: 整数 = "hello";
```
Shows error:
```
类型不匹配: 期望 整数, 实际 字符串
```

## Development

### Project Structure

```
qi-lsp/
├── src/
│   ├── main.rs              # Entry point
│   ├── lib.rs               # Core LSP implementation
│   ├── document/            # Document management
│   ├── diagnostics/         # Error reporting
│   ├── completion/          # Code completion
│   ├── hover/              # Hover information
│   ├── definition/         # Go to definition
│   ├── references/         # Find references
│   ├── formatting/         # Code formatting
│   └── server/             # Server utilities
├── Cargo.toml               # Dependencies
└── README.md               # This file
```

### Adding New Features

1. **Document handling**: Update `src/document/mod.rs`
2. **Language features**: Add to appropriate feature module
3. **Protocol support**: Update `src/lib.rs` for new LSP methods
4. **Testing**: Add tests in the relevant modules

### Debugging

Enable debug logging:

```bash
RUST_LOG=debug QI_LSP_DEBUG=1 /path/to/qi-lsp
```

Common debugging techniques:

- Check parser output in document AST
- Verify position calculations for UTF-8 text
- Monitor LSP message exchange
- Test with simple Qi files first

## Troubleshooting

### Common Issues

**Language server not starting**
- Verify binary path in editor configuration
- Check for missing dependencies
- Ensure file is executable

**No diagnostics showing**
- Confirm file has `.qi` extension
- Check if language server is running
- Verify Qi compiler integration

**Incorrect completion suggestions**
- Document may not be parsed correctly
- Check for syntax errors in source
- Verify UTF-8 encoding

**Position issues with Chinese text**
- Ensure UTF-8 encoding
- Check editor character encoding settings
- Verify LSP position calculations

### Getting Help

- Check logs for error messages
- Try with a simple test file
- Report issues on GitHub
- Join Qi community discussions

## Contributing

We welcome contributions! Please see:

1. **Development setup** - Build and test locally
2. **Code style** - Follow Rust conventions
3. **Testing** - Add tests for new features
4. **Documentation** - Update README and code comments

### Development Commands

```bash
# Build development version
cargo build

# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run

# Format code
cargo fmt

# Run linter
cargo clippy
```

## License

MIT License - see LICENSE file for details.

## Acknowledgments

- Built with [lsp-server](https://github.com/rust-lang/rust-analyzer/tree/master/lsp-server) and [lsp-types](https://github.com/rust-lang/rust-analyzer/tree/master/lsp-types)
- Integrates with the [Qi Compiler](https://github.com/qi-lang/qi-compiler)
- Inspired by existing language servers like rust-analyzer

---

For more information about the Qi programming language, visit [qi-lang.org](https://qi-lang.org).