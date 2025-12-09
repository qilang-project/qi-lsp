//! Hover functionality for the Qi Language Server
//!
//! This module provides hover information for Qi source code,
//! showing type information, documentation, and other useful details.

#![allow(dead_code)]

use anyhow::Result;
use log::debug;
use lsp_server::{Connection, Message, Request, Response};
use lsp_types::{
    Hover, HoverContents, HoverParams, MarkedString, Position,
};

use crate::document::DocumentManager;

/// Handle hover requests
pub async fn handle_hover(
    connection: &Connection,
    request: Request,
    document_manager: &DocumentManager,
) -> Result<()> {
    debug!("Handling hover request");

    let params: HoverParams = serde_json::from_value(request.params)?;
    let uri = params.text_document_position_params.text_document.uri.to_string();
    let position = params.text_document_position_params.position;

    // Try to get hover information
    let hover_info = get_hover_info(&uri, position, document_manager);

    let response = Response {
        id: request.id,
        result: Some(serde_json::to_value(hover_info)?),
        error: None,
    };

    connection.sender.send(Message::Response(response))?;
    debug!("Sent hover response");

    Ok(())
}

/// Get hover information for a position in a document
fn get_hover_info(
    uri: &str,
    position: Position,
    document_manager: &DocumentManager,
) -> Option<Hover> {
    // Get the current word at the cursor position
    let word = get_word_at_position(uri, position, document_manager)?;

    // Check if it's a keyword
    if let Some(keyword_info) = get_keyword_hover_info(&word) {
        return Some(keyword_info);
    }

    // Check if it's a type
    if let Some(type_info) = get_type_hover_info(&word) {
        return Some(type_info);
    }

    // Check if it's in the AST (function, variable, etc.)
    if let Some(ast) = document_manager.get_document_ast(uri) {
        if let Some(symbol_info) = get_symbol_hover_info(&word, &ast, position) {
            return Some(symbol_info);
        }
    }

    None
}

/// Get the word at the current cursor position
fn get_word_at_position(
    uri: &str,
    position: Position,
    document_manager: &DocumentManager,
) -> Option<String> {
    let line_content = document_manager.get_line_content(uri, position.line as usize)?;
    let char_pos = position.character as usize;

    if char_pos >= line_content.len() {
        return None;
    }

    // Find the start and end of the word
    let mut start = char_pos;
    let mut end = char_pos;

    // Find word start (move left until non-word character)
    while start > 0 {
        let ch = line_content.chars().nth(start - 1)?;
        if !is_word_char(ch) {
            break;
        }
        start -= 1;
    }

    // Find word end (move right until non-word character)
    while end < line_content.len() {
        let ch = line_content.chars().nth(end)?;
        if !is_word_char(ch) {
            break;
        }
        end += 1;
    }

    if start < end {
        Some(line_content[start..end].to_string())
    } else {
        None
    }
}

/// Check if a character is part of a word (identifier or Chinese character)
fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch as u32 >= 0x4E00 && ch as u32 <= 0x9FFF
}

/// Get hover information for keywords
fn get_keyword_hover_info(keyword: &str) -> Option<Hover> {
    let keyword_docs = std::collections::HashMap::from([
        ("包", ("package declaration", "包 主程序;\n\ndeclares the package for the current file")),
        ("函数", ("function declaration", "函数 name() {\n    // function body\n}\n\ndeclares a new function")),
        ("异步函数", ("async function declaration", "异步函数 name() {\n    // async function body\n}\n\ndeclares an async function")),
        ("变量", ("variable declaration", "变量 name: Type = value;\n\ndeclares a new variable")),
        ("如果", ("conditional statement", "如果 condition {\n    // then branch\n} 否则 {\n    // else branch\n}\n\nif-else conditional")),
        ("当", ("while loop", "当 condition {\n    // loop body\n}\n\nwhile loop that runs while condition is true")),
        ("对于", ("for loop", "对于 variable 在 range {\n    // loop body\n}\n\nfor loop over a range")),
        ("循环", ("infinite loop", "循环 {\n    // loop body\n}\n\ninfinite loop")),
        ("返回", ("return statement", "返回 value;\n\nreturns a value from a function")),
        ("结构体", ("struct declaration", "结构体 Name {\n    field: Type,\n}\n\ndeclares a struct type")),
        ("枚举", ("enum declaration", "枚举 Name {\n    Variant1,\n    Variant2,\n}\n\ndeclares an enum type")),
        ("打印", ("print function", "打印(\"message\");\n\nprints a message to stdout")),
        ("等待", ("await expression", "等待 async_call();\n\nawaits an async function call")),
        ("整数", ("integer type", "整数\n\n32-bit signed integer type")),
        ("浮点数", ("float type", "浮点数\n\n64-bit floating point type")),
        ("字符串", ("string type", "字符串\n\nUTF-8 string type")),
        ("布尔", ("boolean type", "布尔\n\nboolean type (真/假)")),
    ]);

    if let Some((kind, doc)) = keyword_docs.get(keyword) {
        let contents = HoverContents::Array(vec![
            MarkedString::String(format!("**{}**", kind)),
            MarkedString::String(format!("```qi\n{}\n```", doc)),
        ]);

        Some(Hover {
            contents,
            range: None,
        })
    } else {
        None
    }
}

