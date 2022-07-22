use std::{collections::HashMap, cmp::Ordering};

use crate::{ir::{Module, Block, Body, Instruction, Width, Float, Cmp}, ast::{Module as AstModule, Block as AstBlock, DeviceIn, Stmt, StmtKind, Expr, ExprKind, Literal, UnOp, BinOp}, ty::{Type, Signed, IntWidth}};

const ICE_TYPE: &'static str = "ICE: ir_compiler: expression without type";
const ICE_BITS: &'static str = "ICE: ir_compiler: bit operation on non-bit type";

struct Env<'a> {
    vars: Vec<HashMap<&'a str, u32>>,
    sizes: HashMap<u32, usize>,
    next_id: u32,
    num_vars: u32,
}

impl<'a> Env<'a> {
    fn new() -> Self {
        Env {
            vars: vec![HashMap::new()],
            sizes: HashMap::new(),
            next_id: 0,
            num_vars: 0,
        }
    }

    fn new_stack(&mut self) {
        self.sizes = HashMap::new();
        self.num_vars = 0;
    }

    fn push(&mut self) {
        self.vars.push(HashMap::new());
    }

    fn pop(&mut self) {
        let map = self.vars.pop()
            .expect("ICE: ir_compiler: too many environments popped");
        
        self.next_id = map.values().copied().min().unwrap_or(self.next_id);
    }

    fn insert(&mut self, key: &'a str, size: usize) -> u32 {
        let vars = self.vars.last_mut().expect("ICE: ir_compiler: null environment insert");

        if let Some(id) = vars.get(key) {
            let old_size = self.sizes.get_mut(id)
                .expect("ICE: ir_compiler: variable without size");

            *old_size = usize::max(*old_size, size);

            *id
        } else {
            let id = self.next_id;
            self.next_id += 1;
            if self.next_id == 0 {
                panic!("ICE: ir_compiler: variable overflow");
            }

            if self.next_id > self.num_vars {
                self.num_vars = self.next_id;
            }

            vars.insert(key, id);
            self.sizes.insert(id, size);

            id
        }
    }

    fn get(&self, key: &'a str) -> u32 {
        for vars in self.vars.iter().rev() {
            if let Some(id) = vars.get(key) {
                return *id;
            }
        }

        panic!("ICE: ir_compiler: variable does not exist")
    }
}

pub struct IrCompiler<'a> {
    src: &'a str,
    instrs: Vec<Instruction>,

    env: Env<'a>,
}

impl<'a> IrCompiler<'a> {
    pub fn new(src: &'a str) -> Self {
        IrCompiler {
            src,
            instrs: Vec::new(),

            env: Env::new(),
        }
    }

    pub fn compile(mut self, module: AstModule) -> Module {
        // TODO: Device variables
        let inputs = module.inputs
            .into_iter()
            .map(|input| self.compile_input(input))
            .collect();
        
        Module { inputs }
    }

    fn compile_input(&mut self, input: DeviceIn) -> Body {
        self.env.new_stack();

        let block = self.compile_block(input.body);

        Body {
            block,
            num_vars: self.env.num_vars,
            var_sizes: std::mem::take(&mut self.env.sizes),
        }
    }

    fn compile_block(&mut self, block: AstBlock) -> Block {
        self.env.push();

        for stmt in block.stmts {
            self.compile_stmt(stmt);
        }

        let instrs = std::mem::take(&mut self.instrs);

        self.env.pop();

        Block(instrs)
    }

    fn compile_stmt(&mut self, stmt: Stmt) {
        match stmt.kind {
            StmtKind::Let { name, expr } => {
                let key = name.index_src(self.src);
                let expr_ty = expr.ty.expect(ICE_TYPE);
                let size = expr_ty.stack_size();

                self.compile_expr(expr.kind, expr_ty);

                let var_id = self.env.insert(key, size);

                self.instrs.push(Instruction::VarPut(size, var_id));
            }
            StmtKind::Assign { lval, expr, .. } => {
                let lval_ty = lval.ty.expect(ICE_TYPE);
                let expr_ty = expr.ty.expect(ICE_TYPE);
                let size = lval_ty.stack_size();

                self.compile_expr(expr.kind, expr_ty.clone());
                self.compile_assign_convert(expr_ty, lval_ty.clone());
                self.compile_store(lval.kind, lval_ty, size);
            }
            StmtKind::If { cond, yes, no } => {
                let yes = self.compile_block(yes);
                let no = no.map(|no| self.compile_block(no))
                    .unwrap_or(Block(Vec::new()));
                
                self.compile_expr(cond.kind, cond.ty.expect(ICE_TYPE));
                
                self.instrs.push(Instruction::If { yes, no });
            }
            StmtKind::Expr(expr) => {
                let expr_ty = expr.ty.expect(ICE_TYPE);
                let size = expr_ty.stack_size();

                self.compile_expr(expr.kind, expr_ty);

                self.instrs.push(Instruction::Pop(size));
            }
        }
    }

