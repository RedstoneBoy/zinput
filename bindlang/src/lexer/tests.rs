use crate::{
    token::TokenKind,
    util::{Signed, Width},
};

use super::Lexer;

#[test]
fn tokens() {
    use TokenKind as T;

    let src = r#"
    a _b _1c d_2_
    12345
    12345.
    12345.6
    i8 i16 i32 i64 u8 u16 u32 u64
    { } [ ] ( )
    :: : , . ; #
    || && | & ^ !
    + - * /
    > >= < <= == !=
    << >>
    = |= &= ^= += -= *= /=
    else false if let true
    "#;

    #[rustfmt::skip]
    let expected_tokens = [
        T::Ident, T::Ident, T::Ident, T::Ident, 
        T::Int(12345),
        T::Float(12345.0),
        T::Float(12345.6),
        T::IntType(Width::W8, Signed::Yes),
        T::IntType(Width::W16, Signed::Yes),
        T::IntType(Width::W32, Signed::Yes),
        T::IntType(Width::W64, Signed::Yes),
        T::IntType(Width::W8, Signed::No),
        T::IntType(Width::W16, Signed::No),
        T::IntType(Width::W32, Signed::No),
        T::IntType(Width::W64, Signed::No),
        T::LBrace, T::RBrace, T::LBrack, T::RBrack, T::LParen, T::RParen,
        T::DoubleColon, T::Colon, T::Comma, T::Dot, T::Semicolon, T::Hash,
        T::Or, T::And, T::BitOr, T::BitAnd, T::Xor, T::Not,
        T::Plus, T::Minus, T::Star, T::Slash,
        T::Greater, T::GreaterEq, T::Less, T::LessEq, T::Equals, T::NotEquals,
        T::ShiftLeft, T::ShiftRight,
        T::Assign, T::BitOrAssign, T::BitAndAssign, T::XorAssign,
        T::AddAssign, T::SubAssign, T::MulAssign, T::DivAssign,
        T::KElse, T::KFalse, T::KIf, T::KLet, T::KTrue,
    ];

    let lexer = Lexer::new(src);
    let (tokens, errors) = lexer.scan();
    assert_eq!(errors.len(), 0);
    assert_eq!(tokens.len(), expected_tokens.len());
    for (got, expected) in tokens.iter().map(|tok| &tok.kind).zip(&expected_tokens) {
        assert_eq!(got, expected, "expected {expected:?}, got {got:?}");
    }
}
