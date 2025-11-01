//! Workspace symbols functionality for the Qi Language Server
//!
//! This module provides workspace-wide symbol search capabilities,
//! allowing users to find functions, types, and other symbols across
//! all documents in the workspace.

use anyhow::Result;
use log::debug;
use lsp_server::{Connection, Message, Request, Response};
use lsp_types::{
    WorkspaceSymbol, WorkspaceSymbolParams, SymbolKind, Location, Range, Position,
};

use crate::document::DocumentManager;

/// Handle workspace symbol requests
pub async fn handle_workspace_symbols(
    connection: &Connection,
    request: Request,
    document_manager: &DocumentManager,
) -> Result<()> {
    debug!("Handling workspace symbol request");

    let params: WorkspaceSymbolParams = serde_json::from_value(request.params)?;
    let query = params.query.trim();

    // Search for symbols across all documents
    let symbols = find_workspace_symbols(query, document_manager);
    let symbols_count = symbols.len();

    let response = Response {
        id: request.id,
        result: Some(serde_json::to_value(&symbols)?),
        error: None,
    };

    connection.sender.send(Message::Response(response))?;
    debug!("Sent workspace symbol response with {} symbols", symbols_count);

    Ok(())
}

/// Find workspace symbols matching the query
fn find_workspace_symbols(query: &str, document_manager: &DocumentManager) -> Vec<WorkspaceSymbol> {
    let mut symbols = Vec::new();

    // If query is empty, return all symbols (with reasonable limits)
    let query = if query.is_empty() { "*" } else { query };

    // Search through all open documents
    for uri in document_manager.get_all_uris() {
        if let Some(ast) = document_manager.get_document_ast(&uri) {
            let doc_symbols = find_symbols_in_document(&uri, &ast, query, document_manager);
            symbols.extend(doc_symbols);
        }
    }

    // Sort by relevance (exact matches first, then prefix matches)
    symbols.sort_by(|a, b| {
        let a_name = a.name.to_lowercase();
        let b_name = b.name.to_lowercase();
        let query_lower = query.to_lowercase();

        let a_exact = a_name == query_lower;
        let b_exact = b_name == query_lower;

        match (a_exact, b_exact) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                let a_prefix = a_name.starts_with(&query_lower);
                let b_prefix = b_name.starts_with(&query_lower);
                match (a_prefix, b_prefix) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a_name.cmp(&b_name),
                }
            }
        }
    });

    // Limit results to prevent overwhelming the editor
    symbols.truncate(100);

    symbols
}

/// Find symbols in a specific document
fn find_symbols_in_document(
    uri: &str,
    ast: &qi_compiler::parser::Program,
    query: &str,
    document_manager: &DocumentManager,
) -> Vec<WorkspaceSymbol> {
    let mut symbols = Vec::new();

    // Search through top-level statements
    for statement in &ast.statements {
        extract_symbols_from_statement(uri, statement, query, &mut symbols, document_manager);
    }

    symbols
}

