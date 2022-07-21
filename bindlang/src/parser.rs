use std::{iter::Peekable, vec::IntoIter};

use crate::{
    ast::{
        AssignKind, BinOp, Block, Expr, ExprKind, Literal, Module, Stmt, StmtKind,
        UnOp, DeviceIn,
    },
    span::Span,
    token::{Token, TokenKind},
};

const KW_DEVICES: &'static str = "devices";
const KW_IN: &'static str = "in";
const KW_OUT: &'static str = "out";

pub struct Parser<'a> {
    src: &'a str,
    tokens: Peekable<IntoIter<Token>>,

    errors: Vec<ParserError>,
}

impl<'a> Parser<'a> {
    pub fn new(src: &'a str, tokens: Vec<Token>) -> Self {
        Parser {
            src,
            tokens: tokens.into_iter().peekable(),

            errors: Vec::new(),
        }
    }

    pub fn parse(mut self) -> Result<Module, Vec<ParserError>> {
        match self.parse_module() {
            Some(module) if self.errors.is_empty() => Ok(module),
            _ => Err(self.errors),
        }
    }

    fn parse_module(&mut self) -> Option<Module> {
        self.eat_ident_kw("device")?;
        let output = self.eat_token(TokenKind::Ident)?.span;

        let mut inputs = Vec::new();

        while self.tokens.peek().is_some() {
            let input = self.parse_input()?;
            inputs.push(input);
        }

        Some(Module { output, inputs })
    }

    fn parse_input(&mut self) -> Option<DeviceIn> {
        let device = self.eat_token(TokenKind::Ident)?.span;
        let start = device.start;

        let body = self.parse_block()?;
        let end = body.span.end;

        let span = Span { start, end };

        Some(DeviceIn {
            device,
            body,

            span,
        })
    }

    fn parse_block(&mut self) -> Option<Block> {
        let start = self.eat_token(TokenKind::LBrace)?.span.start;

        let mut stmts = Vec::new();

        loop {
            if self.peek_token(TokenKind::RBrace) {
                break;
            }

            let stmt = self.parse_stmt()?;
            stmts.push(stmt);
        }

        let end = self.eat_token(TokenKind::RBrace)?.span.end;
        let span = Span { start, end };

        Some(Block { stmts, span })
    }

    fn parse_stmt(&mut self) -> Option<Stmt> {
        if let Some(tok) = self.maybe_eat_token(TokenKind::KLet) {
            return self.eat_stmt_let(tok);
        }

        if let Some(tok) = self.maybe_eat_token(TokenKind::KIf) {
            return self.eat_stmt_if(tok);
        }

        let lval = self.parse_expr()?;

        let start = lval.span.start;

        let assign_kind = self.tokens.peek().and_then(|tok| {
            Some(match &tok.kind {
                TokenKind::Assign => AssignKind::Normal,
                TokenKind::BitOrAssign => AssignKind::BitOr,
                TokenKind::BitAndAssign => AssignKind::BitAnd,
                TokenKind::XorAssign => AssignKind::Xor,
                TokenKind::AddAssign => AssignKind::Add,
                TokenKind::SubAssign => AssignKind::Sub,
                TokenKind::MulAssign => AssignKind::Mul,
                TokenKind::DivAssign => AssignKind::Div,
                _ => return None,
            })
        });

        if let Some(kind) = assign_kind {
            self.eat_any_token().unwrap();

            let expr = self.parse_expr()?;

            let end = self.eat_token(TokenKind::Semicolon)?.span.end;

            Some(Stmt {
                span: Span { start, end },
                kind: StmtKind::Assign { lval, kind, expr },
            })
        } else {
            let end = self.eat_token(TokenKind::Semicolon)?.span.end;

            Some(Stmt {
                span: Span { start, end },
                kind: StmtKind::Expr(lval),
            })
        }
    }

