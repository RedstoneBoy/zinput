#![feature(let_else)]

pub mod ast;
pub mod backend_cranelift;
mod error;
mod lexer;
mod parser;
pub mod span;
mod token;
pub mod ty;
mod typecheck;
pub mod util;

use std::collections::HashMap;

pub use error::Errors;
use ty::Type;

pub fn compile_native(source: &str, device_type: Type) -> Result<Vec<backend_cranelift::CompiledFunction>, Errors> {
    let lexer = lexer::Lexer::new(source);
    let (tokens, lexer_errors) = lexer.scan();
    let parser = parser::Parser::new(source, tokens);

    let mut module = match parser.parse() {
        Ok(module) => module,
        Err(parser_errors) => {
            return Err(Errors::new(source, lexer_errors, parser_errors, Vec::new()))
        }
    };

    let mut globals = HashMap::new();
    globals.insert(module.output.index_src(source), device_type.clone());
    for input in &module.inputs {
        globals.insert(input.device.index_src(source), device_type.clone());
    }
    match typecheck::TypeChecker::new(source).check(&mut module, globals) {
        Ok(()) => {}
        Err(type_errors) => return Err(Errors::new(source, lexer_errors, Vec::new(), type_errors)),
    }

    if !lexer_errors.is_empty() {
        return Err(Errors::new(source, lexer_errors, Vec::new(), Vec::new()));
    }

    let compiler = backend_cranelift::Compiler::new(source);
    Ok(compiler.compile(module))
}