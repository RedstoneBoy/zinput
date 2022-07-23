use std::{ffi::c_void, collections::HashMap};

use crate::{ir::{Instruction as Ins, Module, Body, Block}, util::Int};


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


    pub fn run(&mut self, module: &Module, dev_in_id: usize, dev_out: *mut c_void, dev_ins: &[*mut c_void]) -> u32 {
        self.stack.clear();
        self.var_space.clear();
        self.var_map.clear();

        self.stack.extend_from_slice(&dev_out.to_usize().to_le_bytes());
        for dev_in in dev_ins {
            self.stack.extend_from_slice(&dev_in.to_usize().to_le_bytes());
        }

        self.run_body(&module.inputs[dev_in_id])        
    }

    fn run_body(&mut self, body: &Body) -> u32 {
        for var_id in 0..body.num_vars {
            let var_addr = self.var_space.len();
            let var_size = body.var_sizes[&var_id];

            self.var_map.insert(var_id, var_addr);
            self.var_space.extend(std::iter::repeat(0).take(var_size));
        }

        match self.run_block(&body.block) {
            Ok(()) => 0,
            Err(err) => err,
        }
    }

    fn run_block(&mut self, block: &Block) -> Result<(), u32> {
        for instr in &block.0 {
            self.run_instr(instr)?;
        }

        Ok(())
    }

    fn run_instr(&mut self, instr: &Ins) -> Result<(), u32> {
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
            _ => todo!(),
        }

        Ok(())
    }

    fn pop_ptr(&mut self) -> *mut c_void {
        self.pop_usize() as _
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