    fn compile_expr(&mut self, expr: ExprKind, ty: Type) {
        let size = ty.stack_size();

        match expr {
            ExprKind::Literal(lit) => match lit {
                Literal::Bool(val) => {
                    self.instrs.push(Instruction::Push8(val as u8));
                }
                Literal::Int(i) => {
                    self.instrs.push(Instruction::Push64(i));
                }
                Literal::Float(f) => {
                    self.instrs.push(Instruction::Push64(unsafe { std::mem::transmute(f) }));
                }
            }
            ExprKind::Var(name) => {
                let key = name.index_src(self.src);
                let id = self.env.get(key);

                self.instrs.push(Instruction::VarGet(size, id));
            }
            ExprKind::Dot(left, ident) => {
                let ty = left.ty.expect(ICE_TYPE);
                let left = left.kind;
                let ident = ident.index_src(self.src);

                match &ty {
                    // ident == "len"
                    Type::Slice(_) => {
                        self.compile_expr(left, ty);
                        self.instrs.push(Instruction::Swap(std::mem::size_of::<usize>()));
                        self.instrs.push(Instruction::Pop(std::mem::size_of::<usize>()));
                    }
                    Type::Bitfield(_, w, names) => {
                        let w = *w;
                        let bit = *names.0.get(ident)
                            .expect("ICE: ir_compiler: invalid bitfield field");
                        
                        self.compile_expr(left, ty);
                        self.instrs.push(Instruction::Push8(bit));
                        self.instrs.push(Instruction::ShiftRight(w.into()));
                        self.instrs.push(match w {
                            IntWidth::W8 => Instruction::Push8(1),
                            IntWidth::W16 => Instruction::Push16(1),
                            IntWidth::W32 => Instruction::Push32(1),
                            IntWidth::W64 => Instruction::Push64(1),
                        });
                        self.instrs.push(Instruction::And(w.into()));
                    }
                    Type::Struct(s) => {
                        let field = s.fields.get(ident)
                            .expect("ICE: ir_compiler: invalid struct field");
                        let pointer_size = ty.stack_size();
                        let field_offset = field.byte_offset;
                        let field_size = field.ty.stack_size();
                        self.compile_expr(left, ty);
                        match pointer_size {
                            4 => {
                                self.instrs.push(Instruction::Push32(field_offset as _));
                                self.instrs.push(Instruction::Add(Width::W32));
                            }
                            8 => {
                                self.instrs.push(Instruction::Push64(field_offset as _));
                                self.instrs.push(Instruction::Add(Width::W64));
                            }
                            _ => panic!("ICE: ir_compiler: invalid pointer width"),
                        }
                        self.instrs.push(Instruction::Load(field_size));
                    }
                    _ => panic!("ICE: ir_compiler: invalid field access"),
                }
            }
            ExprKind::Index(left, index) => {
                let lty = left.ty.expect(ICE_TYPE);
                let left = left.kind;

                let rty = index.ty.expect(ICE_TYPE);
                let right = index.kind;

                match &lty {
                    Type::Int(lw, _) | Type::Bitfield(_, lw, _) => {
                        let Type::Int(rw, _) = &rty
                        else { panic!("ICE: ir_compiler: invalid index type"); };

                        let (lw, rw) = (*lw, *rw);

                        self.compile_expr(left, lty);
                        self.compile_expr(right, rty);
                        if rw > IntWidth::W8 {
                            self.instrs.push(Instruction::Shorten { from: rw.into(), to: Width::W8 });
                        }

                        self.instrs.push(Instruction::ShiftRight(lw.into()));
                        self.instrs.push(match lw {
                            IntWidth::W8 => Instruction::Push8(1),
                            IntWidth::W16 => Instruction::Push16(1),
                            IntWidth::W32 => Instruction::Push32(1),
                            IntWidth::W64 => Instruction::Push64(1),
                        });
                        self.instrs.push(Instruction::And(lw.into()));
                    }
                    Type::Slice(sty) => {
                        let size = sty.stack_size();
                        let Type::Int(rw, _) = &rty
                        else { panic!("ICE: ir_compiler: invalid index type"); };
                        let rw = *rw;

                        self.compile_expr(left, lty);
                        self.compile_expr(right, rty);
                        match std::mem::size_of::<usize>() {
                            4 => {
                                if rw < IntWidth::W32 {
                                    self.instrs.push(Instruction::Extend { from: rw.into(), to: Width::W32, signed: false });
                                } else if rw > IntWidth::W32 {
                                    self.instrs.push(Instruction::Shorten { from: rw.into(), to: Width::W32 });
                                }

                                self.instrs.push(Instruction::Push32(size as _));
                                self.instrs.push(Instruction::Mul { width: Width::W32, signed: false });
                            }
                            8 => {
                                if rw < IntWidth::W64 {
                                    self.instrs.push(Instruction::Extend { from: rw.into(), to: Width::W64, signed: false });
                                }

                                self.instrs.push(Instruction::Push64(size as _));
                                self.instrs.push(Instruction::Mul { width: Width::W64, signed: false });
                            }
                            _ => panic!("ICE: ir_compiler: invalid pointer width"),
                        }

                        self.instrs.push(Instruction::Load(size));
                    }
                    _ => panic!("ICE: ir_compiler: invalid type indexed"),
                }
            }
            ExprKind::Unary(op, expr) => match op {
                UnOp::Negate => match ty {
                    Type::Int(w, _) => {
                        self.instrs.push(Instruction::Neg(w.into()));
                    }
                    Type::F32 => {
                        self.instrs.push(Instruction::FloatNeg(Float::F32));
                    }
                    Type::F64 => {
                        self.instrs.push(Instruction::FloatNeg(Float::F64));
                    }
                    _ => panic!("ICE: ir_compiler: invalid unary op '-'"),
                }
                UnOp::Not => match ty {
                    Type::Bool => {
                        self.instrs.push(Instruction::BoolNot);
                    }
                    Type::Int(w, _) | Type::Bitfield(_, w, _) => {
                        self.instrs.push(Instruction::Not(w.into()));
                    }
                    _ => panic!("ICE: ir_compiler: invalid unary op '!'"),
                },
            }
            ExprKind::Binary(left, op, right) => {
                let (left, right) = (*left, *right);
                let lty = left.ty.expect(ICE_TYPE);
                let rty = right.ty.expect(ICE_TYPE);

                match op {
                    BinOp::BitOr | BinOp::BitAnd | BinOp::BitXor => {
                        let lw = lty.width().expect(ICE_BITS);
                        let rw = rty.width().expect(ICE_BITS);

                        match lw.cmp(&rw) {
                            Ordering::Greater => {
                                self.compile_expr(right.kind, rty);
                                self.instrs.push(Instruction::Extend { from: rw.into(), to: lw.into(), signed: false });
                                self.compile_expr(left.kind, lty);
                            }
                            Ordering::Less => {
                                self.compile_expr(right.kind, rty);
                                self.compile_expr(left.kind, lty);
                                self.instrs.push(Instruction::Extend { from: lw.into(), to: rw.into(), signed: false });
                            }
                            Ordering::Equal => {
                                self.compile_expr(right.kind, rty);
                                self.compile_expr(left.kind, lty);
                            }
                        }

                        let bigger = lw.max(rw).into();

                        match op {
                            BinOp::BitOr => self.instrs.push(Instruction::Or(bigger)),
                            BinOp::BitAnd => self.instrs.push(Instruction::And(bigger)),
                            BinOp::BitXor => self.instrs.push(Instruction::Xor(bigger)),
                            _ => unreachable!(),
                        }
                    }
                    BinOp::Or | BinOp::And => {
                        self.compile_expr(right.kind, rty);
                        self.compile_expr(left.kind, lty);

                        match op {
                            BinOp::Or => self.instrs.push(Instruction::Or(Width::W8)),
                            BinOp::And => self.instrs.push(Instruction::And(Width::W8)),
                            _ => unreachable!(),
                        }
                    }
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                        let ifn = match op {
                            BinOp::Add => |w, s| Instruction::Add(w),
                            BinOp::Sub => |w, s| Instruction::Sub(w),
                            BinOp::Mul => |w, s| Instruction::Mul { width: w, signed: s },
                            BinOp::Div => |w, s| Instruction::Div { width: w, signed: s },
                            _ => unreachable!(),
                        };

                        let ffn = match op {
                            BinOp::Add => Instruction::FloatAdd,
                            BinOp::Sub => Instruction::FloatSub,
                            BinOp::Mul => Instruction::FloatMul,
                            BinOp::Div => Instruction::FloatDiv,
                            _ => unreachable!(),
                        };

                        let instr = match &lty {
                            Type::Int(w, s) => ifn((*w).into(), s == &Signed::Yes),
                            Type::F32 => ffn(Float::F32),
                            Type::F64 => ffn(Float::F64),
                            _ => panic!("ICE: ir_compiler: invalid arithmetic type"),
                        };

                        self.compile_expr(right.kind, rty);
                        self.compile_expr(left.kind, lty);

                        self.instrs.push(instr);
                    }
                    BinOp::Greater | BinOp::GreaterEq | BinOp::Less | BinOp::LessEq => {
                        let cmp = match op {
                            BinOp::Greater => Cmp::Greater,
                            BinOp::GreaterEq => Cmp::GreaterEq,
                            BinOp::Less => Cmp::Less,
                            BinOp::LessEq => Cmp::LessEq,
                            _ => unreachable!(),
                        };

                        let instr = match &lty {
                            Type::Int(w, s) => {
                                Instruction::IntCompare { width: (*w).into(), cmp, signed: s == &Signed::Yes }
                            }
                            Type::F32 => {
                                Instruction::FloatCompare(Float::F32, cmp)
                            }
                            Type::F64 => {
                                Instruction::FloatCompare(Float::F64, cmp)
                            }
                            _ => panic!("ICE: ir_compiler: invalid compare type"),
                        };

                        self.compile_expr(right.kind, rty);
                        self.compile_expr(left.kind, lty);

                        self.instrs.push(instr);
                    }
                    BinOp::Equals | BinOp::NotEquals => {
                        let cmp = match op {
                            BinOp::Equals => Cmp::Eq,
                            BinOp::NotEquals => Cmp::Neq,
                            _ => unreachable!(),
                        };

                        let instr = match lty {
                            Type::Int(w, _) => Instruction::IntCompare { width: w.into(), cmp, signed: false },
                            Type::Bool => Instruction::IntCompare { width: Width::W8, cmp, signed: false },
                            Type::Bitfield(_, w, _) => Instruction::IntCompare { width: w.into(), cmp, signed: false },
                            Type::F32 => Instruction::FloatCompare(Float::F32, cmp),
                            Type::F64 => Instruction::FloatCompare(Float::F64, cmp),
                            _ => panic!("ICE: ir_compiler: invalid equals type"),
                        };

                        self.compile_expr(right.kind, rty);
                        self.compile_expr(left.kind, lty);

                        self.instrs.push(instr);
                    }
                    BinOp::ShiftLeft | BinOp::ShiftRight => {
                        let lw = lty.width().expect("ICE: ir_compiler: shift non-integer");
                        self.compile_expr(left.kind, lty);

                        let rw = rty.width().expect("ICE: ir_compiler: shift by non-integer");
                        self.compile_expr(right.kind, rty);

                        if rw.size() != 1 {
                            self.instrs.push(Instruction::Shorten { from: rw.into(), to: Width::W8 });
                        }
                        
                        match op {
                            BinOp::ShiftLeft => self.instrs.push(Instruction::ShiftLeft(lw.into())),
                            BinOp::ShiftRight => self.instrs.push(Instruction::ShiftRight(lw.into())),
                            _ => unreachable!(),
                        }
                    }
                }
            }
        }
    }

    fn compile_store(&mut self, expr: ExprKind, ty: Type, size: usize) {
        todo!()
    }

    fn compile_assign_convert(&mut self, from: Type, to: Type) {
        todo!()
    }
}