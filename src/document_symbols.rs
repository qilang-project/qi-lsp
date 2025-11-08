//! Document symbols functionality for the Qi Language Server
//!
//! This module provides document symbol information, allowing
//! users to navigate to symbols within the current document.

use anyhow::Result;
use log::debug;
use lsp_server::{Connection, Message, Request, Response};
use lsp_types::{
    DocumentSymbol, DocumentSymbolParams, SymbolKind, Range, Position,
};

use crate::document::DocumentManager;

/// Handle document symbol requests
pub async fn handle_document_symbols(
    connection: &Connection,
    request: Request,
    document_manager: &DocumentManager,
) -> Result<()> {
    debug!("Handling document symbols request");

    let params: DocumentSymbolParams = serde_json::from_value(request.params)?;
    let uri = params.text_document.uri.to_string();

    // Try to find all document symbols
    let symbols = find_document_symbols(&uri, document_manager);

    let response = Response {
        id: request.id,
        result: Some(serde_json::to_value(&symbols)?),
        error: None,
    };

    connection.sender.send(Message::Response(response))?;
    debug!("Sent document symbols response with {} symbols", symbols.len());

    Ok(())
}

/// Find all symbols in a document
fn find_document_symbols(
    uri: &str,
    document_manager: &DocumentManager,
) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    if let Some(ast) = document_manager.get_document_ast(uri) {
        // Search through the AST for all symbols
        find_symbols_in_ast(&qi_compiler::parser::AstNode::程序((*ast).clone()), uri, &mut symbols, document_manager);
    }

    symbols
}

/// Recursively search the AST for symbols
fn find_symbols_in_ast(
    node: &qi_compiler::parser::AstNode,
    uri: &str,
    symbols: &mut Vec<DocumentSymbol>,
    document_manager: &DocumentManager,
) {
    use qi_compiler::parser::AstNode;

    match node {
        AstNode::函数声明(func_decl) => {
            let symbol = DocumentSymbol {
                name: func_decl.name.clone(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                detail: Some(format_function_signature(func_decl)),
                selection_range: span_to_range(&func_decl.span, uri, document_manager),
                range: span_to_range(&func_decl.span, uri, document_manager),
                children: Some(find_function_symbols(func_decl, uri, document_manager)),
                deprecated: None,
            };
            symbols.push(symbol);
        }
        // Async functions are now handled by 函数声明 with is_async flag
        AstNode::方法声明(method_decl) => {
            let symbol = DocumentSymbol {
                name: method_decl.method_name.clone(),
                kind: SymbolKind::METHOD,
                tags: None,
                detail: Some(format_method_signature(method_decl)),
                selection_range: span_to_range(&method_decl.span, uri, document_manager),
                range: span_to_range(&method_decl.span, uri, document_manager),
                children: Some(find_method_symbols(method_decl, uri, document_manager)),
                deprecated: None,
            };
            symbols.push(symbol);
        }
        AstNode::结构体声明(struct_decl) => {
            let symbol = DocumentSymbol {
                name: struct_decl.name.clone(),
                kind: SymbolKind::STRUCT,
                tags: None,
                detail: Some(format_struct_signature(struct_decl)),
                selection_range: span_to_range(&struct_decl.span, uri, document_manager),
                range: span_to_range(&struct_decl.span, uri, document_manager),
                children: Some(find_struct_symbols(struct_decl, uri, document_manager)),
                deprecated: None,
            };
            symbols.push(symbol);
        }
        AstNode::枚举声明(enum_decl) => {
            let symbol = DocumentSymbol {
                name: enum_decl.name.clone(),
                kind: SymbolKind::ENUM,
                tags: None,
                detail: Some(format_enum_signature(enum_decl)),
                selection_range: span_to_range(&enum_decl.span, uri, document_manager),
                range: span_to_range(&enum_decl.span, uri, document_manager),
                children: Some(find_enum_symbols(enum_decl, uri, document_manager)),
                deprecated: None,
            };
            symbols.push(symbol);
        }
        AstNode::变量声明(var_decl) => {
            let symbol = DocumentSymbol {
                name: var_decl.name.clone(),
                kind: if var_decl.is_mutable { SymbolKind::VARIABLE } else { SymbolKind::CONSTANT },
                tags: None,
                detail: var_decl.type_annotation.as_ref().map(|t| format!("{:?}", t)),
                selection_range: span_to_range(&var_decl.span, uri, document_manager),
                range: span_to_range(&var_decl.span, uri, document_manager),
                children: None,
                deprecated: None,
            };
            symbols.push(symbol);
        }
        AstNode::程序(program) => {
            // Search through statements for top-level declarations
            for stmt in &program.statements {
                find_symbols_in_ast(stmt, uri, symbols, document_manager);
            }
        }
        // Handle other node types that might contain nested symbols
        AstNode::块语句(block_stmt) => {
            // Search for local symbols in blocks
            symbols.extend(find_block_symbols(block_stmt, uri, document_manager));
        }
        _ => {}
    }
}

/// Find symbols in function body
fn find_function_symbols(
    func_decl: &qi_compiler::parser::ast::FunctionDeclaration,
    uri: &str,
    document_manager: &DocumentManager,
) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    // Add parameters as symbols
    for param in &func_decl.parameters {
        let symbol = DocumentSymbol {
            name: param.name.clone(),
            kind: SymbolKind::VARIABLE,
            tags: None,
            detail: Some(format!("{:?}", param.type_annotation)),
            selection_range: span_to_range(&param.span, uri, document_manager),
            range: span_to_range(&param.span, uri, document_manager),
            children: None,
            deprecated: None,
        };
        symbols.push(symbol);
    }

    // Find local symbols in function body
    for stmt in &func_decl.body {
        find_local_symbols_in_statement(stmt, uri, &mut symbols, document_manager);
    }

    symbols
}

