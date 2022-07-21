use std::collections::HashMap;

use crate::{
    ast::{AssignKind, BinOp, Block, Expr, ExprKind, Literal, Module, Stmt, StmtKind, UnOp},
    span::Span,
    ty::{Field, IntWidth, Mutable, Signed, Type},
};

type Result<T> = std::result::Result<T, TypeError>;

pub struct TypeChecker<'a> {
    src: &'a str,
    globals: HashMap<&'a str, Type>,
    vars: HashMap<&'a str, Type>,
}

impl<'a> TypeChecker<'a> {
    pub fn new(src: &'a str, globals: HashMap<&'a str, Type>) -> Self {
        TypeChecker {
            src,
            globals,
            vars: HashMap::new(),
        }
    }

    pub fn check(mut self, module: &mut Module) -> Result<()> {
        for event in &mut module.events {
            self.vars = self.globals.clone();
            self.check_block(&mut event.body)?;
        }

        Ok(())
    }

    fn check_block(&mut self, body: &mut Block) -> Result<()> {
        for stmt in &mut body.stmts {
            self.check_stmt(stmt)?;
        }

        Ok(())
    }

    fn check_stmt(&mut self, stmt: &mut Stmt) -> Result<()> {
        let stmt_span = stmt.span;
        let stmt = &mut stmt.kind;
        let temp_stmt = std::mem::replace(stmt, StmtKind::Expr(Expr {
            kind: ExprKind::Literal(Literal::Bool(false)),
            span: stmt_span,
            ty: None,
        }));

        *stmt = match temp_stmt {
            stmt @ StmtKind::Assign { kind: AssignKind::Normal, .. } => stmt,
            StmtKind::Assign { lval, kind, expr } => {
                let op = match kind {
                    AssignKind::BitOr => BinOp::BitOr,
                    AssignKind::BitAnd => BinOp::BitAnd,
                    AssignKind::Xor => BinOp::BitXor,
                    AssignKind::Add => BinOp::Add,
                    AssignKind::Sub => BinOp::Sub,
                    AssignKind::Mul => BinOp::Mul,
                    AssignKind::Div => BinOp::Div,
                    AssignKind::Normal => unreachable!(),
                };
                let expr_span = expr.span;
                let expr = Expr {
                    kind: ExprKind::Binary(lval.clone().into(), op, expr.into()),
                    span: expr_span,
                    ty: None,
                };

                StmtKind::Assign { lval, kind: AssignKind::Normal, expr }
            }
            stmt => stmt,
        };

        match stmt {
            StmtKind::Let { name, expr } => {
                let name = name.index_src(&self.src);
                let ty = self.check_expr(expr)?.dereferenced();

                self.vars.insert(name, ty);
            }
            StmtKind::Assign { lval, kind, expr } => {
                let lty = self.check_expr(lval)?;
                if !matches!(lty, Type::Reference(_)) {
                    return Err(TypeError::NotLVal(lval.span));
                }

                let lty = lty.dereferenced();

                let rty = match kind {
                    AssignKind::Normal => self.check_expr(expr)?.dereferenced(),
                    _ => panic!("ICE: typechecker encountered non-normal AssignKind"),
                };

                if !lty.assignable_from(&rty) {
                    return Err(TypeError::NotAssignable {
                        left: lval.span,
                        left_ty: lty,
                        right: expr.span,
                        right_ty: rty,
                    });
                }
            }
            StmtKind::If { cond, yes, no } => {
                let condtype = self.check_expr(cond)?.dereferenced();
                if !matches!(condtype, Type::Bool) {
                    return Err(TypeError::TypeMismatch {
                        expected: Type::Bool,
                        got: condtype,
                        span: cond.span,
                    });
                }

                self.check_block(yes)?;
                if let Some(no) = no {
                    self.check_block(no)?;
                }
            }
            StmtKind::Expr(expr) => {
                self.check_expr(expr)?;
            }
        }

        Ok(())
    }

