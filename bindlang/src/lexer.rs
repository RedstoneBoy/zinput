use std::{iter::Peekable, str::Chars};

use crate::{
    span::{Pos, Span},
    token::{Token, TokenKind},
};

#[cfg(test)]
mod tests;

pub struct Lexer<'a> {
    src: &'a str,
    chars: Peekable<Chars<'a>>,

    errors: Vec<LexerError>,

    pos: Pos,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Lexer {
            src,
            chars: src.chars().peekable(),
            errors: Vec::new(),
            pos: Pos {
                index: 0,
                line: 1,
                col: 1,
            },
        }
    }

    pub fn scan(mut self) -> (Vec<Token>, Vec<LexerError>) {
        let mut tokens = Vec::new();

        loop {
            let token = match self.next() {
                Ok(token) => token,
                Err(NextError::Skip) => continue,
                Err(NextError::Eof) => break,
            };
            tokens.push(token);
        }

        (tokens, self.errors)
    }

    fn next(&mut self) -> Result<Token, NextError> {
        let mut start = self.pos;
        let mut ch = self.next_char()?;

        while ch.is_whitespace() {
            start = self.pos;
            ch = self.next_char()?;
        }

        Ok(match ch {
            // Ident
            _ if Self::is_ident_start(ch) => {
                while let Some(&ch) = self.chars.peek() {
                    if Self::is_ident(ch) {
                        self.next_char_unwrap("unwrap in ident");
                    } else {
                        break;
                    }
                }
                let end = self.pos;

                Token {
                    span: Span { start, end },
                    kind: TokenKind::ident(&self.src[start.index..end.index]),
                }
            }

            // Numbers
            _ if ch.is_ascii_digit() => {
                loop {
                    match self.chars.peek().cloned() {
                        Some('.') => {
                            self.next_char_unwrap("number dot unwrap");
                            break;
                        }
                        Some(ch) if ch.is_ascii_digit() => {
                            self.next_char_unwrap("number digit unwrap");
                        }
                        _ => {
                            let end = self.pos;
                            return Ok(Token {
                                span: Span { start, end },
                                kind: TokenKind::Int(
                                    self.src[start.index..end.index]
                                        .parse()
                                        .expect("internal lexer error: error parsing int"),
                                ),
                            });
                        }
                    }
                }

                // float match

                loop {
                    match self.chars.peek().cloned() {
                        Some(ch) if ch.is_ascii_digit() => {
                            self.next_char_unwrap("float digit unwrap");
                        }
                        _ => break,
                    }
                }

                let end = self.pos;

                Token {
                    span: Span { start, end },
                    kind: TokenKind::Float(
                        self.src[start.index..end.index]
                            .parse()
                            .expect("internal lexer error: error parsing float"),
                    ),
                }
            }

            '{' => self.single(TokenKind::LBrace, start),
            '}' => self.single(TokenKind::RBrace, start),
            '[' => self.single(TokenKind::LBrack, start),
            ']' => self.single(TokenKind::RBrack, start),
            '(' => self.single(TokenKind::LParen, start),
            ')' => self.single(TokenKind::RParen, start),

            ':' => self.double(
                TokenKind::Colon,
                |ch| match ch {
                    ':' => Some(TokenKind::DoubleColon),
                    _ => None,
                },
                start,
            ),
            ',' => self.single(TokenKind::Comma, start),
            '.' => self.single(TokenKind::Dot, start),
            ';' => self.single(TokenKind::Semicolon, start),
            '#' => self.single(TokenKind::Hash, start),

            '|' => self.double(
                TokenKind::BitOr,
                |ch| match ch {
                    '|' => Some(TokenKind::Or),
                    '=' => Some(TokenKind::BitOrAssign),
                    _ => None,
                },
                start,
            ),
            '&' => self.double(
                TokenKind::BitAnd,
                |ch| match ch {
                    '&' => Some(TokenKind::And),
                    '=' => Some(TokenKind::BitAndAssign),
                    _ => None,
                },
                start,
            ),
            '^' => self.double(
                TokenKind::Xor,
                |ch| match ch {
                    '=' => Some(TokenKind::XorAssign),
                    _ => None,
                },
                start,
            ),
            '!' => self.double(
                TokenKind::Not,
                |ch| match ch {
                    '=' => Some(TokenKind::NotEquals),
                    _ => None,
                },
                start,
            ),

            '+' => self.double(
                TokenKind::Plus,
                |ch| match ch {
                    '=' => Some(TokenKind::AddAssign),
                    _ => None,
                },
                start,
            ),
            '-' => self.double(
                TokenKind::Minus,
                |ch| match ch {
                    '=' => Some(TokenKind::SubAssign),
                    _ => None,
                },
                start,
            ),
            '*' => self.double(
                TokenKind::Star,
                |ch| match ch {
                    '=' => Some(TokenKind::MulAssign),
                    _ => None,
                },
                start,
            ),
            '/' => self.double(
                TokenKind::Slash,
                |ch| match ch {
                    '=' => Some(TokenKind::DivAssign),
                    _ => None,
                },
                start,
            ),

            '>' => self.double(
                TokenKind::Greater,
                |ch| match ch {
                    '>' => Some(TokenKind::ShiftRight),
                    '=' => Some(TokenKind::GreaterEq),
                    _ => None,
                },
                start,
            ),
            '<' => self.double(
                TokenKind::Less,
                |ch| match ch {
                    '<' => Some(TokenKind::ShiftLeft),
                    '=' => Some(TokenKind::LessEq),
                    _ => None,
                },
                start,
            ),
            '=' => self.double(
                TokenKind::Assign,
                |ch| match ch {
                    '=' => Some(TokenKind::Equals),
                    _ => None,
                },
                start,
            ),

            other => {
                self.errors.push(LexerError {
                    span: Span {
                        start,
                        end: self.pos,
                    },
                    kind: LexerErrorKind::InvalidCharacter(other),
                });
                return Err(NextError::Skip);
            }
        })
    }

    fn next_char(&mut self) -> Result<char, NextError> {
        self.chars
            .next()
            .map(|ch| {
                self.pos.index += ch.len_utf8();
                self.pos.col += 1;
                if ch == '\n' {
                    self.pos.col = 1;
                    self.pos.line += 1;
                }
                ch
            })
            .ok_or(NextError::Eof)
    }

    fn next_char_unwrap(&mut self, msg: &'static str) -> char {
        self.next_char()
            .map_err(|_| format!("internal lexer error: {}", msg))
            .unwrap()
    }

    fn double<F>(&mut self, kind: TokenKind, find: F, start: Pos) -> Token
    where
        F: FnOnce(char) -> Option<TokenKind>,
    {
        let found = self.chars.peek().cloned().and_then(find);

        if found.is_some() {
            self.next_char_unwrap("unwrap in double");
        }

        let kind = found.unwrap_or(kind);

        Token {
            span: Span {
                start,
                end: self.pos,
            },
            kind,
        }
    }

    fn single(&self, kind: TokenKind, start: Pos) -> Token {
        Token {
            span: Span {
                start,
                end: self.pos,
            },
            kind,
        }
    }

    fn is_ident_start(ch: char) -> bool {
        ch == '_' || ch.is_alphabetic()
    }

    fn is_ident(ch: char) -> bool {
        Self::is_ident_start(ch) || ch.is_numeric()
    }
}

#[derive(Clone, Debug)]
pub struct LexerError {
    pub kind: LexerErrorKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum LexerErrorKind {
    InvalidCharacter(char),
}

enum NextError {
    Skip,
    Eof,
}
