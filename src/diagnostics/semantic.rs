//! Semantic analysis for Qi Language Server diagnostics
//!
//! This module provides semantic analysis capabilities including:
//! - Type checking
//! - Unused variable detection
//! - Function call validation
//! - Import analysis
//! - Scope analysis

use crate::document::DocumentManager;
use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range, NumberOrString};
use qi_compiler::parser::AstNode;
use std::collections::HashMap;

/// Perform semantic analysis on an AST and generate diagnostics
pub fn analyze_semantics(
    uri: &str,
    ast: &qi_compiler::parser::Program,
    diagnostics: &mut Vec<Diagnostic>,
    document_manager: &DocumentManager,
) {
    let mut analyzer = SemanticAnalyzer::new(uri, document_manager);
    analyzer.analyze_program(ast);
    diagnostics.extend(analyzer.diagnostics);
}

/// Semantic analyzer that walks the AST and checks for semantic issues
struct SemanticAnalyzer<'a> {
    /// File URI for error reporting
    uri: &'a str,
    /// Document manager for position conversion
    document_manager: &'a DocumentManager,
    /// Collected diagnostics
    diagnostics: Vec<Diagnostic>,
    /// Symbol table for scope tracking
    symbol_table: HashMap<String, SymbolInfo>,
    /// Current scope depth
    scope_depth: usize,
    /// Function stack for context
    function_stack: Vec<String>,
    /// Used variables tracking
    used_variables: HashMap<String, Vec<Position>>,
    /// Declared variables tracking
    declared_variables: HashMap<String, Vec<SymbolInfo>>,
    /// Current file being analyzed
    current_file: String,
}

/// Information about a symbol
#[derive(Debug, Clone)]
struct SymbolInfo {
    name: String,
    symbol_type: SymbolType,
    span: qi_compiler::lexer::Span,
    scope_depth: usize,
    is_used: bool,
}

/// Types of symbols
#[derive(Debug, Clone)]
enum SymbolType {
    Variable { is_mutable: bool, var_type: Option<String> },
    Function { parameters: Vec<String>, return_type: Option<String> },
    Struct { fields: Vec<String> },
    Enum { variants: Vec<String> },
    Parameter,
    Type,
}

impl<'a> SemanticAnalyzer<'a> {
    /// Create a new semantic analyzer
    fn new(uri: &'a str, document_manager: &'a DocumentManager) -> Self {
        Self {
            uri,
            document_manager,
            diagnostics: Vec::new(),
            symbol_table: HashMap::new(),
            scope_depth: 0,
            function_stack: Vec::new(),
            used_variables: HashMap::new(),
            declared_variables: HashMap::new(),
            current_file: uri.to_string(),
        }
    }

    /// Analyze a complete program
    fn analyze_program(&mut self, program: &qi_compiler::parser::Program) {
        // Analyze package declaration
        if let Some(ref package_name) = program.package_name {
            self.symbol_table.insert(
                package_name.clone(),
                SymbolInfo {
                    name: package_name.clone(),
                    symbol_type: SymbolType::Type,
                    span: Default::default(),
                    scope_depth: 0,
                    is_used: true,
                },
            );
        }

        // Analyze import statements
        for import in &program.imports {
            self.analyze_import(import);
        }

        // Analyze top-level statements
        for statement in &program.statements {
            self.analyze_statement(statement);
        }

        // Check for unused variables
        self.check_unused_variables();
    }

    /// Analyze an import statement
    fn analyze_import(&mut self, import: &qi_compiler::parser::ast::ImportStatement) {
        // Basic import validation
        if import.module_path.is_empty() {
            self.add_diagnostic(
                "导入路径不能为空",
                &import.span,
                DiagnosticSeverity::ERROR,
                "empty-import-path",
            );
        }

        // Track imports for potential circular dependencies
        let import_path_str = import.module_path.join("::");
        self.symbol_table.insert(
            format!("import::{}", import_path_str),
            SymbolInfo {
                name: import_path_str.clone(),
                symbol_type: SymbolType::Type,
                span: import.span.clone(),
                scope_depth: 0,
                is_used: false,
            },
        );
    }