// Async function symbols are now handled by find_function_symbols
// since AsyncFunctionDeclaration was merged into FunctionDeclaration with is_async flag

/// Find symbols in method body
fn find_method_symbols(
    method_decl: &qi_compiler::parser::ast::MethodDeclaration,
    uri: &str,
    document_manager: &DocumentManager,
) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    // Add parameters as symbols
    for param in &method_decl.parameters {
        let symbol = DocumentSymbol {
            name: param.name.clone(),
            kind: SymbolKind::VARIABLE,
            tags: None,
            detail: Some(format!("{:?}", param.type_annotation)),
            selection_range: span_to_range(&param.span, uri, document_manager),
            range: span_to_range(&param.span, uri, document_manager),
            children: None,
            deprecated: None,
        };
        symbols.push(symbol);
    }

    // Find local symbols in method body
    for stmt in &method_decl.body {
        find_local_symbols_in_statement(stmt, uri, &mut symbols, document_manager);
    }

    symbols
}

/// Find symbols in struct
fn find_struct_symbols(
    struct_decl: &qi_compiler::parser::ast::StructDeclaration,
    uri: &str,
    document_manager: &DocumentManager,
) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    // Add fields as symbols
    for field in &struct_decl.fields {
        let symbol = DocumentSymbol {
            name: field.name.clone(),
            kind: SymbolKind::FIELD,
            tags: None,
            detail: Some(format!("{:?}", field.type_annotation)),
            selection_range: span_to_range(&field.span, uri, document_manager),
            range: span_to_range(&field.span, uri, document_manager),
            children: None,
            deprecated: None,
        };
        symbols.push(symbol);
    }

    // Add methods as symbols
    for method in &struct_decl.methods {
        find_symbols_in_ast(&qi_compiler::parser::AstNode::方法声明(method.clone()), uri, &mut symbols, document_manager);
    }

    symbols
}

/// Find symbols in enum
fn find_enum_symbols(
    enum_decl: &qi_compiler::parser::ast::EnumDeclaration,
    uri: &str,
    document_manager: &DocumentManager,
) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    // Add variants as symbols
    for variant in &enum_decl.variants {
        let symbol = DocumentSymbol {
            name: variant.name.clone(),
            kind: SymbolKind::ENUM_MEMBER,
            tags: None,
            detail: None,
            selection_range: span_to_range(&variant.span, uri, document_manager),
            range: span_to_range(&variant.span, uri, document_manager),
            children: None,
            deprecated: None,
        };
        symbols.push(symbol);
    }

    symbols
}

/// Find symbols in block statement
fn find_block_symbols(
    block_stmt: &qi_compiler::parser::ast::BlockStatement,
    uri: &str,
    document_manager: &DocumentManager,
) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    for stmt in &block_stmt.statements {
        find_local_symbols_in_statement(stmt, uri, &mut symbols, document_manager);
    }

    symbols
}