    fn eat_stmt_let(&mut self, tok_let: Token) -> Option<Stmt> {
        let start = tok_let.span.start;

        let name = self.eat_token(TokenKind::Ident)?.span;

        self.eat_token(TokenKind::Assign)?;

        let expr = self.parse_expr()?;

        let end = self.eat_token(TokenKind::Semicolon)?.span.end;

        Some(Stmt {
            span: Span { start, end },
            kind: StmtKind::Let { name, expr },
        })
    }

    fn eat_stmt_if(&mut self, tok_if: Token) -> Option<Stmt> {
        let start = tok_if.span.start;

        let cond = self.parse_expr()?;

        let yes = self.parse_block()?;

        if self.maybe_eat_token(TokenKind::KElse).is_none() {
            let end = yes.span.end;

            return Some(Stmt {
                span: Span { start, end },
                kind: StmtKind::If {
                    cond,
                    yes,
                    no: None,
                },
            });
        }

        if let Some(tok_if) = self.maybe_eat_token(TokenKind::KIf) {
            let no_start = tok_if.span.start;

            let else_if = self.eat_stmt_if(tok_if)?;
            let end = else_if.span.end;

            let no = Block {
                stmts: vec![else_if],
                span: Span {
                    start: no_start,
                    end,
                },
            };

            Some(Stmt {
                span: Span { start, end },
                kind: StmtKind::If {
                    cond,
                    yes,
                    no: Some(no),
                },
            })
        } else if self.peek_token(TokenKind::LBrace) {
            let no = self.parse_block()?;
            let end = no.span.end;

            Some(Stmt {
                span: Span { start, end },
                kind: StmtKind::If {
                    cond,
                    yes,
                    no: Some(no),
                },
            })
        } else {
            let got = self.eat_any_token()?;
            self.errors.push(ParserError::UnexpectedToken {
                got,
                expected: vec![TokenKind::KIf, TokenKind::LBrace],
            });

            None
        }
    }

    fn parse_expr(&mut self) -> Option<Expr> {
        self.parse_expr_l1()
    }

    // parse bool or
    fn parse_expr_l1(&mut self) -> Option<Expr> {
        self.parse_bin_op(
            |tk| {
                Some(match tk {
                    TokenKind::Or => BinOp::Or,
                    _ => return None,
                })
            },
            Self::parse_expr_l2,
            true,
        )
    }

    // parse bool or
    fn parse_expr_l2(&mut self) -> Option<Expr> {
        self.parse_bin_op(
            |tk| {
                Some(match tk {
                    TokenKind::And => BinOp::And,
                    _ => return None,
                })
            },
            Self::parse_expr_l3,
            true,
        )
    }

    // parse comparison
    fn parse_expr_l3(&mut self) -> Option<Expr> {
        self.parse_bin_op(
            |tk| {
                Some(match tk {
                    TokenKind::Equals => BinOp::Equals,
                    TokenKind::NotEquals => BinOp::NotEquals,
                    TokenKind::Greater => BinOp::Greater,
                    TokenKind::GreaterEq => BinOp::GreaterEq,
                    TokenKind::Less => BinOp::Less,
                    TokenKind::LessEq => BinOp::LessEq,
                    _ => return None,
                })
            },
            Self::parse_expr_l4,
            false,
        )
    }

    // parse bit or
    fn parse_expr_l4(&mut self) -> Option<Expr> {
        self.parse_bin_op(
            |tk| {
                Some(match tk {
                    TokenKind::BitOr => BinOp::BitOr,
                    _ => return None,
                })
            },
            Self::parse_expr_l5,
            true,
        )
    }

    // parse xor
    fn parse_expr_l5(&mut self) -> Option<Expr> {
        self.parse_bin_op(
            |tk| {
                Some(match tk {
                    TokenKind::Xor => BinOp::BitXor,
                    _ => return None,
                })
            },
            Self::parse_expr_l6,
            true,
        )
    }

