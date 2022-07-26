use std::collections::HashMap;

use crate::{
    ast::{AssignKind, BinOp, Block, Expr, ExprKind, Literal, Module, Stmt, StmtKind, UnOp},
    span::Span,
    ty::{Type, RefData},
    util::{Signed, Width},
};

type Result<T> = std::result::Result<T, TypeError>;

struct Env<'a> {
    vars: Vec<HashMap<&'a str, Type>>,
}

impl<'a> Env<'a> {
    fn new() -> Self {
        Env {
            vars: vec![HashMap::new()],
        }
    }

    fn push(&mut self) {
        self.vars.push(HashMap::new());
    }

    fn pop(&mut self) {
        self.vars.pop();
    }

    fn get(&self, key: &'_ str) -> Option<&Type> {
        for vars in self.vars.iter().rev() {
            if let Some(ty) = vars.get(key) {
                return Some(ty);
            }
        }

        None
    }

    fn insert(&mut self, key: &'a str, ty: Type) {
        self.vars
            .last_mut()
            .expect("ICE: null environment insert")
            .insert(key, ty);
    }
}

pub struct TypeChecker<'a> {
    src: &'a str,
    env: Env<'a>,

    errors: Vec<TypeError>,
}

impl<'a> TypeChecker<'a> {
    pub fn new(src: &'a str) -> Self {
        TypeChecker {
            src,
            env: Env::new(),

            errors: Vec::new(),
        }
    }

