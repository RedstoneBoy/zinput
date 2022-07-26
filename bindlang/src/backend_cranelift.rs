use std::{collections::HashMap, ffi::c_void};

use cranelift::{
    codegen::{ir::StackSlot, Context},
    frontend::{FunctionBuilder, FunctionBuilderContext, Variable},
    prelude::{
        types, AbiParam, EntityRef, FloatCC, InstBuilder, IntCC, MemFlags, StackSlotData,
        StackSlotKind, Type, Value, isa::CallConv,
    },
};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};

use crate::{
    ast::{
        BinOp, Block as AstBlock, DeviceIn, Expr, ExprKind, Literal, Module as AstModule, Stmt,
        StmtKind, UnOp,
    },
    ty::Type as Ty,
    util::{Int, Signed, Width},
};

const ICE_TYPE: &'static str = "ICE: backend_cranelift: expression without type";
const ICE_EXPECT_VAL: &'static str = "ICE: backend_cranelift: value expression returned stack slot";
const ICE_EXPECT_STACK: &'static str =
    "ICE: backend_cranelift: stack slot expression returned value";

const ERROR_INVALID_NUMBER_OF_INPUTS: u32 = 1;
const ERROR_INDEX_OUT_OF_BOUNDS: u32 = 2;

struct Env<'a> {
    vars: Vec<HashMap<&'a str, Result<Variable, StackSlot>>>,
    next_id: u32,
}

impl<'a> Env<'a> {
    fn new() -> Self {
        Env {
            vars: vec![HashMap::new()],
            next_id: 0,
        }
    }

    fn new_stack(&mut self) {
        self.vars.clear();
        self.vars.push(HashMap::new());
        self.next_id = 0;
    }

    fn push(&mut self) {
        self.vars.push(HashMap::new());
    }

    fn pop(&mut self) {
        self.vars
            .pop()
            .expect("ICE: backend_cranelift: too many environments popped");
    }

    fn insert(&mut self, key: &'a str) -> Variable {
        let vars = self
            .vars
            .last_mut()
            .expect("ICE: backend_cranelift: null environment insert");
        let var = Variable::new(self.next_id as _);
        vars.insert(key, Ok(var));
        self.next_id = match self.next_id.checked_add(1) {
            Some(id) => id,
            None => panic!("ICE: backend_cranelift: more than {} variables", u32::MAX),
        };

        var
    }

    fn insert_stack(&mut self, key: &'a str, slot: StackSlot) {
        let vars = self
            .vars
            .last_mut()
            .expect("ICE: backend_cranelift: null environment insert");

        vars.insert(key, Err(slot));
    }

    fn get(&self, key: &'a str) -> Result<Variable, StackSlot> {
        for vars in self.vars.iter().rev() {
            if let Some(var) = vars.get(key) {
                return *var;
            }
        }

        panic!("ICE: backend_cranelift: variable does not exist")
    }
}

pub type CompiledFunction = extern "sysv64" fn(*mut c_void, *mut c_void, u32) -> u32;

pub struct Compiler<'a> {
    src: &'a str,
    env: Env<'a>,

    bctx: FunctionBuilderContext,
    ctx: Context,
    module: JITModule,
}

impl<'a> Compiler<'a> {
    pub fn new(src: &'a str) -> Self {
        let builder = JITBuilder::new(cranelift_module::default_libcall_names())
            .expect("ICE: backend_cranelift: error creating JITBuilder");
        let module = JITModule::new(builder);

        Compiler {
            src,
            env: Env::new(),

            bctx: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            module,
        }
    }