/// Find local symbols in a statement
fn find_local_symbols_in_statement(
    stmt: &qi_compiler::parser::AstNode,
    uri: &str,
    symbols: &mut Vec<DocumentSymbol>,
    document_manager: &DocumentManager,
) {
    use qi_compiler::parser::AstNode;

    match stmt {
        AstNode::变量声明(var_decl) => {
            let symbol = DocumentSymbol {
                name: var_decl.name.clone(),
                kind: if var_decl.is_mutable { SymbolKind::VARIABLE } else { SymbolKind::CONSTANT },
                tags: None,
                detail: var_decl.type_annotation.as_ref().map(|t| format!("{:?}", t)),
                selection_range: span_to_range(&var_decl.span, uri, document_manager),
                range: span_to_range(&var_decl.span, uri, document_manager),
                children: None,
                deprecated: None,
            };
            symbols.push(symbol);
        }
        AstNode::块语句(block_stmt) => {
            // Recursively find symbols in nested blocks
            for stmt in &block_stmt.statements {
                find_local_symbols_in_statement(stmt, uri, symbols, document_manager);
            }
        }
        AstNode::如果语句(if_stmt) => {
            // Find symbols in if branches
            for stmt in &if_stmt.then_branch {
                find_local_symbols_in_statement(stmt, uri, symbols, document_manager);
            }
            if let Some(else_branch) = &if_stmt.else_branch {
                find_local_symbols_in_statement(else_branch, uri, symbols, document_manager);
            }
        }
        AstNode::当语句(while_stmt) => {
            // Find symbols in while body
            for stmt in &while_stmt.body {
                find_local_symbols_in_statement(stmt, uri, symbols, document_manager);
            }
        }
        AstNode::对于语句(for_stmt) => {
            // Add loop variable as symbol
            let symbol = DocumentSymbol {
                name: for_stmt.variable.clone(),
                kind: SymbolKind::VARIABLE,
                tags: None,
                detail: None,
                selection_range: span_to_range(&for_stmt.span, uri, document_manager),
                range: span_to_range(&for_stmt.span, uri, document_manager),
                children: None,
                deprecated: None,
            };
            symbols.push(symbol);

            // Find symbols in for body
            for stmt in &for_stmt.body {
                find_local_symbols_in_statement(stmt, uri, symbols, document_manager);
            }
        }
        AstNode::循环语句(loop_stmt) => {
            // Find symbols in loop body
            for stmt in &loop_stmt.body {
                find_local_symbols_in_statement(stmt, uri, symbols, document_manager);
            }
        }
        _ => {}
    }
}

/// Convert a span to LSP range
fn span_to_range(
    span: &qi_compiler::lexer::Span,
    uri: &str,
    document_manager: &DocumentManager,
) -> Range {
    match crate::definition::span_to_location(span, uri, document_manager) {
        Some(location) => location.range,
        None => Range {
            start: Position { line: 0, character: 0 },
            end: Position { line: 0, character: 0 },
        },
    }
}

/// Format function signature
fn format_function_signature(func_decl: &qi_compiler::parser::ast::FunctionDeclaration) -> String {
    let params: Vec<String> = func_decl.parameters
        .iter()
        .map(|p| format!("{}: {:?}", p.name, p.type_annotation))
        .collect();

    let return_type = func_decl.return_type.as_ref()
        .map(|t| format!(": {:?}", t))
        .unwrap_or_default();

    format!("函数 {}({}){}", func_decl.name, params.join(", "), return_type)
}

// Removed: format_async_function_signature - async functions now use format_function_signature
// with is_async flag in FunctionDeclaration

/// Format method signature
fn format_method_signature(method_decl: &qi_compiler::parser::ast::MethodDeclaration) -> String {
    let params: Vec<String> = method_decl.parameters
        .iter()
        .map(|p| format!("{}: {:?}", p.name, p.type_annotation))
        .collect();

    let return_type = method_decl.return_type.as_ref()
        .map(|t| format!(": {:?}", t))
        .unwrap_or_default();

    format!("方法 {}({}){}", method_decl.method_name, params.join(", "), return_type)
}

/// Format struct signature
fn format_struct_signature(struct_decl: &qi_compiler::parser::ast::StructDeclaration) -> String {
    let fields: Vec<String> = struct_decl.fields
        .iter()
        .map(|f| format!("{}: {:?}", f.name, f.type_annotation))
        .collect();

    format!("结构体 {} {{ {} }}", struct_decl.name, fields.join(", "))
}

/// Format enum signature
fn format_enum_signature(enum_decl: &qi_compiler::parser::ast::EnumDeclaration) -> String {
    let variants: Vec<String> = enum_decl.variants
        .iter()
        .map(|v| v.name.clone())
        .collect();

    format!("枚举 {} {{ {} }}", enum_decl.name, variants.join(" | "))
}