/// Get hover information for types
fn get_type_hover_info(type_name: &str) -> Option<Hover> {
    let type_docs = std::collections::HashMap::from([
        ("整数", ("i32", "32-bit signed integer\n\n范围: -2,147,483,648 到 2,147,483,647")),
        ("长整数", ("i64", "64-bit signed integer\n\n范围: -9,223,372,036,854,775,808 到 9,223,372,036,854,775,807")),
        ("短整数", ("i16", "16-bit signed integer\n\n范围: -32,768 到 32,767")),
        ("字节", ("u8", "8-bit unsigned integer\n\n范围: 0 到 255")),
        ("浮点数", ("f64", "64-bit double-precision floating point number")),
        ("布尔", ("bool", "Boolean value\n\n值: 真 or 假")),
        ("字符", ("char", "Unicode character\n\n单个 Unicode 字符")),
        ("字符串", ("String", "UTF-8 string\n\n可变长度的 UTF-8 字符串")),
        ("空", ("()", "Unit type\n\n表示无返回值或空值")),
    ]);

    if let Some((kind, doc)) = type_docs.get(type_name) {
        let contents = HoverContents::Array(vec![
            MarkedString::String(format!("**Type: {}**", kind)),
            MarkedString::String(doc.to_string()),
        ]);

        Some(Hover {
            contents,
            range: None,
        })
    } else {
        None
    }
}

/// Get hover information for symbols from the AST
fn get_symbol_hover_info(
    symbol: &str,
    ast: &qi_compiler::parser::Program,
    position: Position,
) -> Option<Hover> {
    // Search for symbol definition in the AST
    if let Some(symbol_info) = resolve_symbol(symbol, ast, position) {
        return Some(create_symbol_hover(symbol, symbol_info));
    }

    // If symbol not found, provide helpful information
    let contents = HoverContents::Scalar(MarkedString::String(format!(
        "**未找到符号**: `{}`\n\n此符号在当前作用域中未定义。请检查:\n- 拼写是否正确\n- 符号是否已声明\n- 是否在正确的作用域内",
        symbol
    )));

    Some(Hover {
        contents,
        range: None,
    })
}

/// Symbol information extracted from AST
#[derive(Debug, Clone)]
struct SymbolInfo {
    name: String,
    symbol_type: SymbolType,
    description: String,
    documentation: Option<String>,
    span: qi_compiler::lexer::tokens::Span,
}

/// Types of symbols for hover
#[derive(Debug, Clone)]
enum SymbolType {
    Variable { is_mutable: bool, var_type: Option<String> },
    Function { parameters: Vec<String>, return_type: Option<String> },
    Struct { fields: Vec<String> },
    Enum { variants: Vec<String> },
    Method { parameters: Vec<String>, return_type: Option<String> },
    Parameter,
    Type,
}

/// Resolve a symbol in the AST
fn resolve_symbol(
    symbol_name: &str,
    ast: &qi_compiler::parser::Program,
    _position: Position,
) -> Option<SymbolInfo> {
    // Search through top-level statements
    for statement in &ast.statements {
        if let Some(info) = resolve_symbol_in_statement(symbol_name, statement) {
            return Some(info);
        }
    }

    None
}