    pub fn compile(mut self, module: AstModule) -> Vec<CompiledFunction> {
        let d_out = module.output.index_src(self.src);
        let inputs = module
            .inputs
            .iter()
            .map(|input| input.device.index_src(self.src))
            .collect::<Vec<_>>();

        let mut funcs = Vec::new();

        for (i, func) in module.inputs.into_iter().enumerate() {
            self.env.new_stack();

            self.compile_function(func, d_out, &inputs);

            let id = self
                .module
                .declare_function(&format!("{i}"), Linkage::Export, &self.ctx.func.signature)
                .expect("ICE: backend_cranelift: error declaring function");

            self.module
                .define_function(id, &mut self.ctx)
                .expect("ICE: backend_cranelift: error defining function");

            self.module.clear_context(&mut self.ctx);

            funcs.push(id);
        }

        self.module.finalize_definitions();

        let mut ret = Vec::new();
        for func_id in funcs {
            let ptr = self.module.get_finalized_function(func_id);
            ret.push(unsafe { std::mem::transmute(ptr) });
        }

        ret
    }

    fn compile_function(&mut self, func: DeviceIn, d_out: &'a str, inputs: &[&'a str]) {
        let ptr_type = self.module.target_config().pointer_type();

        // function parameters
        {
            let sig = &mut self.ctx.func.signature;
            sig.call_conv = CallConv::SystemV;
            sig.params.push(AbiParam::new(ptr_type));
            sig.params.push(AbiParam::new(ptr_type));
            sig.params.push(AbiParam::new(types::I32));
            sig.returns.push(AbiParam::new(types::I32));
        }

        let (mut builder, block) = {
            let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.bctx);
            let entry_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);

            (builder, entry_block)
        };

        let fparam1 = builder.block_params(block)[0];
        let fparam2 = builder.block_params(block)[1];
        let fparam3 = builder.block_params(block)[2];

        // device_out variable
        {
            let val = fparam1;
            let var = self.env.insert(d_out);
            builder.declare_var(var, ptr_type);
            builder.def_var(var, val);
        }

        // verify number of input variables
        {
            let val_num_inputs = fparam3;
            let val_cond =
                builder
                    .ins()
                    .icmp_imm(IntCC::NotEqual, val_num_inputs, inputs.len() as i64);

            let then_block = builder.create_block();
            let cont_block = builder.create_block();

            builder.ins().brnz(val_cond, then_block, &[]);
            builder.ins().jump(cont_block, &[]);

            builder.switch_to_block(then_block);
            builder.seal_block(then_block);
            let ret_val = builder
                .ins()
                .iconst(types::I32, ERROR_INVALID_NUMBER_OF_INPUTS as i64);
            builder.ins().return_(&[ret_val]);

            builder.switch_to_block(cont_block);
            builder.seal_block(cont_block);
        }

        // input variables
        {
            let input_list_ptr_val = fparam2;
            for (i, input) in inputs.into_iter().enumerate() {
                let offset = i as u32 * ptr_type.bytes();
                let offset_val = builder.ins().iconst(ptr_type, offset as i64);
                let ptr_val = builder.ins().iadd(input_list_ptr_val, offset_val);
                let var_val = builder.ins().load(ptr_type, MemFlags::new(), ptr_val, 0i32);

                let var = self.env.insert(input);
                builder.declare_var(var, ptr_type);
                builder.def_var(var, var_val);
            }
        }

        // compile body
        let mut func_compiler = FunctionCompiler {
            src: self.src,
            env: &mut self.env,
            builder,
            module: &mut self.module,
            ptr_type,
        };

        func_compiler.compile(func.body);

        let ret = func_compiler.builder.ins().iconst(types::I32, 0i64);
        func_compiler.builder.ins().return_(&[ret]);
        func_compiler.builder.finalize();
    }
}

struct FunctionCompiler<'a, 'b> {
    src: &'a str,
    env: &'b mut Env<'a>,
    builder: FunctionBuilder<'b>,
    module: &'b mut JITModule,
    ptr_type: Type,
}

impl<'a, 'b> FunctionCompiler<'a, 'b> {
    fn compile(&mut self, block: AstBlock) {
        self.env.push();

        for stmt in block.stmts {
            self.compile_stmt(stmt);
        }

        self.env.pop();
    }

