use std::fmt::{Display, Formatter, Result};

use crate::span::Span;

pub type Ident = Span;

#[derive(Clone, Debug)]
pub struct Module {
    pub devices: Devices,
    pub events: Vec<Event>,
}

impl Module {
    pub fn display<'a, 'b>(&'a self, source: &'b str) -> AstDisplay<'a, 'b> {
        AstDisplay {
            source,
            module: self,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Devices {
    pub d_in: Vec<Ident>,
    pub d_out: Ident,

    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Event {
    pub source: Ident,
    pub kind: Ident,
    pub body: Block,

    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Block {
    pub stmts: Vec<Stmt>,

    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Stmt {
    pub kind: StmtKind,

    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum StmtKind {
    Let {
        name: Ident,
        expr: Expr,
    },
    Assign {
        lval: Expr,
        kind: AssignKind,
        expr: Expr,
    },
    If {
        cond: Expr,
        yes: Block,
        no: Option<Block>,
    },
    Expr(Expr),
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,

    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Literal(Literal),
    Var(Ident),
    Dot(Box<Expr>, Ident),
    Index(Box<Expr>, Box<Expr>),

    Unary(UnOp, Box<Expr>),
    Binary(Box<Expr>, BinOp, Box<Expr>),
}

#[derive(Copy, Clone, Debug)]
pub enum UnOp {
    Negate,
    Not,
}

#[derive(Copy, Clone, Debug)]
pub enum BinOp {
    BitOr,
    BitAnd,
    BitXor,
    Or,
    And,

    Add,
    Sub,
    Mul,
    Div,

    Greater,
    GreaterEq,
    Less,
    LessEq,
    Equals,
    NotEquals,

    ShiftLeft,
    ShiftRight,
}

#[derive(Clone, Debug)]
pub enum Literal {
    Int(u64),
    Float(f64),
    Bool(bool),
}

#[derive(Clone, Debug)]
pub enum AssignKind {
    Normal,
    BitOr,
    BitAnd,
    Xor,
    Add,
    Sub,
    Mul,
    Div,
}

pub struct AstDisplay<'a, 'b> {
    module: &'a Module,
    source: &'b str,
}

impl<'a, 'b> AstDisplay<'a, 'b> {
    fn write_tabs(&self, f: &mut Formatter, tabs: usize) -> Result {
        for _ in 0..tabs {
            write!(f, "\t")?;
        }

        Ok(())
    }

    fn write_block(&self, f: &mut Formatter, block: &Block, tabs: usize) -> Result {
        write!(f, "{{\n")?;

        for stmt in &block.stmts {
            match &stmt.kind {
                StmtKind::Let { name, expr } => {
                    self.write_tabs(f, tabs)?;
                    write!(f, "let {} = ", name.index_src(&self.source))?;
                    self.write_expr(f, expr)?;
                    write!(f, ";\n")?;
                }
                StmtKind::Assign { lval, kind, expr } => {
                    self.write_tabs(f, tabs)?;
                    self.write_expr(f, lval)?;

                    let assign = match kind {
                        AssignKind::Normal => "=",
                        AssignKind::BitOr => "|=",
                        AssignKind::BitAnd => "&=",
                        AssignKind::Xor => "^=",
                        AssignKind::Add => "+=",
                        AssignKind::Sub => "-=",
                        AssignKind::Mul => "*=",
                        AssignKind::Div => "/=",
                    };

                    write!(f, " {} ", assign)?;

                    self.write_expr(f, expr)?;

                    write!(f, ";\n")?;
                }
                StmtKind::If { cond, yes, no } => {
                    self.write_tabs(f, tabs)?;
                    write!(f, "if ")?;
                    self.write_expr(f, cond)?;
                    self.write_block(f, yes, tabs + 1)?;
                    if let Some(no) = no {
                        write!(f, " else ")?;
                        self.write_block(f, no, tabs + 1)?;
                    }
                    write!(f, "\n")?;
                }
                StmtKind::Expr(expr) => {
                    self.write_tabs(f, tabs)?;
                    self.write_expr(f, expr)?;
                    write!(f, ";\n")?;
                }
            }
        }

        self.write_tabs(f, tabs - 1)?;
        write!(f, "}}")
    }

    fn write_expr(&self, f: &mut Formatter, expr: &Expr) -> Result {
        match &expr.kind {
            ExprKind::Literal(literal) => match literal {
                Literal::Bool(val) => write!(f, "{val}")?,
                Literal::Int(val) => write!(f, "{val}")?,
                Literal::Float(val) => write!(f, "{val}")?,
            },
            ExprKind::Var(ident) => write!(f, "{}", ident.index_src(&self.source))?,
            ExprKind::Dot(left, ident) => {
                self.write_expr(f, left)?;
                write!(f, ".{}", ident.index_src(&self.source))?;
            }
            ExprKind::Index(left, index) => {
                self.write_expr(f, left)?;
                write!(f, "[")?;
                self.write_expr(f, index)?;
                write!(f, "]")?;
            }
            ExprKind::Unary(op, expr) => {
                match op {
                    UnOp::Negate => write!(f, "-")?,
                    UnOp::Not => write!(f, "!")?,
                }

                self.write_expr(f, expr)?;
            }
            ExprKind::Binary(left, op, right) => {
                write!(f, "(")?;
                self.write_expr(f, left)?;

                let op = match op {
                    BinOp::BitOr => "|",
                    BinOp::BitAnd => "&",
                    BinOp::Or => "||",
                    BinOp::And => "&&",
                    BinOp::BitXor => "^",

                    BinOp::Add => "+",
                    BinOp::Sub => "-",
                    BinOp::Mul => "*",
                    BinOp::Div => "/",

                    BinOp::Greater => ">",
                    BinOp::GreaterEq => ">=",
                    BinOp::Less => "<",
                    BinOp::LessEq => "<=",
                    BinOp::Equals => "==",
                    BinOp::NotEquals => "!=",

                    BinOp::ShiftLeft => "<<",
                    BinOp::ShiftRight => ">>",
                };
                write!(f, " {} ", op)?;

                self.write_expr(f, right)?;
                write!(f, ")")?;
            }
        }

        Ok(())
    }
}

impl<'a, 'b> Display for AstDisplay<'a, 'b> {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "devices {{\n\tin: [")?;
        let mut comma = false;
        for d in &self.module.devices.d_in {
            if comma {
                write!(f, ", ")?;
            }
            write!(f, "{}", &self.source[d.start.index..d.end.index])?;

            comma = true;
        }
        write!(
            f,
            "],\n\tout: {},\n}}\n\n",
            &self.source
                [self.module.devices.d_out.start.index..self.module.devices.d_out.end.index]
        )?;

        for event in &self.module.events {
            write!(
                f,
                "{}:{} ",
                event.source.index_src(&self.source),
                event.kind.index_src(&self.source)
            )?;

            self.write_block(f, &event.body, 1)?;

            write!(f, "\n\n")?;
        }

        Ok(())
    }
}
