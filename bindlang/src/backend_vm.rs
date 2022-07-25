use std::{ffi::c_void, collections::HashMap};

use crate::{ir::{Instruction as Ins, Module, Body, Block, Float, Cmp}, util::{Int, Width}};


pub struct Vm {
    stack: Vec<u8>,
    var_space: Vec<u8>,
    // VarIndex -> index for vars
    var_map: HashMap<u32, usize>,
}

impl Vm {
    pub fn new() -> Self {
        Vm {
            stack: Vec::new(),
            var_space: Vec::new(),
            var_map: HashMap::new(),
        }
    }


    pub fn run(&mut self, module: &Module, dev_in_id: usize, dev_out: *mut c_void, dev_ins: &[*mut c_void]) -> u8 {
        self.stack.clear();
        self.var_space.clear();
        self.var_map.clear();

        self.run_body(&module.inputs[dev_in_id], dev_out, dev_ins)        
    }

    fn run_body(&mut self, body: &Body, dev_out: *mut c_void, dev_ins: &[*mut c_void]) -> u8 {
        for var_id in 0..body.num_vars {
            let var_addr = self.var_space.len();
            let var_size = body.var_sizes[&var_id];

            self.var_map.insert(var_id, var_addr);
            self.var_space.extend(std::iter::repeat(0).take(var_size));
        }

        self.var_space[0..8].copy_from_slice(&dev_out.to_usize().to_le_bytes());
        for i in 0..dev_ins.len() {
            let ptr = dev_ins[i].to_usize();
            self.var_space[((i + 1) * 8)..][..8].copy_from_slice(&ptr.to_le_bytes());
        }

        match self.run_block(&body.block) {
            Ok(()) => 0,
            Err(err) => err,
        }
    }

    fn run_block(&mut self, block: &Block) -> Result<(), u8> {
        for instr in &block.0 {
            self.run_instr(instr)?;
        }

        Ok(())
    }