    fn compile_stmt(&mut self, stmt: Stmt) {
        match stmt.kind {
            StmtKind::Let { name, expr } => {
                let name = name.index_src(self.src);

                let ty = expr.ty.clone().expect(ICE_TYPE).dereferenced();
                match self.convert_type(ty) {
                    Some(ty) => {
                        let var = self.env.insert(name);
                        self.builder.declare_var(var, ty);
                        let val = self.compile_expr(expr).expect(ICE_EXPECT_VAL);
                        self.builder.def_var(var, val);
                    }
                    None => {
                        let slot = self.compile_expr(expr).expect_err(ICE_EXPECT_VAL);

                        self.env.insert_stack(name, slot);
                    }
                }
            }
            StmtKind::Assign { lval, expr, .. } => {
                let lty = lval.ty.clone().expect(ICE_TYPE).dereferenced();
                let rty = expr.ty.clone().expect(ICE_TYPE).dereferenced();

                let value = self.compile_expr(expr);
                let value = self.compile_assign_convert(value, rty, lty);

                self.compile_assign(lval, value);
            }
            StmtKind::If { cond, yes, no } => {
                let cond_val = self.compile_expr(cond).expect(ICE_EXPECT_VAL);

                let then_block = self.builder.create_block();
                let else_block = self.builder.create_block();
                let merge_block = self.builder.create_block();

                self.builder.ins().brz(cond_val, else_block, &[]);
                self.builder.ins().jump(then_block, &[]);

                self.builder.switch_to_block(then_block);
                self.builder.seal_block(then_block);
                self.compile(yes);
                self.builder.ins().jump(merge_block, &[]);

                self.builder.switch_to_block(else_block);
                self.builder.seal_block(else_block);
                if let Some(no) = no {
                    self.compile(no);
                }
                self.builder.ins().jump(merge_block, &[]);

                self.builder.switch_to_block(merge_block);
                self.builder.seal_block(merge_block);
            }
            StmtKind::Expr(e) => {
                let _ = self.compile_expr(e);
            }
        }
    }