    /// Analyze a statement
    fn analyze_statement(&mut self, statement: &AstNode) {
        use qi_compiler::parser::AstNode;

        match statement {
            AstNode::函数声明(func_decl) => {
                self.analyze_function_declaration(func_decl);
            }
            AstNode::异步函数声明(async_func_decl) => {
                self.analyze_async_function_declaration(async_func_decl);
            }
            AstNode::结构体声明(struct_decl) => {
                self.analyze_struct_declaration(struct_decl);
            }
            AstNode::枚举声明(enum_decl) => {
                self.analyze_enum_declaration(enum_decl);
            }
            AstNode::变量声明(var_decl) => {
                self.analyze_variable_declaration(var_decl);
            }
            AstNode::方法声明(method_decl) => {
                self.analyze_method_declaration(method_decl);
            }
            AstNode::块语句(block_stmt) => {
                self.analyze_block_statement(block_stmt);
            }
            AstNode::如果语句(if_stmt) => {
                self.analyze_if_statement(if_stmt);
            }
            AstNode::当语句(while_stmt) => {
                self.analyze_while_statement(while_stmt);
            }
            AstNode::对于语句(for_stmt) => {
                self.analyze_for_statement(for_stmt);
            }
            AstNode::循环语句(loop_stmt) => {
                self.analyze_loop_statement(loop_stmt);
            }
            AstNode::返回语句(return_stmt) => {
                self.analyze_return_statement(return_stmt);
            }
            AstNode::表达式语句(expr_stmt) => {
                self.analyze_expression_statement(expr_stmt);
            }
            _ => {}
        }
    }

    /// Analyze function declaration
    fn analyze_function_declaration(&mut self, func_decl: &qi_compiler::parser::ast::FunctionDeclaration) {
        // Check for duplicate function names
        if let Some(_existing) = self.symbol_table.get(&func_decl.name) {
            self.add_diagnostic(
                &format!("函数 '{}' 已定义", func_decl.name),
                &func_decl.span,
                DiagnosticSeverity::ERROR,
                "duplicate-function",
            );
        }

        // Add function to symbol table
        self.symbol_table.insert(
            func_decl.name.clone(),
            SymbolInfo {
                name: func_decl.name.clone(),
                symbol_type: SymbolType::Function {
                    parameters: func_decl.parameters.iter().map(|p| p.name.clone()).collect(),
                    return_type: func_decl.return_type.as_ref().map(|t| format!("{:?}", t)),
                },
                span: func_decl.span.clone(),
                scope_depth: self.scope_depth,
                is_used: false,
            },
        );

        // Push function context and analyze body
        self.function_stack.push(func_decl.name.clone());
        self.scope_depth += 1;

        // Analyze parameters
        for param in &func_decl.parameters {
            self.symbol_table.insert(
                param.name.clone(),
                SymbolInfo {
                    name: param.name.clone(),
                    symbol_type: SymbolType::Parameter,
                    span: param.span.clone(),
                    scope_depth: self.scope_depth,
                    is_used: false,
                },
            );
        }

        // Analyze function body
        for stmt in &func_decl.body {
            self.analyze_statement(stmt);
        }

        self.scope_depth -= 1;
        self.function_stack.pop();
    }

    /// Analyze async function declaration
    fn analyze_async_function_declaration(&mut self, async_func_decl: &qi_compiler::parser::ast::AsyncFunctionDeclaration) {
        // Similar to function declaration but marked as async
        if let Some(_existing) = self.symbol_table.get(&async_func_decl.name) {
            self.add_diagnostic(
                &format!("异步函数 '{}' 已定义", async_func_decl.name),
                &async_func_decl.span,
                DiagnosticSeverity::ERROR,
                "duplicate-function",
            );
        }

        self.symbol_table.insert(
            async_func_decl.name.clone(),
            SymbolInfo {
                name: async_func_decl.name.clone(),
                symbol_type: SymbolType::Function {
                    parameters: async_func_decl.parameters.iter().map(|p| p.name.clone()).collect(),
                    return_type: async_func_decl.return_type.as_ref().map(|t| format!("{:?}", t)),
                },
                span: async_func_decl.span.clone(),
                scope_depth: self.scope_depth,
                is_used: false,
            },
        );

        self.function_stack.push(async_func_decl.name.clone());
        self.scope_depth += 1;

        for param in &async_func_decl.parameters {
            self.symbol_table.insert(
                param.name.clone(),
                SymbolInfo {
                    name: param.name.clone(),
                    symbol_type: SymbolType::Parameter,
                    span: param.span.clone(),
                    scope_depth: self.scope_depth,
                    is_used: false,
                },
            );
        }

        for stmt in &async_func_decl.body {
            self.analyze_statement(stmt);
        }

        self.scope_depth -= 1;
        self.function_stack.pop();
    }