/// Resolve symbol in a statement
fn resolve_symbol_in_statement(
    symbol_name: &str,
    statement: &qi_compiler::parser::AstNode,
) -> Option<SymbolInfo> {
    use qi_compiler::parser::AstNode;

    match statement {
        AstNode::变量声明(var_decl) => {
            if var_decl.name == symbol_name {
                let var_type = var_decl.type_annotation.as_ref()
                    .map(|t| format_type_annotation(t));
                let description = if var_decl.is_mutable {
                    format!("可变变量: {}", var_decl.name)
                } else {
                    format!("不可变变量: {}", var_decl.name)
                };

                return Some(SymbolInfo {
                    name: var_decl.name.clone(),
                    symbol_type: SymbolType::Variable {
                        is_mutable: var_decl.is_mutable,
                        var_type,
                    },
                    description,
                    documentation: Some("变量声明".to_string()),
                    span: var_decl.span,
                });
            }
        }
        AstNode::函数声明(func_decl) => {
            if func_decl.name == symbol_name {
                let parameters = func_decl.parameters
                    .iter()
                    .map(|p| format!("{}: {}", p.name,
                        p.type_annotation.as_ref()
                            .map(|t| format_type_annotation(t))
                            .unwrap_or_else(|| "_".to_string())))
                    .collect();
                let return_type = func_decl.return_type.as_ref()
                    .map(|t| format_type_annotation(t));

                return Some(SymbolInfo {
                    name: func_decl.name.clone(),
                    symbol_type: SymbolType::Function { parameters, return_type },
                    description: format!("函数: {}", func_decl.name),
                    documentation: Some("函数声明".to_string()),
                    span: func_decl.span,
                });
            }
        }
        // Async functions are now handled by 函数声明 with is_async flag
        AstNode::结构体声明(struct_decl) => {
            if struct_decl.name == symbol_name {
                let fields = struct_decl.fields
                    .iter()
                    .map(|f| format!("{}: {}", f.name, format_type_annotation(&f.type_annotation)))
                    .collect();

                return Some(SymbolInfo {
                    name: struct_decl.name.clone(),
                    symbol_type: SymbolType::Struct { fields },
                    description: format!("结构体: {}", struct_decl.name),
                    documentation: Some("结构体声明".to_string()),
                    span: struct_decl.span,
                });
            }
        }
        AstNode::枚举声明(enum_decl) => {
            if enum_decl.name == symbol_name {
                let variants = enum_decl.variants
                    .iter()
                    .map(|v| v.name.clone())
                    .collect();

                return Some(SymbolInfo {
                    name: enum_decl.name.clone(),
                    symbol_type: SymbolType::Enum { variants },
                    description: format!("枚举: {}", enum_decl.name),
                    documentation: Some("枚举声明".to_string()),
                    span: enum_decl.span,
                });
            }
        }
        AstNode::方法声明(method_decl) => {
            if method_decl.method_name == symbol_name {
                let parameters = method_decl.parameters
                    .iter()
                    .map(|p| format!("{}: {}", p.name,
                        p.type_annotation.as_ref()
                            .map(|t| format_type_annotation(t))
                            .unwrap_or_else(|| "_".to_string())))
                    .collect();
                let return_type = method_decl.return_type.as_ref()
                    .map(|t| format_type_annotation(t));

                return Some(SymbolInfo {
                    name: method_decl.method_name.clone(),
                    symbol_type: SymbolType::Method { parameters, return_type },
                    description: format!("方法: {}.{}", method_decl.receiver_type, method_decl.method_name),
                    documentation: Some("方法声明".to_string()),
                    span: method_decl.span,
                });
            }
        }
        AstNode::块语句(block_stmt) => {
            // Search recursively in block statements
            for stmt in &block_stmt.statements {
                if let Some(info) = resolve_symbol_in_statement(symbol_name, stmt) {
                    return Some(info);
                }
            }
        }
        AstNode::如果语句(if_stmt) => {
            // Search in then branch
            for stmt in &if_stmt.then_branch {
                if let Some(info) = resolve_symbol_in_statement(symbol_name, stmt) {
                    return Some(info);
                }
            }
            // Search in else branch
            if let Some(else_branch) = &if_stmt.else_branch {
                if let Some(info) = resolve_symbol_in_statement(symbol_name, else_branch) {
                    return Some(info);
                }
            }
        }
        AstNode::当语句(while_stmt) => {
            for stmt in &while_stmt.body {
                if let Some(info) = resolve_symbol_in_statement(symbol_name, stmt) {
                    return Some(info);
                }
            }
        }
        AstNode::对于语句(for_stmt) => {
            if for_stmt.variable == symbol_name {
                return Some(SymbolInfo {
                    name: for_stmt.variable.clone(),
                    symbol_type: SymbolType::Variable {
                        is_mutable: false,
                        var_type: None,
                    },
                    description: format!("循环变量: {}", for_stmt.variable),
                    documentation: Some("对于循环变量".to_string()),
                    span: for_stmt.span,
                });
            }

            for stmt in &for_stmt.body {
                if let Some(info) = resolve_symbol_in_statement(symbol_name, stmt) {
                    return Some(info);
                }
            }
        }
        AstNode::循环语句(loop_stmt) => {
            for stmt in &loop_stmt.body {
                if let Some(info) = resolve_symbol_in_statement(symbol_name, stmt) {
                    return Some(info);
                }
            }
        }
        _ => {}
    }

    None
}

