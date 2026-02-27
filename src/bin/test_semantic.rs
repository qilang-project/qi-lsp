//! Test semantic analysis functionality

use qi_lsp::diagnostics::semantic;
use qi_lsp::document::DocumentManager;

fn main() {
    // Create test content with semantic issues
    let test_content = r#"
包 测试包;

函数 测试函数() {
    变量 未使用变量 = 42;
    变量 正常变量 = 10;
    打印行("值:", 正常变量);
    变量 重复声明 = 1;
    变量 重复声明 = 2;
    打印行(未定义符号);
    未定义函数();
    返回 正常变量;
}

函数 测试函数() {
    返回 0;
}
"#;

    println!("🧪 测试语义分析功能...\n");

    // Create document manager
    let document_manager = DocumentManager::new();
    let uri = "file:///test_semantic.qi";

    // Open document
    document_manager.open_document(uri, test_content.to_string());

    // Get AST
    match document_manager.get_document_ast(uri) {
        Some(ast) => {
            println!("✅ AST 解析成功");

            // Run semantic analysis
            let mut diagnostics = Vec::new();
            semantic::analyze_semantics(uri, &ast, &mut diagnostics, &document_manager);

            println!("📊 发现 {} 个语义问题:\n", diagnostics.len());

            // Print diagnostics
            for (i, diagnostic) in diagnostics.iter().enumerate() {
                println!("{}. {} [{:?}]",
                    i + 1,
                    diagnostic.message,
                    diagnostic.severity
                );
                if let Some(code) = &diagnostic.code {
                    if let lsp_types::NumberOrString::String(code_str) = code {
                        println!("   代码: {}", code_str);
                    }
                }
                println!("   位置: 行 {}-{}",
                    diagnostic.range.start.line + 1,
                    diagnostic.range.end.line + 1
                );
                println!();
            }

            if diagnostics.is_empty() {
                println!("✅ 没有发现语义问题");
            } else {
                println!("🔍 语义分析功能正常工作！检测到以下类型的问题:");
                let mut unused_vars = 0;
                let mut undefined_symbols = 0;
                let mut duplicate_decls = 0;

                for diagnostic in &diagnostics {
                    if let Some(code) = &diagnostic.code {
                        if let lsp_types::NumberOrString::String(code_str) = code {
                            match code_str.as_str() {
                                "unused-variable" => unused_vars += 1,
                                "undefined-symbol" | "undefined-function" => undefined_symbols += 1,
                                "duplicate-function" | "duplicate-variable" => duplicate_decls += 1,
                                _ => {}
                            }
                        }
                    }
                }

                println!("  - 未使用变量: {} 个", unused_vars);
                println!("  - 未定义符号: {} 个", undefined_symbols);
                println!("  - 重复声明: {} 个", duplicate_decls);
            }

        }
        None => {
            println!("❌ AST 解析失败");
        }
    }

    // Close document
    document_manager.close_document(uri);
    println!("\n🎉 语义分析测试完成！");
}