    fn compile_expr(&mut self, expr: Expr) -> Result<Value, StackSlot> {
        Ok(match expr.kind {
            ExprKind::Literal(l) => match l {
                Literal::Int(int, _) => match int {
                    Int::W8(val) => self.builder.ins().iconst(types::I8, val as i64),
                    Int::W16(val) => self.builder.ins().iconst(types::I16, val as i64),
                    Int::W32(val) => self.builder.ins().iconst(types::I32, val as i64),
                    Int::W64(val) => self.builder.ins().iconst(types::I64, val as i64),
                },
                Literal::Float(val) => self.builder.ins().f64const(val),
                Literal::Bool(val) => self.builder.ins().bconst(types::B1, val),
            },
            ExprKind::Var(ident) => {
                let ident = ident.index_src(self.src);

                match self.env.get(ident) {
                    Ok(var) => self.builder.use_var(var),
                    Err(ss) => return Err(ss),
                }
            }
            ExprKind::Dot(left, field) => {
                let field = field.index_src(self.src);

                let lty = left.ty.clone().expect(ICE_TYPE).dereferenced();
                let lval = self.compile_expr(*left);

                match lty {
                    Ty::Slice(_) => match field {
                        "len" => {
                            let slot = lval.expect_err(ICE_EXPECT_STACK);
                            self.builder.ins().stack_load(self.ptr_type, slot, 8i32)
                        }
                        _ => panic!("ICE: backend_cranelift: invalid slice field access"),
                    },
                    Ty::Bitfield(_, _, names) => {
                        let bit = *names
                            .0
                            .get(field)
                            .expect("ICE: backend_cranelift: invalid bitfield access");

                        let val = lval.expect(ICE_EXPECT_VAL);
                        let val = self.builder.ins().ushr_imm(val, bit as i64);
                        let val = self.builder.ins().band_imm(val, 1i64);

                        self.builder.ins().icmp_imm(IntCC::NotEqual, val, 0i64)
                    }
                    Ty::Struct(s) => {
                        let field = s
                            .fields
                            .get(field)
                            .expect("ICE: backend_cranelift: invalid struct field access");

                        let ptr_val = lval.expect(ICE_EXPECT_VAL);
                        let ptr_val = self
                            .builder
                            .ins()
                            .iadd_imm(ptr_val, field.byte_offset as i64);

                        match self.convert_type(field.ty.clone()) {
                            Some(ty) => self.builder.ins().load(ty, MemFlags::new(), ptr_val, 0i32),
                            None => {
                                let slot = self.builder.create_stack_slot(StackSlotData::new(
                                    StackSlotKind::ExplicitSlot,
                                    field.ty.stack_size() as u32,
                                ));
                                let slot_addr_val =
                                    self.builder.ins().stack_addr(self.ptr_type, slot, 0i32);

                                let size_val = self
                                    .builder
                                    .ins()
                                    .iconst(self.ptr_type, field.ty.stack_size() as i64);

                                self.builder.call_memcpy(
                                    self.module.target_config(),
                                    slot_addr_val,
                                    ptr_val,
                                    size_val,
                                );

                                return Err(slot);
                            }
                        }
                    }
                    _ => panic!("ICE: backend_cranelift: field access on invalid type"),
                }
            }
            ExprKind::Index(pexpr, iexpr) => {
                let pty = pexpr.ty.clone().expect(ICE_TYPE).dereferenced();
                let pval = self.compile_expr(*pexpr);

                // let ity = iexpr.ty.clone().expect(ICE_TYPE).dereferenced();
                let ival = self.compile_expr(*iexpr).expect(ICE_EXPECT_VAL);

                match pty {
                    Ty::Int(_, _) | Ty::Bitfield(_, _, _) => {
                        let val = pval.expect(ICE_EXPECT_VAL);
                        let val = self.builder.ins().ushr(val, ival);
                        let val = self.builder.ins().band_imm(val, 1i64);

                        self.builder.ins().icmp_imm(IntCC::NotEqual, val, 0i64)
                    }
                    Ty::Slice(rty) => {
                        let rty = rty.dereferenced();

                        let pval = pval.expect_err(ICE_EXPECT_STACK);

                        // bounds-check
                        {
                            let len_val = self.builder.ins().stack_load(self.ptr_type, pval, 8i32);

                            let val_cond =
                                self.builder
                                    .ins()
                                    .icmp(IntCC::UnsignedLessThan, ival, len_val);

                            let then_block = self.builder.create_block();
                            let cont_block = self.builder.create_block();

                            self.builder.ins().brz(val_cond, then_block, &[]);
                            self.builder.ins().jump(cont_block, &[]);

                            self.builder.switch_to_block(then_block);
                            self.builder.seal_block(then_block);
                            let ret_val = self
                                .builder
                                .ins()
                                .iconst(types::I32, ERROR_INDEX_OUT_OF_BOUNDS as i64);
                            self.builder.ins().return_(&[ret_val]);

                            self.builder.switch_to_block(cont_block);
                            self.builder.seal_block(cont_block);
                        }

                        let ptr_offset = self.builder.ins().imul_imm(ival, rty.stack_size() as i64);

                        let pval = self.builder.ins().stack_load(self.ptr_type, pval, 0i32);

                        let ptr_val = self.builder.ins().iadd(pval, ptr_offset);

                        match self.convert_type(rty.clone()) {
                            Some(ty) => self.builder.ins().load(ty, MemFlags::new(), ptr_val, 0i32),
                            None => {
                                let slot = self.builder.create_stack_slot(StackSlotData::new(
                                    StackSlotKind::ExplicitSlot,
                                    rty.stack_size() as u32,
                                ));
                                let slot_addr_val =
                                    self.builder.ins().stack_addr(self.ptr_type, slot, 0i32);

                                let size_val = self
                                    .builder
                                    .ins()
                                    .iconst(self.ptr_type, rty.stack_size() as i64);

                                self.builder.call_memcpy(
                                    self.module.target_config(),
                                    slot_addr_val,
                                    ptr_val,
                                    size_val,
                                );

                                return Err(slot);
                            }
                        }
                    }
                    _ => panic!("ICE: backend_cranelift: index on invalid type"),
                }
            }

            ExprKind::Unary(op, expr) => {
                let ty = expr.ty.clone().expect(ICE_TYPE).dereferenced();
                let val = self.compile_expr(*expr).expect(ICE_EXPECT_VAL);

                match op {
                    UnOp::Negate => match ty {
                        Ty::Int(_, _) => self.builder.ins().ineg(val),
                        Ty::F32 | Ty::F64 => self.builder.ins().fneg(val),
                        _ => panic!("ICE: backend_cranelift: invalid unary op '-'"),
                    },
                    UnOp::Not => match ty {
                        Ty::Bool | Ty::Int(_, _) | Ty::Bitfield(_, _, _) => {
                            self.builder.ins().bnot(val)
                        }
                        _ => panic!("ICE: backend_cranelift: invalid unary op '!'"),
                    },
                }
            }
            ExprKind::Binary(left, op, right) => {
                let lty = left.ty.clone().expect(ICE_TYPE).dereferenced();

                let lval = self.compile_expr(*left).expect(ICE_EXPECT_VAL);

                let rval = self.compile_expr(*right).expect(ICE_EXPECT_VAL);

                match op {
                    BinOp::BitOr | BinOp::Or => self.builder.ins().bor(lval, rval),
                    BinOp::BitAnd | BinOp::And => self.builder.ins().band(lval, rval),
                    BinOp::BitXor => self.builder.ins().bxor(lval, rval),

                    BinOp::Add => match lty {
                        Ty::Int(_, _) => self.builder.ins().iadd(lval, rval),
                        Ty::F32 | Ty::F64 => self.builder.ins().fadd(lval, rval),
                        _ => panic!("ICE: backend_cranelift: invalid binary op '+'"),
                    },
                    BinOp::Sub => match lty {
                        Ty::Int(_, _) => self.builder.ins().isub(lval, rval),
                        Ty::F32 | Ty::F64 => self.builder.ins().fsub(lval, rval),
                        _ => panic!("ICE: backend_cranelift: invalid binary op '+'"),
                    },
                    BinOp::Mul => match lty {
                        Ty::Int(_, _) => self.builder.ins().imul(lval, rval),
                        Ty::F32 | Ty::F64 => self.builder.ins().fmul(lval, rval),
                        _ => panic!("ICE: backend_cranelift: invalid binary op '+'"),
                    },
                    BinOp::Div => match lty {
                        Ty::Int(_, Signed::No) => self.builder.ins().udiv(lval, rval),
                        Ty::Int(_, Signed::Yes) => self.builder.ins().sdiv(lval, rval),
                        Ty::F32 | Ty::F64 => self.builder.ins().fdiv(lval, rval),
                        _ => panic!("ICE: backend_cranelift: invalid binary op '+'"),
                    },

                    BinOp::Greater | BinOp::GreaterEq | BinOp::Less | BinOp::LessEq => match lty {
                        Ty::Int(_, Signed::No) => {
                            let cc = match op {
                                BinOp::Greater => IntCC::UnsignedGreaterThan,
                                BinOp::GreaterEq => IntCC::UnsignedGreaterThanOrEqual,
                                BinOp::Less => IntCC::UnsignedLessThan,
                                BinOp::LessEq => IntCC::UnsignedLessThanOrEqual,
                                _ => unreachable!(),
                            };

                            self.builder.ins().icmp(cc, lval, rval)
                        }
                        Ty::Int(_, Signed::Yes) => {
                            let cc = match op {
                                BinOp::Greater => IntCC::SignedGreaterThan,
                                BinOp::GreaterEq => IntCC::SignedGreaterThanOrEqual,
                                BinOp::Less => IntCC::SignedLessThan,
                                BinOp::LessEq => IntCC::SignedLessThanOrEqual,
                                _ => unreachable!(),
                            };

                            self.builder.ins().icmp(cc, lval, rval)
                        }
                        Ty::F32 | Ty::F64 => {
                            let cc = match op {
                                BinOp::Greater => FloatCC::GreaterThan,
                                BinOp::GreaterEq => FloatCC::GreaterThanOrEqual,
                                BinOp::Less => FloatCC::LessThan,
                                BinOp::LessEq => FloatCC::LessThanOrEqual,
                                _ => unreachable!(),
                            };

                            self.builder.ins().fcmp(cc, lval, rval)
                        }
                        _ => panic!("ICE: backend_cranelift: invalid binary compare op"),
                    },

                    BinOp::Equals | BinOp::NotEquals => match lty {
                        Ty::Int(_, Signed::No) => {
                            let cc = match op {
                                BinOp::Equals => IntCC::Equal,
                                BinOp::NotEquals => IntCC::NotEqual,
                                _ => unreachable!(),
                            };

                            self.builder.ins().icmp(cc, lval, rval)
                        }
                        Ty::F32 | Ty::F64 => {
                            let cc = match op {
                                BinOp::Equals => FloatCC::Equal,
                                BinOp::NotEquals => FloatCC::NotEqual,
                                _ => unreachable!(),
                            };

                            self.builder.ins().fcmp(cc, lval, rval)
                        }
                        Ty::Bool => {
                            let out = self.builder.ins().bxor(lval, rval);

                            match op {
                                BinOp::Equals => self.builder.ins().bnot(out),
                                BinOp::NotEquals => out,
                                _ => unreachable!(),
                            }
                        }
                        _ => panic!("ICE: backend_cranelift: invalid binary equals op"),
                    },

                    BinOp::ShiftLeft => self.builder.ins().ishl(lval, rval),
                    BinOp::ShiftRight => self.builder.ins().ushr(lval, rval),
                }
            }
        })
    }