    /// Analyze struct declaration
    fn analyze_struct_declaration(&mut self, struct_decl: &qi_compiler::parser::ast::StructDeclaration) {
        if let Some(_existing) = self.symbol_table.get(&struct_decl.name) {
            self.add_diagnostic(
                &format!("结构体 '{}' 已定义", struct_decl.name),
                &struct_decl.span,
                DiagnosticSeverity::ERROR,
                "duplicate-struct",
            );
        }

        let field_names: Vec<String> = struct_decl.fields.iter().map(|f| f.name.clone()).collect();

        self.symbol_table.insert(
            struct_decl.name.clone(),
            SymbolInfo {
                name: struct_decl.name.clone(),
                symbol_type: SymbolType::Struct { fields: field_names },
                span: struct_decl.span.clone(),
                scope_depth: self.scope_depth,
                is_used: false,
            },
        );

        // Analyze methods
        for method in &struct_decl.methods {
            self.analyze_statement(&AstNode::方法声明(method.clone()));
        }
    }

    /// Analyze enum declaration
    fn analyze_enum_declaration(&mut self, enum_decl: &qi_compiler::parser::ast::EnumDeclaration) {
        if let Some(_existing) = self.symbol_table.get(&enum_decl.name) {
            self.add_diagnostic(
                &format!("枚举 '{}' 已定义", enum_decl.name),
                &enum_decl.span,
                DiagnosticSeverity::ERROR,
                "duplicate-enum",
            );
        }

        let variant_names: Vec<String> = enum_decl.variants.iter().map(|v| v.name.clone()).collect();

        self.symbol_table.insert(
            enum_decl.name.clone(),
            SymbolInfo {
                name: enum_decl.name.clone(),
                symbol_type: SymbolType::Enum { variants: variant_names },
                span: enum_decl.span.clone(),
                scope_depth: self.scope_depth,
                is_used: false,
            },
        );
    }

    /// Analyze variable declaration
    fn analyze_variable_declaration(&mut self, var_decl: &qi_compiler::parser::ast::VariableDeclaration) {
        if let Some(_existing) = self.symbol_table.get(&var_decl.name) {
            self.add_diagnostic(
                &format!("变量 '{}' 已在当前作用域中定义", var_decl.name),
                &var_decl.span,
                DiagnosticSeverity::ERROR,
                "duplicate-variable",
            );
        }

        let symbol_info = SymbolInfo {
            name: var_decl.name.clone(),
            symbol_type: SymbolType::Variable {
                is_mutable: var_decl.is_mutable,
                var_type: var_decl.type_annotation.as_ref().map(|t| format!("{:?}", t)),
            },
            span: var_decl.span.clone(),
            scope_depth: self.scope_depth,
            is_used: false,
        };

        self.symbol_table.insert(var_decl.name.clone(), symbol_info.clone());

        // Track declared variables for unused variable checking
        self.declared_variables
            .entry(var_decl.name.clone())
            .or_insert_with(Vec::new)
            .push(symbol_info);

        // Analyze initial value expression if present
        if let Some(ref expr) = var_decl.initializer {
            self.analyze_expression(expr);
        }
    }

    /// Analyze method declaration
    fn analyze_method_declaration(&mut self, method_decl: &qi_compiler::parser::ast::MethodDeclaration) {
        // Method analysis similar to function but with receiver
        self.scope_depth += 1;

        for param in &method_decl.parameters {
            self.symbol_table.insert(
                param.name.clone(),
                SymbolInfo {
                    name: param.name.clone(),
                    symbol_type: SymbolType::Parameter,
                    span: param.span.clone(),
                    scope_depth: self.scope_depth,
                    is_used: false,
                },
            );
        }

        for stmt in &method_decl.body {
            self.analyze_statement(stmt);
        }

        self.scope_depth -= 1;
    }