    fn run_instr(&mut self, instr: &Ins) -> Result<(), u8> {
        match instr {
            Ins::PushInt(Int::W8(int)) => self.stack.push(*int),
            Ins::PushInt(Int::W16(int)) => self.stack.extend(&int.to_le_bytes()),
            Ins::PushInt(Int::W32(int)) => self.stack.extend(&int.to_le_bytes()),
            Ins::PushInt(Int::W64(int)) => self.stack.extend(&int.to_le_bytes()),

            Ins::Pop(size) => for _ in 0..*size { self.stack.pop(); },

            Ins::VarGet(size, var_id) => {
                let var_ptr = self.var_map[var_id];

                for i in 0..*size {
                    let byte = self.var_space[var_ptr + i];
                    self.stack.push(byte);
                }
            }
            Ins::VarPut(size, var_id) => {
                let start = self.var_map[var_id];
                let end = start + *size;
                
                for i in start..end {
                    let byte = self.stack.pop().unwrap();
                    self.var_space[i] = byte;
                }

                self.var_space[start..end].reverse();
            }

            Ins::Load(size) => {
                let ptr = self.pop_ptr() as *mut u8;
                assert!((ptr as usize) % *size == 0, "unaligned load");

                for i in 0..*size {
                    unsafe {
                        let byte = *ptr.offset(i as _);
                        self.stack.push(byte);
                    }
                }
            }
            Ins::Store(size) => {
                let ptr = self.pop_ptr() as *mut u8;
                assert!((ptr as usize) % *size == 0, "unaligned store");

                for i in (0..*size).rev() {
                    let byte = self.stack.pop().unwrap();
                    unsafe { *ptr.offset(i as _) = byte; }
                }
            }

            Ins::BoolNot => {
                let val = self.pop_u8() != 0;
                self.stack.push(!val as _);
            }
            
            Ins::Neg(w) => self.int_op_unary(w, Int::neg),
            Ins::Not(w) => self.int_op_unary(w, Int::not),

            Ins::Or(w) => self.int_op_binary(w, Int::or),
            Ins::And(w) => self.int_op_binary(w, Int::and),
            Ins::Xor(w) => self.int_op_binary(w, Int::xor),
            
            Ins::Add(w) => self.int_op_binary(w, Int::add),
            Ins::Sub(w) => self.int_op_binary(w, Int::sub),
            Ins::Mul { width: w, signed: false } => self.int_op_binary(w, Int::mul_unsigned),
            Ins::Mul { width: w, signed: true } => self.int_op_binary(w, Int::mul_signed),
            Ins::Div { width: w, signed: false } => self.int_op_binary(w, Int::div_unsigned),
            Ins::Div { width: w, signed: true } => self.int_op_binary(w, Int::div_signed),

            Ins::ShiftLeft(w) => {
                let bit = self.pop_u8();
                self.int_op_unary(w, |i| Int::shift_left(i, bit))
            }
            Ins::ShiftRight(w) => {
                let bit = self.pop_u8();
                self.int_op_unary(w, |i| Int::shift_right(i, bit))
            }

            Ins::IntCompare { width: w, cmp, signed: false }
                => self.int_op_binary(w, |l, r| Int::cmp_unsigned(l, r, *cmp)),
            Ins::IntCompare { width: w, cmp, signed: true }
                => self.int_op_binary(w, |l, r| Int::cmp_signed(l, r, *cmp)),

            Ins::FloatNeg(Float::F32) => {
                let val = self.pop_f32();
                self.push_f32(-val);
            }
            Ins::FloatAdd(Float::F32) => {
                let l = self.pop_f32();
                let r = self.pop_f32();
                self.push_f32(l + r);
            }
            Ins::FloatSub(Float::F32) => {
                let l = self.pop_f32();
                let r = self.pop_f32();
                self.push_f32(l - r);
            }
            Ins::FloatMul(Float::F32) => {
                let l = self.pop_f32();
                let r = self.pop_f32();
                self.push_f32(l * r);
            }
            Ins::FloatDiv(Float::F32) => {
                let l = self.pop_f32();
                let r = self.pop_f32();
                self.push_f32(l / r);
            }
            Ins::FloatCompare(Float::F32, cmp) => {
                let l = self.pop_f32();
                let r = self.pop_f32();
                let b = match cmp {
                    Cmp::Eq => l == r,
                    Cmp::Neq => l != r,
                    Cmp::Greater => l > r,
                    Cmp::GreaterEq => l >= r,
                    Cmp::Less => l < r,
                    Cmp::LessEq => l <= r,
                };
                self.push_u8(b as _);
            }
            Ins::FloatNeg(Float::F64) => {
                let val = self.pop_f64();
                self.push_f64(-val);
            }
            Ins::FloatAdd(Float::F64) => {
                let l = self.pop_f64();
                let r = self.pop_f64();
                self.push_f64(l + r);
            }
            Ins::FloatSub(Float::F64) => {
                let l = self.pop_f64();
                let r = self.pop_f64();
                self.push_f64(l - r);
            }
            Ins::FloatMul(Float::F64) => {
                let l = self.pop_f64();
                let r = self.pop_f64();
                self.push_f64(l * r);
            }
            Ins::FloatDiv(Float::F64) => {
                let l = self.pop_f64();
                let r = self.pop_f64();
                self.push_f64(l / r);
            }
            Ins::FloatCompare(Float::F64, cmp) => {
                let l = self.pop_f64();
                let r = self.pop_f64();
                let b = match cmp {
                    Cmp::Eq => l == r,
                    Cmp::Neq => l != r,
                    Cmp::Greater => l > r,
                    Cmp::GreaterEq => l >= r,
                    Cmp::Less => l < r,
                    Cmp::LessEq => l <= r,
                };
                self.push_u8(b as _);
            }
            
            Ins::Extend { from, to, signed: false } | Ins::Shorten { from, to } => {
                let int = self.pop_int(*from);
                let val: u64 = int.into();
                let int = to.int_truncate(val);
                self.push_int(int);
            }
            Ins::Extend { from, to, signed: true } => self.int_op_unary(from, |i| Int::sign_extend(i, *to)),

            Ins::F32To64 => {
                let val = self.pop_f32();
                self.push_f64(val as _);
            }
            Ins::F64To32 => {
                let val = self.pop_f64();
                self.push_f32(val as _);
            }

            Ins::IntToFloat { width, signed: false, float: Float::F32 } => self.int_op_unary(width, Int::to_f32_unsigned),
            Ins::IntToFloat { width, signed: false, float: Float::F64 } => self.int_op_unary(width, Int::to_f64_unsigned),
            Ins::IntToFloat { width, signed: true, float: Float::F32 } => self.int_op_unary(width, Int::to_f32_signed),
            Ins::IntToFloat { width, signed: true, float: Float::F64 } => self.int_op_unary(width, Int::to_f64_signed),

            Ins::FloatToInt { width, signed, float: Float::F32 } => {
                let val = self.pop_f32();
                self.push_int(Int::from_f32(val, *width, *signed));
            }
            Ins::FloatToInt { width, signed, float: Float::F64 } => {
                let val = self.pop_f64();
                self.push_int(Int::from_f64(val, *width, *signed));
            }

            Ins::If { yes, no } => {
                let cond = self.pop_u8();
                if cond != 0 {
                    self.run_block(yes)?;
                } else {
                    self.run_block(no)?;
                }
            }

            Ins::Swap(len) => {
                let mut a = self.stack.len() - 1;
                let mut b = self.stack.len() - 1 - *len;
                for _ in 0..*len {
                    self.stack.swap(a, b);
                    a -= 1;
                    b -= 1;
                }
            }
            Ins::Error(err) => return Err(*err),
        }

        Ok(())
    }

