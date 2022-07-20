pub mod ast;
mod error;
mod ir;
mod lexer;
mod parser;
pub mod span;
mod token;
pub mod ty;
mod typecheck;

pub use error::Errors;

pub fn parse(source: &str) -> Result<ast::Module, Errors> {
    let lexer = lexer::Lexer::new(source);
    let (tokens, lexer_errors) = lexer.scan();
    let parser = parser::Parser::new(source, tokens);

    match parser.parse() {
        Ok(module) if lexer_errors.is_empty() => Ok(module),
        Ok(_) => Err(Errors::new(source, lexer_errors, Vec::new())),
        Err(parser_errors) => Err(Errors::new(source, lexer_errors, parser_errors)),
    }
}