    /// Analyze block statement
    fn analyze_block_statement(&mut self, block_stmt: &qi_compiler::parser::ast::BlockStatement) {
        self.scope_depth += 1;

        for stmt in &block_stmt.statements {
            self.analyze_statement(stmt);
        }

        self.scope_depth -= 1;
    }

    /// Analyze if statement
    fn analyze_if_statement(&mut self, if_stmt: &qi_compiler::parser::ast::IfStatement) {
        self.analyze_expression(&if_stmt.condition);

        for stmt in &if_stmt.then_branch {
            self.analyze_statement(stmt);
        }

        if let Some(ref else_branch) = if_stmt.else_branch {
            self.analyze_statement(else_branch);
        }
    }

    /// Analyze while statement
    fn analyze_while_statement(&mut self, while_stmt: &qi_compiler::parser::ast::WhileStatement) {
        self.analyze_expression(&while_stmt.condition);

        for stmt in &while_stmt.body {
            self.analyze_statement(stmt);
        }
    }

    /// Analyze for statement
    fn analyze_for_statement(&mut self, for_stmt: &qi_compiler::parser::ast::ForStatement) {
        self.scope_depth += 1;

        // Add loop variable to symbol table
        self.symbol_table.insert(
            for_stmt.variable.clone(),
            SymbolInfo {
                name: for_stmt.variable.clone(),
                symbol_type: SymbolType::Variable {
                    is_mutable: false,
                    var_type: None,
                },
                span: for_stmt.span.clone(),
                scope_depth: self.scope_depth,
                is_used: true, // Loop variables are typically used
            },
        );

        self.analyze_expression(&for_stmt.range);

        for stmt in &for_stmt.body {
            self.analyze_statement(stmt);
        }

        self.scope_depth -= 1;
    }

    /// Analyze loop statement
    fn analyze_loop_statement(&mut self, loop_stmt: &qi_compiler::parser::ast::LoopStatement) {
        for stmt in &loop_stmt.body {
            self.analyze_statement(stmt);
        }
    }

    /// Analyze return statement
    fn analyze_return_statement(&mut self, return_stmt: &qi_compiler::parser::ast::ReturnStatement) {
        if let Some(ref value) = return_stmt.value {
            self.analyze_expression(value);
        }
    }

    /// Analyze expression statement
    fn analyze_expression_statement(&mut self, expr_stmt: &qi_compiler::parser::ast::ExpressionStatement) {
        self.analyze_expression(&expr_stmt.expression);
    }

    /// Analyze an expression
    fn analyze_expression(&mut self, expr: &AstNode) {
        use qi_compiler::parser::AstNode;

        match expr {
            AstNode::标识符表达式(ident_expr) => {
                self.analyze_identifier_expression(ident_expr);
            }
            AstNode::函数调用表达式(func_call) => {
                self.analyze_function_call_expression(func_call);
            }
            AstNode::方法调用表达式(method_call) => {
                self.analyze_method_call_expression(method_call);
            }
            AstNode::二元操作表达式(binary_expr) => {
                self.analyze_binary_expression(binary_expr);
            }
            AstNode::赋值表达式(assign_expr) => {
                self.analyze_assignment_expression(assign_expr);
            }
            AstNode::数组访问表达式(array_access) => {
                self.analyze_array_access_expression(array_access);
            }
            AstNode::等待表达式(await_expr) => {
                self.analyze_await_expression(await_expr);
            }
            _ => {}
        }
    }