/// Extract symbols from an AST statement
fn extract_symbols_from_statement(
    uri: &str,
    statement: &qi_compiler::parser::AstNode,
    query: &str,
    symbols: &mut Vec<WorkspaceSymbol>,
    document_manager: &DocumentManager,
) {
    use qi_compiler::parser::AstNode;

    match statement {
        AstNode::函数声明(func_decl) => {
            if matches_query(&func_decl.name, query) {
                symbols.push(create_workspace_symbol(
                    &func_decl.name,
                    SymbolKind::FUNCTION,
                    uri,
                    &func_decl.span,
                    Some(format!("函数 {}({})", func_decl.name, format_parameters(&func_decl.parameters))),
                    document_manager,
                ));
            }
        }
        AstNode::异步函数声明(async_func_decl) => {
            if matches_query(&async_func_decl.name, query) {
                symbols.push(create_workspace_symbol(
                    &async_func_decl.name,
                    SymbolKind::FUNCTION,
                    uri,
                    &async_func_decl.span,
                    Some(format!("异步函数 {}({})", async_func_decl.name, format_parameters(&async_func_decl.parameters))),
                    document_manager,
                ));
            }
        }
        AstNode::变量声明(var_decl) => {
            if matches_query(&var_decl.name, query) {
                let kind = if var_decl.is_mutable { SymbolKind::VARIABLE } else { SymbolKind::CONSTANT };
                let type_info = var_decl.type_annotation.as_ref()
                    .map(|t| format_type_annotation(t))
                    .unwrap_or_else(|| "自动推断".to_string());

                symbols.push(create_workspace_symbol(
                    &var_decl.name,
                    kind,
                    uri,
                    &var_decl.span,
                    Some(format!("变量: {} ({})", var_decl.name, type_info)),
                    document_manager,
                ));
            }
        }
        AstNode::结构体声明(struct_decl) => {
            if matches_query(&struct_decl.name, query) {
                let fields_info = if struct_decl.fields.is_empty() {
                    "空结构体".to_string()
                } else {
                    format!("结构体 ({} 个字段)", struct_decl.fields.len())
                };

                symbols.push(create_workspace_symbol(
                    &struct_decl.name,
                    SymbolKind::STRUCT,
                    uri,
                    &struct_decl.span,
                    Some(fields_info),
                    document_manager,
                ));

                // Also add struct fields as symbols
                for field in &struct_decl.fields {
                    if matches_query(&field.name, query) {
                        symbols.push(create_workspace_symbol(
                            &format!("{}.{}", struct_decl.name, field.name),
                            SymbolKind::FIELD,
                            uri,
                            &field.span,
                            Some(format!("字段: {} ({})", field.name, format_type_annotation(&field.type_annotation))),
                            document_manager,
                        ));
                    }
                }

                // Add struct methods
                for method in &struct_decl.methods {
                    if matches_query(&method.method_name, query) {
                        symbols.push(create_workspace_symbol(
                            &format!("{}.{}", struct_decl.name, method.method_name),
                            SymbolKind::METHOD,
                            uri,
                            &method.span,
                            Some(format!("方法: {}({})", method.method_name, format_parameters(&method.parameters))),
                            document_manager,
                        ));
                    }
                }
            }
        }
        AstNode::枚举声明(enum_decl) => {
            if matches_query(&enum_decl.name, query) {
                let variants_info = if enum_decl.variants.is_empty() {
                    "空枚举".to_string()
                } else {
                    format!("枚举 ({} 个变体)", enum_decl.variants.len())
                };

                symbols.push(create_workspace_symbol(
                    &enum_decl.name,
                    SymbolKind::ENUM,
                    uri,
                    &enum_decl.span,
                    Some(variants_info),
                    document_manager,
                ));

                // Also add enum variants as symbols
                for variant in &enum_decl.variants {
                    if matches_query(&variant.name, query) {
                        symbols.push(create_workspace_symbol(
                            &format!("{}.{}", enum_decl.name, variant.name),
                            SymbolKind::ENUM_MEMBER,
                            uri,
                            &variant.span,
                            Some(format!("枚举变体: {}", variant.name)),
                            document_manager,
                        ));
                    }
                }
            }
        }
        AstNode::方法声明(method_decl) => {
            if matches_query(&method_decl.method_name, query) {
                symbols.push(create_workspace_symbol(
                    &format!("{}.{}", method_decl.receiver_type, method_decl.method_name),
                    SymbolKind::METHOD,
                    uri,
                    &method_decl.span,
                    Some(format!("方法: {}({})", method_decl.method_name, format_parameters(&method_decl.parameters))),
                    document_manager,
                ));
            }
        }
        AstNode::块语句(block_stmt) => {
            // Recursively search in block statements for nested symbols
            for stmt in &block_stmt.statements {
                extract_symbols_from_statement(uri, stmt, query, symbols, document_manager);
            }
        }
        // Add other statement types as needed
        _ => {}
    }
}