/// Create hover information for a symbol
fn create_symbol_hover(symbol_name: &str, symbol_info: SymbolInfo) -> Hover {
    let mut content = Vec::new();

    // Add symbol type and description
    content.push(MarkedString::String(format!("**{}**", symbol_info.description)));

    // Add detailed information based on symbol type
    match symbol_info.symbol_type {
        SymbolType::Variable { is_mutable, var_type } => {
            let mutability = if is_mutable { "可变" } else { "不可变" };
            let type_info = var_type.unwrap_or_else(|| "自动推断".to_string());
            content.push(MarkedString::String(format!("类型: {}", type_info)));
            content.push(MarkedString::String(format!("可变性: {}", mutability)));
        }
        SymbolType::Function { parameters, return_type } => {
            let params_str = if parameters.is_empty() {
                String::new()
            } else {
                format!("({})", parameters.join(", "))
            };
            let return_str = return_type
                .map(|t| format!(" -> {}", t))
                .unwrap_or_default();

            content.push(MarkedString::String(format!("```qi\n{}{}{}\n```", symbol_name, params_str, return_str)));
        }
        SymbolType::Method { parameters, return_type } => {
            let params_str = if parameters.is_empty() {
                String::new()
            } else {
                format!("({})", parameters.join(", "))
            };
            let return_str = return_type
                .map(|t| format!(" -> {}", t))
                .unwrap_or_default();

            content.push(MarkedString::String(format!("```qi\n{}{}{}\n```", symbol_name, params_str, return_str)));
        }
        SymbolType::Struct { fields } => {
            if !fields.is_empty() {
                let fields_str = fields.join("\n  ");
                content.push(MarkedString::String(format!("字段:\n  {}", fields_str)));
            } else {
                content.push(MarkedString::String("空结构体".to_string()));
            }
        }
        SymbolType::Enum { variants } => {
            if !variants.is_empty() {
                let variants_str = variants.join(", ");
                content.push(MarkedString::String(format!("变体: {}", variants_str)));
            } else {
                content.push(MarkedString::String("空枚举".to_string()));
            }
        }
        SymbolType::Parameter => {
            content.push(MarkedString::String("函数参数".to_string()));
        }
        SymbolType::Type => {
            content.push(MarkedString::String("类型定义".to_string()));
        }
    }

    // Add documentation if available
    if let Some(doc) = symbol_info.documentation {
        content.push(MarkedString::String(format!("\n{}", doc)));
    }

    let contents = HoverContents::Array(content);

    Hover {
        contents,
        range: None,
    }
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

/// Create hover information for a function
fn create_function_hover(name: &str, params: &[String], return_type: Option<&str>) -> Hover {
    let signature = if params.is_empty() {
        format!("函数 {}()", name)
    } else {
        format!("函数 {}({})", name, params.join(", "))
    };

    let mut doc = signature;
    if let Some(ret_type) = return_type {
        doc.push_str(&format!(" -> {}", ret_type));
    }
    doc.push_str("\n\n函数定义");

    let contents = HoverContents::Scalar(MarkedString::String(doc));

    Hover {
        contents,
        range: None,
    }
}

/// Create hover information for a variable
fn create_variable_hover(name: &str, var_type: &str, is_mutable: bool) -> Hover {
    let mutability = if is_mutable { "可变" } else { "不可变" };
    let doc = format!("**{}** `{}`: {}", mutability, name, var_type);

    let contents = HoverContents::Scalar(MarkedString::String(doc));

    Hover {
        contents,
        range: None,
    }
}