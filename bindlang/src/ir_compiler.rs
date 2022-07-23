use std::collections::HashMap;

use crate::{ir::{Module, Block, Body, Instruction, Float, Cmp}, ast::{Module as AstModule, Block as AstBlock, DeviceIn, Stmt, StmtKind, ExprKind, Literal, UnOp, BinOp}, ty::Type, util::{Width, Signed, Int}};

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
        self.sizes.clear();
        self.vars.first_mut()
            .expect("ICE: ir_compiler: too many environments popped")
            .clear();
        self.num_vars = 0;
        self.next_id = 0;
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
        // Device variables

        let out_name = module.output.index_src(self.src);
        let in_names: Vec<_> = module.inputs
            .iter()
            .map(|input| input.device.index_src(self.src))
            .collect();

        let inputs = module.inputs
            .into_iter()
            .map(|input| self.compile_input(input, out_name, &in_names))
            .collect();
        
        Module { inputs }
    }

    fn compile_input(&mut self, input: DeviceIn, out_name: &'a str, in_names: &[&'a str]) -> Body {
        self.env.new_stack();

        self.env.insert(out_name, 8);
        for in_name in in_names {
            self.env.insert(in_name, 8);
        }

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

                self.compile_expr(expr.kind, expr_ty.clone());
                self.compile_assign_convert(expr_ty.clone(), lval_ty.clone());
                self.compile_store(lval.kind, lval_ty);
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
        let ty = ty.dereferenced();
        let size = ty.stack_size();

        match expr {
            ExprKind::Literal(lit) => match lit {
                Literal::Bool(val) => {
                    self.instrs.push(Instruction::PushInt(val.into()));
                }
                Literal::Int(int, _) => {
                    self.instrs.push(Instruction::PushInt(int));
                }
                Literal::Float(f) => {
                    let bits: u64 = unsafe { std::mem::transmute(f) };
                    self.instrs.push(Instruction::PushInt(bits.into()));
                }
            }
            ExprKind::Var(name) => {
                let key = name.index_src(self.src);
                let id = self.env.get(key);

                self.instrs.push(Instruction::VarGet(size, id));
            }
            ExprKind::Dot(left, ident) => {
                let ty = left.ty.expect(ICE_TYPE).dereferenced();
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
                        self.instrs.push(Instruction::PushInt(bit.into()));
                        self.instrs.push(Instruction::ShiftRight(w));
                        self.instrs.push(Instruction::PushInt(w.int(1)));
                        self.instrs.push(Instruction::And(w));
                    }
                    Type::Struct(s) => {
                        let field = s.fields.get(ident)
                            .expect("ICE: ir_compiler: invalid struct field");
                        let field_offset = field.byte_offset;
                        let field_size = field.ty.stack_size();
                        self.compile_expr(left, ty);
                        let field_offset_int = Int::WSize(field_offset);

                        self.instrs.push(Instruction::PushInt(field_offset_int));
                        self.instrs.push(Instruction::Add(Width::WSize));

                        self.instrs.push(Instruction::Load(field_size));
                    }
                    _ => panic!("ICE: ir_compiler: invalid field access"),
                }
            }
            ExprKind::Index(left, index) => {
                let lty = left.ty.expect(ICE_TYPE).dereferenced();
                let left = left.kind;

                let rty = index.ty.expect(ICE_TYPE).dereferenced();
                let right = index.kind;

                match &lty {
                    Type::Int(lw, _) | Type::Bitfield(_, lw, _) => {
                        let Type::Int(rw, _) = &rty
                        else { panic!("ICE: ir_compiler: invalid index type"); };

                        let (lw, rw) = (*lw, *rw);

                        self.compile_expr(left, lty);
                        self.compile_expr(right, rty);
                        if rw > Width::W8 {
                            self.instrs.push(Instruction::Shorten { from: rw, to: Width::W8 });
                        }

                        self.instrs.push(Instruction::ShiftRight(lw));
                        self.instrs.push(Instruction::PushInt(lw.int(1)));
                        self.instrs.push(Instruction::And(lw));
                    }
                    Type::Slice(sty) => {
                        let size = sty.stack_size();
                        let Type::Int(rw, _) = &rty
                        else { panic!("ICE: ir_compiler: invalid index type"); };
                        let rw = *rw;

                        // Stack: [pointer, length]
                        self.compile_expr(left, lty);
                        // Stack: [pointer, length, index]
                        self.compile_expr(right.clone(), rty.clone());

                        // sizeof(index) = sizeof(length)
                        if rw < Width::WSize {
                            self.instrs.push(Instruction::Extend { from: rw, to: Width::WSize, signed: false });
                        } else if rw > Width::WSize {
                            self.instrs.push(Instruction::Shorten { from: rw, to: Width::WSize });
                        }

                        // Stack: [pointer, index >= length]
                        self.instrs.push(Instruction::IntCompare { width: Width::WSize, cmp: Cmp::GreaterEq, signed: false });

                        // Stack: [pointer]
                        self.instrs.push(Instruction::If {
                            yes: Block(vec![Instruction::Error(1)]),
                            no: Block(Vec::new()),
                        });

                        // Stack: [pointer, index]
                        self.compile_expr(right, rty);

                        if rw < Width::WSize {
                            self.instrs.push(Instruction::Extend { from: rw, to: Width::WSize, signed: false });
                        } else if rw > Width::WSize {
                            self.instrs.push(Instruction::Shorten { from: rw, to: Width::WSize });
                        }

                        self.instrs.push(Instruction::PushInt(Int::WSize(size)));
                        self.instrs.push(Instruction::Mul { width: Width::WSize, signed: false });
                        self.instrs.push(Instruction::Add(Width::WSize));

                        self.instrs.push(Instruction::Load(size));
                    }
                    _ => panic!("ICE: ir_compiler: invalid type indexed"),
                }
            }
            ExprKind::Unary(op, expr) => {
                self.compile_expr(expr.kind, expr.ty.expect(ICE_TYPE));

                match op {
                    UnOp::Negate => match ty {
                        Type::Int(w, _) => {
                            self.instrs.push(Instruction::Neg(w));
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
                            self.instrs.push(Instruction::Not(w));
                        }
                        _ => panic!("ICE: ir_compiler: invalid unary op '!'"),
                    },
                }
            }
            ExprKind::Binary(left, op, right) => {
                let (left, right) = (*left, *right);
                let lty = left.ty.expect(ICE_TYPE).dereferenced();
                let rty = right.ty.expect(ICE_TYPE).dereferenced();

                match op {
                    BinOp::BitOr | BinOp::BitAnd | BinOp::BitXor => {
                        let width = lty.width().expect(ICE_BITS);

                        self.compile_expr(right.kind, rty);
                        self.compile_expr(left.kind, lty);

                        match op {
                            BinOp::BitOr => self.instrs.push(Instruction::Or(width)),
                            BinOp::BitAnd => self.instrs.push(Instruction::And(width)),
                            BinOp::BitXor => self.instrs.push(Instruction::Xor(width)),
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
                            BinOp::Add => |w, _| Instruction::Add(w),
                            BinOp::Sub => |w, _| Instruction::Sub(w),
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
                            Type::Int(w, s) => ifn(*w, s == &Signed::Yes),
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
                                Instruction::IntCompare { width: *w, cmp, signed: s == &Signed::Yes }
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
                            Type::Int(w, _) => Instruction::IntCompare { width: w, cmp, signed: false },
                            Type::Bool => Instruction::IntCompare { width: Width::W8, cmp, signed: false },
                            Type::Bitfield(_, w, _) => Instruction::IntCompare { width: w, cmp, signed: false },
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
                            self.instrs.push(Instruction::Shorten { from: rw, to: Width::W8 });
                        }
                        
                        match op {
                            BinOp::ShiftLeft => self.instrs.push(Instruction::ShiftLeft(lw)),
                            BinOp::ShiftRight => self.instrs.push(Instruction::ShiftRight(lw)),
                            _ => unreachable!(),
                        }
                    }
                }
            }
        }
    }

    fn compile_store(&mut self, expr: ExprKind, ty: Type) {
        let ty = ty.dereferenced();

        match expr {
            ExprKind::Var(name) => {
                let name = name.index_src(self.src);
                let var_index = self.env.get(name);

                self.instrs.push(Instruction::VarPut(ty.stack_size(), var_index));
            }
            ExprKind::Dot(left, field) => {
                let lty = left.ty.expect(ICE_TYPE).dereferenced();
                let left = left.kind;
                match &lty {
                    Type::Bitfield(_, w, names) => {
                        assert!(ty == Type::Bool, "ICE: ir_compiler: tried to store non-bool field in bitfield");
                        
                        let w = *w;
                        let bit = *names.0.get(field.index_src(self.src))
                            .expect("ICE: ir_compiler: invalid bitfield field");
                        
                        if w > Width::W8 {
                            self.instrs.push(Instruction::Extend { from: Width::W8, to: w, signed: false });
                        }
                        self.instrs.push(Instruction::PushInt(bit.into()));
                        self.instrs.push(Instruction::ShiftLeft(w));

                        match w {
                            Width::W8 => self.instrs.push(Instruction::PushInt((!(1 << bit)).into())),
                            Width::W16 => self.instrs.push(Instruction::PushInt((!(1 << bit as u16)).into())),
                            Width::W32 => self.instrs.push(Instruction::PushInt((!(1 << bit as u32)).into())),
                            Width::W64 => self.instrs.push(Instruction::PushInt((!(1 << bit as u64)).into())),
                        }

                        self.compile_expr(left.clone(), lty.clone());

                        self.instrs.push(Instruction::And(w));
                        self.instrs.push(Instruction::Or(w));

                        self.compile_store(left, lty);
                    }
                    Type::Struct(s) => {
                        let field = s.fields.get(field.index_src(self.src))
                            .expect("ICE: ir_compiler: invalid struct field");
                        
                        let offset = field.byte_offset;
                        let size = field.ty.stack_size();
                        
                        if size != ty.stack_size() {
                            panic!("ICE: ir_compiler: tried to store wrong field in struct");
                        }

                        self.compile_expr(left, lty);

                        self.instrs.push(Instruction::PushInt(offset.into()));
                        self.instrs.push(Instruction::Add(Width::WSize));

                        self.instrs.push(Instruction::Store(size));
                    }
                    ty => panic!("ICE: ir_compiler: store in invalid type '{ty:?}'"),
                }
            }
            ExprKind::Index(left, right) => {
                let lty = left.ty.expect(ICE_TYPE).dereferenced();
                let left = left.kind;
                let rty = right.ty.expect(ICE_TYPE).dereferenced();
                let right = right.kind;

                match &lty {
                    Type::Int(lw, _) | Type::Bitfield(_, lw, _) => {
                        assert!(ty == Type::Bool, "ICE: ir_compiler: tried to set bit to non-bool");

                        let Type::Int(rw, _) = &rty
                        else { panic!("ICE: ir_compiler: invalid index type"); };

                        let (lw, rw) = (*lw, *rw);

                        // Widen bool on stack
                        if lw > Width::W8 {
                            self.instrs.push(Instruction::Extend { from: Width::W8, to: lw, signed: false });
                        }
                        // Push bit on to stack
                        self.compile_expr(right.clone(), rty.clone());
                        // Shorten it for the shift operator
                        if rw > Width::W8 {
                            self.instrs.push(Instruction::Shorten { from: rw, to: Width::W8 });
                        }
                        // Shift bool
                        self.instrs.push(Instruction::ShiftRight(lw));

                        // Stack now contains OR argument

                        // Now construct inverted bit mask

                        // Push 1 on to stack
                        self.instrs.push(Instruction::PushInt(lw.int(1)));

                        // Push bit on to stack
                        self.compile_expr(right.clone(), rty.clone());
                        // Shorten it for the shift operator
                        if rw > Width::W8 {
                            self.instrs.push(Instruction::Shorten { from: rw, to: Width::W8 });
                        }
                        // Shift 1
                        self.instrs.push(Instruction::ShiftRight(lw));

                        // Stack now contains inverted bit mask
                        self.instrs.push(Instruction::Not(lw));

                        // Load value
                        self.compile_expr(left.clone(), lty.clone());

                        // AND with inverted bit mask
                        self.instrs.push(Instruction::And(lw));
                        // OR with bool
                        self.instrs.push(Instruction::Or(lw));

                        // Store value
                        self.compile_store(left, lty);
                    }
                    Type::Slice(sty) => {
                        // TODO "FIX EXPR COMPILE SLICE INDEX"

                        let Type::Int(rw, _) = &rty
                        else { panic!("ICE: ir_compiler: invalid index type"); };

                        let rw = *rw;
                        let size = sty.stack_size();

                        // Stack: [value, pointer, length]
                        self.compile_expr(left, lty);
                        // Stack: [value, pointer, length, index]
                        self.compile_expr(right.clone(), rty.clone());

                        if rw < Width::WSize {
                            self.instrs.push(Instruction::Extend { from: rw, to: Width::WSize, signed: false });
                        } else if rw > Width::WSize {
                            self.instrs.push(Instruction::Shorten { from: rw, to: Width::WSize });
                        }

                        // Stack: [value, pointer, index >= length]
                        self.instrs.push(Instruction::IntCompare { width: Width::WSize, cmp: Cmp::GreaterEq, signed: false });

                        // Stack: [value, pointer]
                        self.instrs.push(Instruction::If {
                            yes: Block(vec![Instruction::Error(1)]),
                            no: Block(Vec::new()),
                        });

                        // Stack: [value, pointer, index]
                        self.compile_expr(right, rty);

                        // index: usize
                        if rw < Width::WSize {
                            self.instrs.push(Instruction::Extend { from: rw, to: Width::WSize, signed: false });
                        } else if rw > Width::WSize {
                            self.instrs.push(Instruction::Shorten { from: rw, to: Width::WSize });
                        }

                        // Stack: [value, pointer, index, value_size]
                        self.instrs.push(Instruction::PushInt(size.into()));
                        // Stack: [value, pointer, index * value_size]
                        self.instrs.push(Instruction::Mul { width: Width::WSize, signed: false });
                        // Stack: [value, pointer + index * value_size]
                        self.instrs.push(Instruction::Add(Width::WSize));

                        self.instrs.push(Instruction::Store(size));
                    }
                    _ => panic!("ICE: ir_compiler: invalid type indexed"),
                }
            }
            _ => panic!("ICE: ir_compiler: store in invalid expr"),
        }
    }

    fn compile_assign_convert(&mut self, from: Type, to: Type) {
        let from = from.dereferenced();
        let to = to.dereferenced();

        if from == to { return; }

        match to {
            Type::Int(width, signed) => match from {
                Type::Int(owidth, Signed::No) => {
                    self.instrs.push(Instruction::Extend { from: owidth, to: width, signed: false });
                }
                Type::Int(owidth, Signed::Yes) if signed == Signed::Yes => {
                    self.instrs.push(Instruction::Extend { from: owidth, to: width, signed: true });
                }
                Type::Bool => {
                    self.instrs.push(Instruction::Extend { from: Width::W8, to: width, signed: false });
                }
                Type::Bitfield(_, owidth, _) => {
                    self.instrs.push(Instruction::Extend { from: owidth, to: width, signed: false });
                }
                _ => panic!("ICE: ir_compiler: invalid assign conversion"),
            }
            Type::F32 => match from {
                Type::Int(width, signed) => {
                    self.instrs.push(Instruction::IntToFloat { width: width, signed: signed == Signed::Yes, float: Float::F32 });
                }
                _ => panic!("ICE: ir_compiler: invalid assign conversion"),
            }
            Type::F64 => match from {
                Type::F32 => {
                    self.instrs.push(Instruction::F32To64);
                }
                Type::Int(width, signed) => {
                    self.instrs.push(Instruction::IntToFloat { width: width, signed: signed == Signed::Yes, float: Float::F64 });
                }
                _ => panic!("ICE: ir_compiler: invalid assign conversion"),
            }
            Type::Bitfield(_, _, _) => match from {
                Type::Int(_, _) => {}
                _ => panic!("ICE: ir_compiler: invalid assign conversion"),
            }
            _ => panic!("ICE: ir_compiler: invalid assign conversion"),
        }
    }
}