    /// Returns a variable in Ok, or an address to write to in Err
    fn compile_assign(&mut self, expr: Expr, val: Result<Value, StackSlot>) {
        let val_size = expr.ty.clone().expect(ICE_TYPE).dereferenced().stack_size();

        match expr.kind {
            ExprKind::Var(ident) => {
                let ident = ident.index_src(self.src);

                match self.env.get(ident) {
                    Ok(var) => {
                        let val = val.expect(ICE_EXPECT_VAL);
                        self.builder.def_var(var, val);
                    }
                    Err(src) => {
                        let src_ptr = self.builder.ins().stack_addr(self.ptr_type, src, 0i32);

                        let dest = val.expect_err(ICE_EXPECT_STACK);
                        let dest_ptr = self.builder.ins().stack_addr(self.ptr_type, dest, 0i32);

                        let size_val = self.builder.ins().iconst(self.ptr_type, val_size as i64);

                        self.builder.call_memcpy(
                            self.module.target_config(),
                            dest_ptr,
                            src_ptr,
                            size_val,
                        );
                    }
                }
            }
            ExprKind::Dot(lexpr, field) => {
                let field = field.index_src(self.src);

                let lty = lexpr.ty.clone().expect(ICE_TYPE).dereferenced();

                match lty.clone() {
                    Ty::Bitfield(_, _, names) => {
                        let val = val.expect(ICE_EXPECT_VAL);

                        let clty = self.convert_type(lty).expect(ICE_EXPECT_VAL);

                        let bit = *names
                            .0
                            .get(field)
                            .expect("ICE: backend_cranelift: invalid bitfield access");

                        let clear_mask = !(1u64 << bit);
                        let or_val = self.builder.ins().bint(clty, val);
                        let or_val = self.builder.ins().ishl_imm(or_val, bit as i64);

                        let bitfield = self.compile_expr(*lexpr.clone()).expect(ICE_EXPECT_VAL);

                        let out = self.builder.ins().band_imm(bitfield, clear_mask as i64);
                        let out = self.builder.ins().bor(out, or_val);

                        self.compile_assign(*lexpr, Ok(out));
                    }
                    Ty::Struct(s) => {
                        let ptr_val = self.compile_expr(*lexpr).expect(ICE_EXPECT_VAL);

                        let field = s
                            .fields
                            .get(field)
                            .expect("ICE: backend_cranelift: invalid struct field access");

                        let ptr_val = self
                            .builder
                            .ins()
                            .iadd_imm(ptr_val, field.byte_offset as i64);
                        assert!(
                            expr.ty.expect(ICE_TYPE).dereferenced() == field.ty,
                            "ICE: backend_cranelift: tried to load wrong type from struct field"
                        );

                        match val {
                            Ok(val) => {
                                self.builder
                                    .ins()
                                    .store(MemFlags::new(), val, ptr_val, 0i32);
                            }
                            Err(src) => {
                                let src_ptr =
                                    self.builder.ins().stack_addr(self.ptr_type, src, 0i32);
                                let dest_ptr = ptr_val;

                                let size_val =
                                    self.builder.ins().iconst(self.ptr_type, val_size as i64);

                                self.builder.call_memcpy(
                                    self.module.target_config(),
                                    dest_ptr,
                                    src_ptr,
                                    size_val,
                                );
                            }
                        }
                    }
                    _ => panic!("ICE: backend_cranelift: field access on invalid type"),
                }
            }
            ExprKind::Index(pexpr, iexpr) => {
                let ival = self.compile_expr(*iexpr).expect(ICE_EXPECT_VAL);

                let pty = pexpr.ty.clone().expect(ICE_TYPE).dereferenced();
                match pty.clone() {
                    Ty::Bitfield(_, _, _) | Ty::Int(_, _) => {
                        let cpty = self.convert_type(pty).expect(ICE_EXPECT_VAL);

                        let val = val.expect(ICE_EXPECT_VAL);

                        let bit_val = ival;
                        let clear_mask_val = self.builder.ins().iconst(cpty, 1i64);
                        let clear_mask_val = self.builder.ins().ishl(clear_mask_val, bit_val);
                        let clear_mask_val = self.builder.ins().bnot(clear_mask_val);

                        let or_val = self.builder.ins().bint(cpty, val);
                        let or_val = self.builder.ins().ishl(or_val, bit_val);

                        let bitfield = self.compile_expr(*pexpr.clone()).expect(ICE_EXPECT_VAL);

                        let out = self.builder.ins().band(bitfield, clear_mask_val);
                        let out = self.builder.ins().bor(out, or_val);

                        self.compile_assign(*pexpr, Ok(out));
                    }
                    Ty::Slice(sty) => {
                        let slice_slot = self.compile_expr(*pexpr).expect_err(ICE_EXPECT_STACK);

                        // bounds-check
                        {
                            let len_val =
                                self.builder
                                    .ins()
                                    .stack_load(self.ptr_type, slice_slot, 8i32);

                            let val_cond =
                                self.builder
                                    .ins()
                                    .icmp(IntCC::UnsignedLessThan, ival, len_val);

                            let then_block = self.builder.create_block();
                            let cont_block = self.builder.create_block();

                            self.builder.ins().brz(val_cond, then_block, &[]);
                            self.builder.ins().jump(cont_block, &[]);

                            self.builder.switch_to_block(then_block);
                            self.builder.seal_block(then_block);
                            let ret_val = self
                                .builder
                                .ins()
                                .iconst(types::I32, ERROR_INDEX_OUT_OF_BOUNDS as i64);
                            self.builder.ins().return_(&[ret_val]);

                            self.builder.switch_to_block(cont_block);
                            self.builder.seal_block(cont_block);
                        }

                        let ptr_offset = self.builder.ins().imul_imm(ival, sty.stack_size() as i64);
                        let ptr_val =
                            self.builder
                                .ins()
                                .stack_load(self.ptr_type, slice_slot, 0i32);
                        let ptr_val = self.builder.ins().iadd(ptr_val, ptr_offset);

                        match val {
                            Ok(val) => {
                                self.builder
                                    .ins()
                                    .store(MemFlags::new(), val, ptr_val, 0i32);
                            }
                            Err(src) => {
                                let src_ptr =
                                    self.builder.ins().stack_addr(self.ptr_type, src, 0i32);
                                let dest_ptr = ptr_val;

                                let size_val =
                                    self.builder.ins().iconst(self.ptr_type, val_size as i64);

                                self.builder.call_memcpy(
                                    self.module.target_config(),
                                    dest_ptr,
                                    src_ptr,
                                    size_val,
                                );
                            }
                        }
                    }
                    _ => panic!("ICE: backend_cranelift: index on invalid type"),
                }
            }
            _ => panic!("ICE: backend_cranelift: invalid assign expression"),
        }
    }