    // parse bit and
    fn parse_expr_l6(&mut self) -> Option<Expr> {
        self.parse_bin_op(
            |tk| {
                Some(match tk {
                    TokenKind::BitAnd => BinOp::BitAnd,
                    _ => return None,
                })
            },
            Self::parse_expr_l7,
            true,
        )
    }

    // parse shift
    fn parse_expr_l7(&mut self) -> Option<Expr> {
        self.parse_bin_op(
            |tk| {
                Some(match tk {
                    TokenKind::ShiftLeft => BinOp::ShiftLeft,
                    TokenKind::ShiftRight => BinOp::ShiftRight,
                    _ => return None,
                })
            },
            Self::parse_expr_l8,
            true,
        )
    }

    // parse add sub
    fn parse_expr_l8(&mut self) -> Option<Expr> {
        self.parse_bin_op(
            |tk| {
                Some(match tk {
                    TokenKind::Plus => BinOp::Add,
                    TokenKind::Minus => BinOp::Sub,
                    _ => return None,
                })
            },
            Self::parse_expr_l9,
            true,
        )
    }

    // parse mul
    fn parse_expr_l9(&mut self) -> Option<Expr> {
        self.parse_bin_op(
            |tk| {
                Some(match tk {
                    TokenKind::Star => BinOp::Mul,
                    TokenKind::Slash => BinOp::Div,
                    _ => return None,
                })
            },
            Self::parse_expr_l10,
            true,
        )
    }

    // parse shift
    fn parse_expr_l10(&mut self) -> Option<Expr> {
        self.parse_bin_op(
            |tk| {
                Some(match tk {
                    TokenKind::ShiftLeft => BinOp::ShiftLeft,
                    TokenKind::ShiftRight => BinOp::ShiftRight,
                    _ => return None,
                })
            },
            Self::parse_expr_l11,
            true,
        )
    }

    // parse unary
    fn parse_expr_l11(&mut self) -> Option<Expr> {
        if let Some(tok) = self.maybe_eat_token(TokenKind::Minus) {
            let expr = self.parse_expr_l12()?;
            Some(Expr {
                span: Span {
                    start: tok.span.start,
                    end: expr.span.end,
                },
                kind: ExprKind::Unary(UnOp::Negate, Box::new(expr)),
                ty: None,
            })
        } else if let Some(tok) = self.maybe_eat_token(TokenKind::Not) {
            let expr = self.parse_expr_l12()?;
            Some(Expr {
                span: Span {
                    start: tok.span.start,
                    end: expr.span.end,
                },
                kind: ExprKind::Unary(UnOp::Not, Box::new(expr)),
                ty: None,
            })
        } else {
            self.parse_expr_l12()
        }
    }

    // parse placeholder
    fn parse_expr_l12(&mut self) -> Option<Expr> {
        let expr = self.parse_expr_l13()?;

        Some(expr)
    }

    // parse index
    fn parse_expr_l13(&mut self) -> Option<Expr> {
        let expr = self.parse_expr_l14()?;

        if let Some(_) = self.maybe_eat_token(TokenKind::LBrack) {
            let index = self.parse_expr()?;
            let end = self.eat_token(TokenKind::RBrack)?.span.end;

            Some(Expr {
                span: Span {
                    start: expr.span.start,
                    end,
                },
                kind: ExprKind::Index(Box::new(expr), Box::new(index)),
                ty: None,
            })
        } else {
            Some(expr)
        }
    }

    // parse dot
    fn parse_expr_l14(&mut self) -> Option<Expr> {
        let mut expr = self.parse_expr_l15()?;

        while let Some(_) = self.maybe_eat_token(TokenKind::Dot) {
            let ident = self.eat_token(TokenKind::Ident)?;
            expr = Expr {
                span: Span {
                    start: expr.span.start,
                    end: ident.span.end,
                },
                kind: ExprKind::Dot(Box::new(expr), ident.span),
                ty: None,
            };
        }

        Some(expr)
    }