    pub fn check(
        mut self,
        module: &mut Module,
        globals: HashMap<&'a str, Type>,
    ) -> std::result::Result<(), Vec<TypeError>> {
        for (key, ty) in globals {
            self.env.insert(key, ty);
        }

        let mut input_names = HashMap::new();

        for input in &mut module.inputs {
            let name = input.device.index_src(self.src);
            if let Some(old) = input_names.get(name) {
                self.errors.push(TypeError::DeviceAlreadyExists {
                    old: *old,
                    new: input.device,
                });
            } else {
                input_names.insert(name, input.device);
            }

            self.check_block(&mut input.body);
        }

        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors)
        }
    }

    fn check_block(&mut self, body: &mut Block) {
        self.env.push();

        for stmt in &mut body.stmts {
            match self.check_stmt(stmt) {
                Ok(()) => {}
                Err(e) => {
                    self.errors.push(e);
                    break;
                }
            }
        }

        self.env.pop();
    }

    fn check_stmt(&mut self, stmt: &mut Stmt) -> Result<()> {
        let stmt_span = stmt.span;
        let stmt = &mut stmt.kind;
        let temp_stmt = std::mem::replace(
            stmt,
            StmtKind::Expr(Expr {
                kind: ExprKind::Literal(Literal::Bool(false)),
                span: stmt_span,
                ty: None,
            }),
        );

        *stmt = match temp_stmt {
            stmt @ StmtKind::Assign {
                kind: AssignKind::Normal,
                ..
            } => stmt,
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

                StmtKind::Assign {
                    lval,
                    kind: AssignKind::Normal,
                    expr,
                }
            }
            stmt => stmt,
        };

        match stmt {
            StmtKind::Let { name, expr } => {
                let name = name.index_src(&self.src);
                let ty = self.check_expr(expr)?.dereferenced();

                self.env.insert(name, ty.clone());
            }
            StmtKind::Assign { lval, kind, expr } => {
                let lty = match self.check_expr(lval) {
                    Ok(t) => t,
                    Err(e) => {
                        self.errors.push(e);
                        return Ok(());
                    }
                };

                if !matches!(lty, Type::Reference(_, _)) {
                    self.errors.push(TypeError::NotLVal(lval.span));
                    return Ok(());
                }

                let lty = lty.dereferenced();

                let rty = match kind {
                    AssignKind::Normal => self.check_expr(expr)?.dereferenced(),
                    _ => panic!("ICE: typechecker encountered non-normal AssignKind"),
                };

                if !lty.assignable_from(&rty) {
                    self.errors.push(TypeError::NotAssignable {
                        left: lval.span,
                        left_ty: lty,
                        right: expr.span,
                        right_ty: rty,
                    });
                    return Ok(());
                }
            }
            StmtKind::If { cond, yes, no } => {
                let condtype = match self.check_expr(cond) {
                    Ok(e) => e,
                    Err(e) => {
                        self.errors.push(e);
                        Type::Bool
                    }
                }
                .dereferenced();
                if !matches!(condtype, Type::Bool) {
                    self.errors.push(TypeError::TypeMismatch {
                        expected: Type::Bool,
                        got: condtype,
                        span: cond.span,
                    });
                }

                self.check_block(yes);
                if let Some(no) = no {
                    self.check_block(no);
                }
            }
            StmtKind::Expr(expr) => match self.check_expr(expr) {
                Ok(_) => {}
                Err(e) => {
                    self.errors.push(e);
                }
            },
        }

        Ok(())
    }

    fn check_expr(&mut self, expr: &mut Expr) -> Result<Type> {
        let ty = match &mut expr.kind {
            ExprKind::Literal(lit) => match lit {
                Literal::Int(int, signed) => Type::Int(int.width(), *signed),
                Literal::Float(f) if *f <= f32::MAX as f64 => Type::F32,
                Literal::Float(_) => Type::F64,
                Literal::Bool(_) => Type::Bool,
            },
            ExprKind::Var(name) => {
                let name_str = name.index_src(&self.src);
                let ty = self
                    .env
                    .get(name_str)
                    .ok_or(TypeError::InvalidVariable(*name))?;

                Type::Reference(ty.clone().into(), RefData(()))
            }
            ExprKind::Dot(left, name_span) => {
                let left_ty = self.check_expr(left)?;
                let is_ref = matches!(left_ty, Type::Reference(_, _));
                let left_ty = left_ty.dereferenced();

                let name = name_span.index_src(&self.src);

                match &left_ty {
                    Type::Slice(_) => match name {
                        "len" => Type::Int(Width::W32, Signed::No),
                        _ => {
                            return Err(TypeError::InvalidField {
                                ty: left_ty.clone(),
                                field: *name_span,
                            })
                        }
                    },
                    Type::Bitfield(_, _, names) => names
                        .0
                        .get(name)
                        .map(|_| {
                            if is_ref {
                                Type::Reference(Type::Bool.into(), RefData(()))
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
                        .map(|field| Type::Reference(field.ty.clone().into(), RefData(())))
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
                let is_ref = matches!(left_ty, Type::Reference(_, _));
                let left_ty = left_ty.dereferenced();

                let idx_type = self.check_expr(idx)?.dereferenced();
                if !matches!(idx_type, Type::Int(_, Signed::No)) {
                    return Err(TypeError::NotAnIndex {
                        ty: idx_type,
                        expr: idx.span,
                    });
                }

                match left_ty {
                    Type::Int(_, _) | Type::Bitfield(_, _, _) => {
                        if is_ref {
                            Type::Reference(Type::Bool.into(), RefData(()))
                        } else {
                            Type::Bool
                        }
                    }
                    Type::Slice(ty) => Type::Reference(ty, RefData(())),
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
                        Type::Bitfield(_, _, _) => ty,
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
                        if lty.width().is_some() && lty.width() == rty.width() {
                            Type::Int(lty.width().unwrap(), Signed::No)
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
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                        if lty.is_num() && rty.is_num() && lty == rty {
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
                    BinOp::Greater | BinOp::GreaterEq | BinOp::Less | BinOp::LessEq => {
                        if lty.is_num() && rty.is_num() && lty == rty {
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
                        if lty == rty && lty.width().is_some() {
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
                        if matches!(&lty, Type::Int(_, _) | Type::Bitfield(_, _, _))
                            && matches!(&rty, Type::Int(_, Signed::No))
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
            ExprKind::Cast(expr, ty, tymeta) => {
                todo!();
            }
        };

        expr.ty = Some(ty.clone().dereferenced());

        Ok(ty)
    }
}

#[derive(Clone, Debug)]
pub enum TypeError {
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
    /// Device already exists
    DeviceAlreadyExists { old: Span, new: Span },
}