    /// Analyze identifier expression
    fn analyze_identifier_expression(&mut self, ident_expr: &qi_compiler::parser::ast::IdentifierExpression) {
        // Check if identifier is defined
        if !self.symbol_table.contains_key(&ident_expr.name) {
            self.add_diagnostic(
                &format!("未定义的符号: '{}'", ident_expr.name),
                &ident_expr.span,
                DiagnosticSeverity::ERROR,
                "undefined-symbol",
            );
        } else {
            // Mark as used
            if let Some(mut symbol) = self.symbol_table.get_mut(&ident_expr.name) {
                symbol.is_used = true;
            }

            // Track usage
            let position = self.span_to_position(&ident_expr.span);
            if let Some(pos) = position {
                self.used_variables
                    .entry(ident_expr.name.clone())
                    .or_insert_with(Vec::new)
                    .push(pos);
            }
        }
    }

    /// Analyze function call expression
    fn analyze_function_call_expression(&mut self, func_call: &qi_compiler::parser::ast::FunctionCallExpression) {
        // Check if function is defined
        if !self.symbol_table.contains_key(&func_call.callee) {
            self.add_diagnostic(
                &format!("未定义的函数: '{}'", func_call.callee),
                &func_call.span,
                DiagnosticSeverity::ERROR,
                "undefined-function",
            );
        }

        // Analyze arguments
        for arg in &func_call.arguments {
            self.analyze_expression(arg);
        }
    }

    /// Analyze method call expression
    fn analyze_method_call_expression(&mut self, method_call: &qi_compiler::parser::ast::MethodCallExpression) {
        self.analyze_expression(&method_call.object);

        for arg in &method_call.arguments {
            self.analyze_expression(arg);
        }
    }

    /// Analyze binary expression
    fn analyze_binary_expression(&mut self, binary_expr: &qi_compiler::parser::ast::BinaryExpression) {
        self.analyze_expression(&binary_expr.left);
        self.analyze_expression(&binary_expr.right);
    }

    /// Analyze assignment expression
    fn analyze_assignment_expression(&mut self, assign_expr: &qi_compiler::parser::ast::AssignmentExpression) {
        self.analyze_expression(&assign_expr.target);
        self.analyze_expression(&assign_expr.value);
    }

    /// Analyze array access expression
    fn analyze_array_access_expression(&mut self, array_access: &qi_compiler::parser::ast::ArrayAccessExpression) {
        self.analyze_expression(&array_access.array);
        self.analyze_expression(&array_access.index);
    }

    /// Analyze await expression
    fn analyze_await_expression(&mut self, await_expr: &qi_compiler::parser::ast::AwaitExpression) {
        self.analyze_expression(&await_expr.expression);
    }

    /// Check for unused variables
    fn check_unused_variables(&mut self) {
        let unused_vars: Vec<(String, Vec<SymbolInfo>)> = self.declared_variables
            .iter()
            .filter(|(var_name, _)| !self.used_variables.contains_key(*var_name) && !var_name.starts_with('_'))
            .map(|(var_name, symbols)| (var_name.clone(), symbols.clone()))
            .collect();

        for (var_name, symbols) in unused_vars {
            for symbol in symbols {
                if !symbol.is_used {
                    self.add_diagnostic(
                        &format!("未使用的变量: '{}'", var_name),
                        &symbol.span,
                        DiagnosticSeverity::WARNING,
                        "unused-variable",
                    );
                }
            }
        }
    }

    /// Add a diagnostic
    fn add_diagnostic(&mut self, message: &str, span: &qi_compiler::lexer::Span, severity: DiagnosticSeverity, code: &str) {
        let range = self.span_to_range(span);
        let diagnostic = Diagnostic {
            range,
            severity: Some(severity),
            code: Some(NumberOrString::String(code.to_string())),
            code_description: None,
            source: Some("qi-semantic-analyzer".to_string()),
            message: message.to_string(),
            related_information: None,
            tags: None,
            data: None,
        };
        self.diagnostics.push(diagnostic);
    }

    /// Convert span to range
    fn span_to_range(&self, span: &qi_compiler::lexer::Span) -> Range {
        crate::definition::span_to_location(span, self.uri, self.document_manager)
            .map(|loc| loc.range)
            .unwrap_or_else(|| Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 0, character: 0 },
            })
    }

    /// Convert span to position
    fn span_to_position(&self, span: &qi_compiler::lexer::Span) -> Option<Position> {
        crate::definition::span_to_location(span, self.uri, self.document_manager)
            .map(|loc| loc.range.start)
    }
}