/// Check if a symbol name matches the query
fn matches_query(symbol_name: &str, query: &str) -> bool {
    if query == "*" {
        return true;
    }

    let symbol_lower = symbol_name.to_lowercase();
    let query_lower = query.to_lowercase();

    // Exact match
    if symbol_lower == query_lower {
        return true;
    }

    // Prefix match
    if symbol_lower.starts_with(&query_lower) {
        return true;
    }

    // Contains match (less prioritized)
    symbol_lower.contains(&query_lower)
}

/// Create a workspace symbol
fn create_workspace_symbol(
    name: &str,
    kind: SymbolKind,
    uri: &str,
    span: &qi_compiler::lexer::tokens::Span,
    detail: Option<String>,
    document_manager: &DocumentManager,
) -> WorkspaceSymbol {
    let location = Location {
        uri: uri.parse::<lsp_types::Uri>().unwrap_or_else(|_| "file://unknown".parse::<lsp_types::Uri>().unwrap()),
        range: span_to_range(span, document_manager).unwrap_or_else(|| Range {
            start: Position { line: 0, character: 0 },
            end: Position { line: 0, character: 1 },
        }),
    };

    WorkspaceSymbol {
        name: name.to_string(),
        kind,
        tags: None,
        location: lsp_types::OneOf::Left(location),
        container_name: detail,
        data: None,
    }
}

/// Format parameters for display
fn format_parameters(parameters: &[qi_compiler::parser::Parameter]) -> String {
    if parameters.is_empty() {
        return String::new();
    }

    parameters
        .iter()
        .map(|param| {
            let type_info = param.type_annotation.as_ref()
                .map(|t| format_type_annotation(t))
                .unwrap_or_else(|| "_".to_string());
            format!("{}: {}", param.name, type_info)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Format type annotation for display
fn format_type_annotation(type_annotation: &qi_compiler::parser::TypeNode) -> String {
    use qi_compiler::parser::TypeNode;

    match type_annotation {
        TypeNode::基础类型(basic_type) => {
            match basic_type {
                qi_compiler::parser::BasicType::整数 => "整数".to_string(),
                qi_compiler::parser::BasicType::长整数 => "长整数".to_string(),
                qi_compiler::parser::BasicType::短整数 => "短整数".to_string(),
                qi_compiler::parser::BasicType::字节 => "字节".to_string(),
                qi_compiler::parser::BasicType::浮点数 => "浮点数".to_string(),
                qi_compiler::parser::BasicType::布尔 => "布尔".to_string(),
                qi_compiler::parser::BasicType::字符 => "字符".to_string(),
                qi_compiler::parser::BasicType::字符串 => "字符串".to_string(),
                qi_compiler::parser::BasicType::空 => "空".to_string(),
                qi_compiler::parser::BasicType::数组 => "数组".to_string(),
                qi_compiler::parser::BasicType::字典 => "字典".to_string(),
                qi_compiler::parser::BasicType::列表 => "列表".to_string(),
                qi_compiler::parser::BasicType::集合 => "集合".to_string(),
                qi_compiler::parser::BasicType::指针 => "指针".to_string(),
                qi_compiler::parser::BasicType::引用 => "引用".to_string(),
                qi_compiler::parser::BasicType::可变引用 => "可变引用".to_string(),
            }
        }
        TypeNode::自定义类型(name) => name.clone(),
        TypeNode::数组类型(array_type) => {
            format!("[{}]", format_type_annotation(&array_type.element_type))
        }
        TypeNode::函数类型(func_type) => {
            let params = func_type.parameters
                .iter()
                .map(|t| format_type_annotation(t))
                .collect::<Vec<_>>()
                .join(", ");
            let ret = format_type_annotation(&func_type.return_type);
            format!("({}) -> {}", params, ret)
        }
        _ => format!("{:?}", type_annotation), // Fallback
    }
}

/// Convert span to range
fn span_to_range(
    span: &qi_compiler::lexer::tokens::Span,
    document_manager: &DocumentManager,
) -> Option<Range> {
    // This is a simplified implementation
    // In a real implementation, you'd need to properly convert byte offsets to line/column
    Some(Range {
        start: Position {
            line: 0,
            character: span.start as u32,
        },
        end: Position {
            line: 0,
            character: span.end as u32,
        },
    })
}