    fn parse_expr_l15(&mut self) -> Option<Expr> {
        let tok = self.eat_any_token()?;
        Some(match &tok.kind {
            TokenKind::KTrue => {
                let span = tok.span;
                Expr {
                    span,
                    kind: ExprKind::Literal(Literal::Bool(true)),
                    ty: None,
                }
            }
            TokenKind::KFalse => {
                let span = tok.span;
                Expr {
                    span,
                    kind: ExprKind::Literal(Literal::Bool(false)),
                    ty: None,
                }
            }
            TokenKind::Int(val) => {
                let span = tok.span;
                Expr {
                    span,
                    kind: ExprKind::Literal(Literal::Int(*val)),
                    ty: None,
                }
            }
            TokenKind::Float(val) => {
                let span = tok.span;
                Expr {
                    span,
                    kind: ExprKind::Literal(Literal::Float(*val)),
                    ty: None,
                }
            }
            TokenKind::Ident => {
                let span = tok.span;
                Expr {
                    span,
                    kind: ExprKind::Var(span),
                    ty: None,
                }
            }
            TokenKind::LParen => {
                let expr = self.parse_expr()?;
                self.eat_token(TokenKind::RParen)?;
                expr
            }
            _ => {
                self.errors.push(ParserError::UnexpectedToken {
                    got: tok,
                    expected: vec![
                        TokenKind::KTrue,
                        TokenKind::KFalse,
                        TokenKind::Int(0),
                        TokenKind::Float(0.0),
                    ],
                });
                return None;
            }
        })
    }

    fn parse_bin_op<T, F>(
        &mut self,
        mut token_to_op: T,
        mut next_level: F,
        multiple: bool,
    ) -> Option<Expr>
    where
        T: FnMut(&TokenKind) -> Option<BinOp>,
        F: FnMut(&mut Self) -> Option<Expr>,
    {
        let mut left = next_level(self)?;

        while let Some(tok) = self.tokens.peek() {
            if let Some(bin_op) = token_to_op(&tok.kind) {
                self.eat_any_token().unwrap();

                let right = next_level(self)?;
                left = Expr {
                    span: Span {
                        start: left.span.start,
                        end: right.span.end,
                    },
                    kind: ExprKind::Binary(Box::new(left), bin_op, Box::new(right)),
                    ty: None,
                };
            } else {
                break;
            }

            if !multiple {
                break;
            }
        }

        Some(left)
    }

    fn eat_ident_kw(&mut self, kw: &'static str) -> Option<Token> {
        let token = match self.tokens.next() {
            Some(token) => token,
            None => {
                self.errors.push(ParserError::UnexpectedEof);
                return None;
            }
        };

        match &token.kind {
            TokenKind::Ident if &self.src[token.span.start.index..token.span.end.index] == kw => {
                Some(token)
            }
            _ => {
                self.errors.push(ParserError::ExpectedIdentKeyWord {
                    got: token,
                    expected: kw,
                });

                None
            }
        }
    }

    fn eat_token(&mut self, kind: TokenKind) -> Option<Token> {
        let token = self.eat_any_token()?;

        if token.kind == kind {
            Some(token)
        } else {
            self.errors.push(ParserError::UnexpectedToken {
                got: token,
                expected: vec![kind],
            });
            None
        }
    }

    fn eat_any_token(&mut self) -> Option<Token> {
        match self.tokens.next() {
            Some(token) => Some(token),
            None => {
                self.errors.push(ParserError::UnexpectedEof);
                return None;
            }
        }
    }

    fn peek_token(&mut self, kind: TokenKind) -> bool {
        self.tokens
            .peek()
            .map(|tok| tok.kind == kind)
            .unwrap_or(false)
    }

    fn maybe_eat_token(&mut self, kind: TokenKind) -> Option<Token> {
        if self.peek_token(kind.clone()) {
            self.eat_token(kind)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub enum ParserError {
    UnexpectedToken {
        got: Token,
        expected: Vec<TokenKind>,
    },
    ExpectedIdentKeyWord {
        got: Token,
        expected: &'static str,
    },
    UnexpectedEof,
}
