# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Qi Language Server is a Language Server Protocol (LSP) implementation for the Qi programming language. It provides intelligent language features like code completion, diagnostics, go-to-definition, and formatting for Qi source code in editors and IDEs.

## Common Development Commands

### Building and Testing
```bash
# Build the language server (development build)
cargo build

# Build with optimizations
cargo build --release

# Run tests
cargo test

# Run tests with verbose output
cargo test -- --nocapture

# Format source code
cargo fmt

# Run linter
cargo clippy

# Run all checks (format, lint, test)
make check
```

### Development and Debugging
```bash
# Run development server with debug logging
make dev
# or
RUST_LOG=debug QI_LSP_DEBUG=1 cargo run

# Test with Qi examples
make example-test

# Run benchmarks
cargo bench

# Generate documentation
cargo doc --open
```

### Language Server Usage
```bash
# Start the language server
./target/release/qi-lsp

# Start with custom log level
RUST_LOG=info ./target/release/qi-lsp

# Show help
./target/release/qi-lsp --help
```

## Architecture Overview

### Core Components

#### Main Server (`src/lib.rs`)
- **QiLanguageServer**: Main LSP server implementation
- Handles LSP protocol initialization and message loop
- Dispatches requests to appropriate feature modules
- Manages server lifecycle and configuration

#### Document Management (`src/document/mod.rs`)
- **DocumentManager**: Manages all open documents
- **Document**: Represents a single editor document
- Integrates with Qi compiler parser for AST generation
- Provides position/offset conversion utilities
- Efficient text handling with Rope data structure

#### Feature Modules
- **Diagnostics** (`src/diagnostics/mod.rs`): Error reporting and syntax checking
- **Completion** (`src/completion/mod.rs`): Code completion for keywords, types, and symbols
- **Hover** (`src/hover/mod.rs`): Hover information and documentation
- **Definition** (`src/definition/mod.rs`): Go-to-definition functionality
- **References** (`src/references/mod.rs`): Find all references
- **Formatting** (`src/formatting/mod.rs`): Code formatting

### LSP Integration

#### Protocol Handling
- Full LSP 3.x support
- Text document synchronization (full sync)
- Multi-file workspace support
- Proper error handling and response management

#### Qi Compiler Integration
- Direct dependency on `qi-compiler` crate
- Shared parser and AST structures
- Real-time syntax analysis and error reporting
- Chinese keyword and identifier support

## Important Implementation Details

### Chinese Language Support
- UTF-8 text handling throughout
- Chinese keyword completion (函数, 变量, 如果, etc.)
- Proper position calculation for multi-byte characters
- Chinese type system integration (整数, 浮点数, 字符串, etc.)

### Document Management
- Rope-based text storage for efficient editing
- Incremental parsing with change detection
- AST caching for performance
- Document lifecycle management (open, change, close)

### Error Handling
- Comprehensive error types and propagation
- Graceful degradation for malformed input
- Debug logging throughout the codebase
- User-friendly error messages

### Performance Considerations
- Async/await for concurrent request handling
- Efficient text operations with Rope
- AST caching to avoid re-parsing
- Minimal allocation hot paths

## Development Notes

### Adding New Features

1. **Create module**: Add new feature module in `src/`
2. **Update lib.rs**: Add handler in main server loop
3. **Add tests**: Create unit and integration tests
4. **Update docs**: Document the new functionality

### Testing Strategy

```bash
# Unit tests
cargo test

# Integration tests with Qi files
make example-test

# LSP conformance tests
cargo test --test lsp_conformance

# Performance benchmarks
cargo bench
```

### Debugging Techniques

- Enable debug logging: `RUST_LOG=debug QI_LSP_DEBUG=1`
- Use VS Code LSP inspector
- Test with simple Qi files first
- Check parser output separately
- Verify position calculations

### Common Patterns

#### LSP Request Handling
```rust
pub async fn handle_feature_request(
    connection: &Connection,
    request: Request,
    document_manager: &DocumentManager,
) -> Result<()> {
    let params: FeatureParams = serde_json::from_value(request.params)?;
    let uri = params.text_document.uri.to_string();

    // Process request...

    let response = Response {
        id: request.id,
        result: Some(serde_json::to_value(result)?),
        error: None,
    };

    connection.sender.send(Message::Response(response))?;
    Ok(())
}
```

#### Document Operations
```rust
// Get document content
let content = document_manager.get_document_content(&uri)?;

// Get AST
let ast = document_manager.get_document_ast(&uri)?;

// Position conversion
let offset = document_manager.position_to_offset(&uri, position)?;
let position = document_manager.offset_to_position(&uri, offset)?;
```

### File Structure Notes

#### Critical Files
- `src/lib.rs`: Main LSP server implementation
- `src/document/mod.rs`: Document management and text handling
- `src/main.rs`: Entry point and configuration
- `Cargo.toml`: Dependencies and crate configuration
- `Makefile`: Development automation

#### Configuration Files
- `CLAUDE.md`: This file - Claude development guidance
- `README.md`: User documentation and setup instructions
- `.github/workflows/ci.yml`: CI/CD pipeline

## Known Issues and Workarounds

### Position Calculations
- Chinese characters require careful UTF-8 offset handling
- LSP uses UTF-16 positions internally
- Test with multi-byte characters extensively

### Parser Integration
- Parse errors may not have detailed span information
- Error recovery for incomplete syntax
- Async parsing for large files

### Memory Management
- Large files may cause memory pressure
- Document cleanup on close
- AST caching limits

## Development Guidelines

### Code Style
- Follow Rust conventions and `rustfmt` output
- Use `clippy` recommendations
- Document all public APIs
- Write comprehensive tests

### Performance Guidelines
- Use `Rope` for text operations
- Cache expensive computations
- Avoid blocking operations in request handlers
- Profile with `cargo bench`

### Testing Guidelines
- Test with real Qi source files
- Include Chinese text in test cases
- Mock LSP client for integration tests
- Test error conditions thoroughly

### Release Process
1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Run full test suite
4. Build release binary
5. Test with multiple editors
6. Create git tag and release

## Editor Integration Notes

### VS Code
- Configuration via `settings.json`
- Extension development with TypeScript
- Debug adapter protocol support

### Neovim
- Built-in LSP client
- Lua configuration
- Custom keybindings

### Emacs
- `lsp-mode` integration
- Eglot alternative
- Custom configuration

## Community and Support

### Reporting Issues
- Include minimal reproduction case
- Provide Qi source code examples
- Include editor and OS information
- Add debug logs if possible

### Contributing
- Fork the repository
- Create feature branch
- Add tests for new features
- Follow contribution guidelines
- Submit pull request with description

### Documentation
- Update README.md for user-facing changes
- Update CLAUDE.md for development changes
- Add inline code documentation
- Create examples and tutorials