    fn compile_assign_convert(
        &mut self,
        val: Result<Value, StackSlot>,
        from: Ty,
        to: Ty,
    ) -> Result<Value, StackSlot> {
        if from == to {
            return val;
        }

        Ok(match to {
            Ty::Int(w, s) => match from {
                Ty::Int(ow, Signed::No) if ow < w => self
                    .builder
                    .ins()
                    .uextend(convert_width(w), val.expect(ICE_EXPECT_VAL)),
                
                Ty::Int(ow, Signed::No) if ow == w => return val,

                Ty::Int(ow, Signed::Yes) if s == Signed::Yes && ow < w => self
                    .builder
                    .ins()
                    .sextend(convert_width(w), val.expect(ICE_EXPECT_VAL)),

                Ty::Int(ow, Signed::No) if s == Signed::Yes && ow == w => return val,
                
                Ty::Bool => self
                    .builder
                    .ins()
                    .bint(convert_width(w), val.expect(ICE_EXPECT_VAL)),

                Ty::Bitfield(_, ow, _) if ow < w => self
                    .builder
                    .ins()
                    .uextend(convert_width(w), val.expect(ICE_EXPECT_VAL)),

                Ty::Bitfield(_, ow, _) if ow == w => return val,
                
                _ => panic!("ICE: backend_cranelift: invalid assign conversion"),
            },
            Ty::F32 => match from {
                Ty::F64 => self
                    .builder
                    .ins()
                    .fdemote(types::F32, val.expect(ICE_EXPECT_VAL)),

                Ty::Int(_, Signed::No) => self
                    .builder
                    .ins()
                    .fcvt_from_uint(types::F32, val.expect(ICE_EXPECT_VAL)),

                Ty::Int(_, Signed::Yes) => self
                    .builder
                    .ins()
                    .fcvt_from_sint(types::F32, val.expect(ICE_EXPECT_VAL)),

                _ => panic!("ICE: backend_cranelift: invalid assign conversion"),
            },
            Ty::F64 => match from {
                Ty::F32 => self
                    .builder
                    .ins()
                    .fpromote(types::F64, val.expect(ICE_EXPECT_VAL)),

                Ty::Int(_, Signed::No) => self
                    .builder
                    .ins()
                    .fcvt_from_uint(types::F64, val.expect(ICE_EXPECT_VAL)),

                Ty::Int(_, Signed::Yes) => self
                    .builder
                    .ins()
                    .fcvt_from_sint(types::F64, val.expect(ICE_EXPECT_VAL)),

                _ => panic!("ICE: backend_cranelift: invalid assign conversion"),
            },
            Ty::Bitfield(_, w, _) => match from {
                Ty::Bitfield(_, ow, _) | Ty::Int(ow, _) if ow == w => return val,
                _ => panic!("ICE: backend_cranelift: invalid assign conversion"),
            },
            _ => panic!("ICE: backend_cranelift: invalid assign conversion"),
        })
    }

    fn convert_type(&self, ty: Ty) -> Option<Type> {
        Some(match ty.dereferenced() {
            Ty::Reference(_, _) => unreachable!(),
            Ty::Int(w, _) | Ty::Bitfield(_, w, _) => match w {
                Width::W8 => types::I8,
                Width::W16 => types::I16,
                Width::W32 => types::I32,
                Width::W64 => types::I64,
            },
            Ty::F32 => types::F32,
            Ty::F64 => types::F64,
            Ty::Bool => types::B1,
            Ty::Slice(_) => return None,
            Ty::Struct(_) => self.ptr_type,
        })
    }
}

fn convert_width(width: Width) -> Type {
    match width {
        Width::W8 => types::I8,
        Width::W16 => types::I16,
        Width::W32 => types::I32,
        Width::W64 => types::I64,
    }
}
