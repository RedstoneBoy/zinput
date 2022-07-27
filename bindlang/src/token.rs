use std::fmt::{Display, Formatter, Result};

use crate::{
    span::Span,
    util::{Signed, Width},
};

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

    IntType(Width, Signed),

    LBrace,
    RBrace,
    LBrack,
    RBrack,
    LParen,
    RParen,

    DoubleColon,
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
            "u8" => TokenKind::IntType(Width::W8, Signed::No),
            "u16" => TokenKind::IntType(Width::W16, Signed::No),
            "u32" => TokenKind::IntType(Width::W32, Signed::No),
            "u64" => TokenKind::IntType(Width::W64, Signed::No),
            "i8" => TokenKind::IntType(Width::W8, Signed::Yes),
            "i16" => TokenKind::IntType(Width::W16, Signed::Yes),
            "i32" => TokenKind::IntType(Width::W32, Signed::Yes),
            "i64" => TokenKind::IntType(Width::W64, Signed::Yes),

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

            IntType(Width::W8, Signed::No) => write!(f, "u8"),
            IntType(Width::W16, Signed::No) => write!(f, "u16"),
            IntType(Width::W32, Signed::No) => write!(f, "u32"),
            IntType(Width::W64, Signed::No) => write!(f, "u64"),
            IntType(Width::W8, Signed::Yes) => write!(f, "i8"),
            IntType(Width::W16, Signed::Yes) => write!(f, "i16"),
            IntType(Width::W32, Signed::Yes) => write!(f, "i32"),
            IntType(Width::W64, Signed::Yes) => write!(f, "i64"),

            LBrace => write!(f, "{{"),
            RBrace => write!(f, "}}"),
            LBrack => write!(f, "["),
            RBrack => write!(f, "]"),
            LParen => write!(f, "("),
            RParen => write!(f, ")"),

            DoubleColon => write!(f, "::"),
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