    fn check_expr(&mut self, expr: &mut Expr) -> Result<Type> {
        let ty = match &mut expr.kind {
            ExprKind::Literal(lit) => match lit {
                Literal::Int(i) if *i <= u8::MAX as u64 => Type::Int(IntWidth::W8, Signed::No),
                Literal::Int(i) if *i <= u16::MAX as u64 => Type::Int(IntWidth::W16, Signed::No),
                Literal::Int(i) if *i <= u32::MAX as u64 => Type::Int(IntWidth::W32, Signed::No),
                Literal::Int(_) => Type::Int(IntWidth::W64, Signed::No),
                Literal::Float(f) if *f <= f32::MAX as f64 => Type::F32,
                Literal::Float(_) => Type::F64,
                Literal::Bool(_) => Type::Bool,
            },
            ExprKind::Var(name) => {
                let name_str = name.index_src(&self.src);
                let ty = self
                    .vars
                    .get(name_str)
                    .ok_or(TypeError::InvalidVariable(*name))?;

                Type::Reference(ty.clone().into())
            }
            ExprKind::Dot(left, name_span) => {
                let left_ty = self.check_expr(left)?;
                let is_ref = matches!(left_ty, Type::Reference(_));
                let left_ty = left_ty.dereferenced();

                let name = name_span.index_src(&self.src);

                match &left_ty {
                    Type::Slice(_) => match name {
                        "len" => Type::Int(IntWidth::W32, Signed::No),
                        _ => {
                            return Err(TypeError::InvalidField {
                                ty: left_ty.clone(),
                                field: *name_span,
                            })
                        }
                    },
                    Type::Bitfield(_, names) => names
                        .names
                        .get(name)
                        .map(|_| {
                            if is_ref {
                                Type::Reference(Type::Bool.into())
                            } else {
                                Type::Bool
                            }
                        })
                        .ok_or(TypeError::InvalidField {
                            ty: left_ty.clone(),
                            field: *name_span,
                        })?,
                    Type::Struct(s) => s
                        .fields
                        .get(name)
                        .map(|field| Type::Reference(field.ty.clone().into()))
                        .ok_or(TypeError::InvalidField {
                            ty: left_ty.clone(),
                            field: *name_span,
                        })?,
                    _ => {
                        return Err(TypeError::InvalidField {
                            ty: left_ty.clone(),
                            field: *name_span,
                        })
                    }
                }
            }
            ExprKind::Index(left, idx) => {
                let left_ty = self.check_expr(left)?;
                let is_ref = matches!(left_ty, Type::Reference(_));
                let left_ty = left_ty.dereferenced();

                let idx_type = self.check_expr(idx)?.dereferenced();
                if !matches!(idx_type, Type::Int(_, Signed::No)) {
                    return Err(TypeError::NotAnIndex {
                        ty: idx_type,
                        expr: idx.span,
                    });
                }

                match left_ty {
                    Type::Int(_, _) | Type::Bitfield(_, _) => {
                        if is_ref {
                            Type::Reference(Type::Bool.into())
                        } else {
                            Type::Bool
                        }
                    }
                    Type::Slice(ty) => *ty,
                    other => {
                        return Err(TypeError::NotIndexable {
                            ty: other,
                            expr: left.span,
                        })
                    }
                }
            }
            ExprKind::Unary(op, expr) => {
                let expr_span = expr.span;
                let ty = self.check_expr(expr)?.dereferenced();

                match op {
                    UnOp::Negate => match ty {
                        Type::Int(_, Signed::Yes) | Type::F32 | Type::F64 => ty,
                        _ => {
                            return Err(TypeError::InvalidUnOp {
                                op: *op,
                                ty: ty.clone(),
                                expr: expr_span,
                            })
                        }
                    },
                    UnOp::Not => match ty {
                        Type::Bool => Type::Bool,
                        Type::Int(_, _) => ty,
                        Type::Bitfield(_, _) => ty,
                        ty => {
                            return Err(TypeError::InvalidUnOp {
                                op: *op,
                                ty: ty.clone(),
                                expr: expr_span,
                            })
                        }
                    },
                }
            }
            ExprKind::Binary(left, op, right) => {
                let expr_span = expr.span;
                let lty = self.check_expr(left)?.dereferenced();
                let rty = self.check_expr(right)?.dereferenced();

                match op {
                    BinOp::BitOr | BinOp::BitAnd | BinOp::BitXor => {
                        if let (Some(lw), Some(rw)) = (lty.is_bits(), rty.is_bits()) {
                            Type::Int(lw.max(rw), Signed::No)
                        } else {
                            return Err(TypeError::InvalidBinOp {
                                left: lty,
                                op: *op,
                                right: rty,
                                expr: expr_span,
                            });
                        }
                    }
                    BinOp::Or | BinOp::And => {
                        if matches!((&lty, &rty), (Type::Bool, Type::Bool)) {
                            Type::Bool
                        } else {
                            return Err(TypeError::InvalidBinOp {
                                left: lty,
                                op: *op,
                                right: rty,
                                expr: expr_span,
                            });
                        }
                    }
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => match (lty, rty) {
                        (Type::Int(w1, s1), Type::Int(w2, s2)) => {
                            if w1 >= w2 {
                                Type::Int(w1, s1)
                            } else {
                                Type::Int(w2, s2)
                            }
                        }
                        (Type::Int(w, _), Type::F32) | (Type::F32, Type::Int(w, _))
                            if w <= IntWidth::W32 =>
                        {
                            Type::F32
                        }
                        (Type::Int(_, _), Type::F64) | (Type::F64, Type::Int(_, _)) => Type::F64,
                        (Type::F32, Type::F32) => Type::F32,
                        (Type::F64, Type::F64) => Type::F64,
                        (Type::F32, Type::F64) | (Type::F64, Type::F32) => Type::F64,
                        (lty, rty) => {
                            return Err(TypeError::InvalidBinOp {
                                left: lty,
                                op: *op,
                                right: rty,
                                expr: expr_span,
                            })
                        }
                    },
                    BinOp::Greater | BinOp::GreaterEq | BinOp::Less | BinOp::LessEq => {
                        if lty.is_num() && rty.is_num() {
                            Type::Bool
                        } else {
                            return Err(TypeError::InvalidBinOp {
                                left: lty,
                                op: *op,
                                right: rty,
                                expr: expr_span,
                            });
                        }
                    }
                    BinOp::Equals | BinOp::NotEquals => {
                        if lty.is_num() && rty.is_num() {
                            Type::Bool
                        } else if lty == rty {
                            Type::Bool
                        } else {
                            return Err(TypeError::InvalidBinOp {
                                left: lty,
                                op: *op,
                                right: rty,
                                expr: expr_span,
                            });
                        }
                    }
                    BinOp::ShiftLeft | BinOp::ShiftRight => {
                        if matches!(&lty, Type::Int(_, _) | Type::Bitfield(_, _))
                            && matches!(&rty, Type::Int(_, _))
                        {
                            lty
                        } else {
                            return Err(TypeError::InvalidBinOp {
                                left: lty,
                                op: *op,
                                right: rty,
                                expr: expr_span,
                            });
                        }
                    }
                }
            }
        };

        expr.ty = Some(ty.clone());

        Ok(ty)
    }
}

pub enum TypeError {
    /// This expression cannot be stored in a variable
    NotStorable(Span),
    /// This expression cannot be assigned a value
    NotLVal(Span),
    /// Expression `right` cannot be assigned to expression `left`
    NotAssignable {
        left: Span,
        left_ty: Type,
        right: Span,
        right_ty: Type,
    },
    /// Type Mismatch
    TypeMismatch {
        expected: Type,
        got: Type,
        span: Span,
    },
    /// Variable does not exist
    InvalidVariable(Span),
    /// Field does not exist in type
    InvalidField { ty: Type, field: Span },
    /// This type cannot be indexed
    NotIndexable { ty: Type, expr: Span },
    /// This type cannot be used to index
    NotAnIndex { ty: Type, expr: Span },
    /// This unary operator is invalid on this type
    InvalidUnOp { op: UnOp, ty: Type, expr: Span },
    /// This binary operator is invalid on these types
    InvalidBinOp {
        left: Type,
        op: BinOp,
        right: Type,
        expr: Span,
    },
}
