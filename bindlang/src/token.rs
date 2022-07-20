use std::fmt::{Display, Formatter, Result};

use crate::span::Span;

#[derive(Clone, Debug)]
pub struct Token {
    pub span: Span,
    pub kind: TokenKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Ident,
    Int(u64),
    Float(f64),

    LBrace,
    RBrace,
    LBrack,
    RBrack,
    LParen,
    RParen,

    Colon,
    Comma,
    Dot,
    Semicolon,
    Hash,

    BitOr,
    BitAnd,
    Or,
    And,
    Xor,
    Not,

    Plus,
    Minus,
    Star,
    Slash,

    Greater,
    GreaterEq,
    Less,
    LessEq,
    Equals,
    NotEquals,

    ShiftLeft,
    ShiftRight,

    Assign,
    BitOrAssign,
    BitAndAssign,
    XorAssign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,

    KElse,
    KFalse,
    KIf,
    KLet,
    KTrue,
}

impl TokenKind {
    pub fn ident(ident: &str) -> TokenKind {
        match ident {
            "else" => TokenKind::KElse,
            "false" => TokenKind::KFalse,
            "if" => TokenKind::KIf,
            "let" => TokenKind::KLet,
            "true" => TokenKind::KTrue,
            _ => TokenKind::Ident,
        }
    }
}

impl Display for TokenKind {
    fn fmt(&self, f: &mut Formatter) -> Result {
        use TokenKind::*;
        match self {
            Ident => write!(f, "{{identifier}}"),
            Int(_) => write!(f, "{{int}}"),
            Float(_) => write!(f, "{{float}}"),

            LBrace => write!(f, "{{"),
            RBrace => write!(f, "}}"),
            LBrack => write!(f, "["),
            RBrack => write!(f, "]"),
            LParen => write!(f, "("),
            RParen => write!(f, ")"),

            Colon => write!(f, ":"),
            Comma => write!(f, ";"),
            Dot => write!(f, "."),
            Semicolon => write!(f, ";"),
            Hash => write!(f, "#"),

            BitOr => write!(f, "|"),
            BitAnd => write!(f, "&"),
            Or => write!(f, "||"),
            And => write!(f, "&&"),
            Xor => write!(f, "^"),
            Not => write!(f, "!"),

            Plus => write!(f, "+"),
            Minus => write!(f, "-"),
            Star => write!(f, "*"),
            Slash => write!(f, "/"),

            Greater => write!(f, "?"),
            GreaterEq => write!(f, ">="),
            Less => write!(f, "<"),
            LessEq => write!(f, "<="),
            Equals => write!(f, "=="),
            NotEquals => write!(f, "!="),

            ShiftLeft => write!(f, "<<"),
            ShiftRight => write!(f, ">>"),

            Assign => write!(f, "="),
            BitOrAssign => write!(f, "|="),
            BitAndAssign => write!(f, "&="),
            XorAssign => write!(f, "^="),
            AddAssign => write!(f, "+="),
            SubAssign => write!(f, "-="),
            MulAssign => write!(f, "*="),
            DivAssign => write!(f, "/="),

            KElse => write!(f, "else"),
            KFalse => write!(f, "false"),
            KIf => write!(f, "if"),
            KLet => write!(f, "let"),
            KTrue => write!(f, "true"),
        }
    }
}