    fn int_op_unary(&mut self, w: &Width, op: impl FnOnce(Int) -> Int) {
        let int = self.pop_int(*w);
        let int = op(int);
        self.push_int(int);
    }

    fn int_op_binary(&mut self, w: &Width, op: impl FnOnce(Int, Int) -> Int) {
        let left = self.pop_int(*w);
        let right = self.pop_int(*w);
        let int = op(left, right);
        self.push_int(int);
    }

    fn pop_ptr(&mut self) -> *mut c_void {
        self.pop_usize() as _
    }

    fn pop_u8(&mut self) -> u8 {
        self.stack.pop().unwrap()
    }

    fn pop_u16(&mut self) -> u16 {
        let hi = self.stack.pop().unwrap();
        let lo = self.stack.pop().unwrap();
        u16::from_le_bytes([lo, hi])
    }

    fn pop_u32(&mut self) -> u32 {
        let b4 = self.stack.pop().unwrap();
        let b3 = self.stack.pop().unwrap();
        let b2 = self.stack.pop().unwrap();
        let b1 = self.stack.pop().unwrap();
        u32::from_le_bytes([b1, b2, b3, b4])
    }

    fn pop_u64(&mut self) -> u64 {
        let b8 = self.stack.pop().unwrap();
        let b7 = self.stack.pop().unwrap();
        let b6 = self.stack.pop().unwrap();
        let b5 = self.stack.pop().unwrap();
        let b4 = self.stack.pop().unwrap();
        let b3 = self.stack.pop().unwrap();
        let b2 = self.stack.pop().unwrap();
        let b1 = self.stack.pop().unwrap();
        u64::from_le_bytes([b1, b2, b3, b4, b5, b6, b7, b8])
    }

    fn pop_usize(&mut self) -> usize {
        if std::mem::size_of::<usize>() != 8 { panic!("32-bit not supported"); }

        self.pop_u64() as usize
    }

    fn pop_int(&mut self, w: Width) -> Int {
        match w {
            Width::W8 => self.pop_u8().into(),
            Width::W16 => self.pop_u16().into(),
            Width::W32 => self.pop_u32().into(),
            Width::W64 => self.pop_u64().into(),
        }
    }

    fn pop_f32(&mut self) -> f32 {
        unsafe { std::mem::transmute(self.pop_u32()) }
    }

    fn pop_f64(&mut self) -> f64 {
        unsafe { std::mem::transmute(self.pop_u64()) }
    }

    fn push_u8(&mut self, val: u8) {
        self.stack.push(val);
    }

    fn push_u16(&mut self, val: u16) {
        self.stack.extend(&val.to_le_bytes());
    }

    fn push_u32(&mut self, val: u32) {
        self.stack.extend(&val.to_le_bytes());
    }

    fn push_u64(&mut self, val: u64) {
        self.stack.extend(&val.to_le_bytes());
    }

    fn push_f32(&mut self, val: f32) {
        let val: u32 = unsafe { std::mem::transmute(val) };
        self.stack.extend(&val.to_le_bytes());
    }

    fn push_f64(&mut self, val: f64) {
        let val: u64 = unsafe { std::mem::transmute(val) };
        self.stack.extend(&val.to_le_bytes());
    }

    fn push_int(&mut self, int: Int) {
        match int {
            Int::W8(val) => self.push_u8(val),
            Int::W16(val) => self.push_u16(val),
            Int::W32(val) => self.push_u32(val),
            Int::W64(val) => self.push_u64(val),
        }
    }
}

trait PtrToBits {
    fn to_usize(self) -> usize;
}

impl<T> PtrToBits for *const T {
    fn to_usize(self) -> usize {
        self as usize
    }
}

impl<T> PtrToBits for *mut T {
    fn to_usize(self) -> usize {
        self as usize
    }
}