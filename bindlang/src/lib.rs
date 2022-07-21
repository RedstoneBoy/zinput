pub mod ast;
mod error;
mod ir;
mod lexer;
mod parser;
pub mod span;
mod token;
pub mod ty;
mod typecheck;

use std::collections::HashMap;

pub use error::Errors;

pub fn parse(source: &str) -> Result<ast::Module, Errors> {
    let lexer = lexer::Lexer::new(source);
    let (tokens, lexer_errors) = lexer.scan();
    let parser = parser::Parser::new(source, tokens);

    let mut module= match parser.parse() {
        Ok(module) => module,
        Err(parser_errors) => return Err(Errors::new(source, lexer_errors, parser_errors, Vec::new())),
    };

    match typecheck::TypeChecker::new(source).check(&mut module, HashMap::new()) {
        Ok(()) => {},
        Err(type_errors) => return Err(Errors::new(source, lexer_errors, Vec::new(), type_errors)),
    }

    if !lexer_errors.is_empty() {
        return Err(Errors::new(source, lexer_errors, Vec::new(), Vec::new()));
    }

    Ok(module)
}
