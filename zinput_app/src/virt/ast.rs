use std::collections::HashMap;

use zinput_engine::{
    device::{component::ComponentKind, Device, DeviceMut, DeviceInfo, components},
    DeviceAlreadyExists, DeviceHandle, DeviceView, Engine,
};

use super::{Updater, VerificationError};

pub struct Program {
    pub name: String,
    pub updates: Vec<Block>,
    pub reqs: Vec<DeviceReq>,
    pub output: DeviceReq,
}

impl Updater for Program {
    fn verify(&self, info: &[DeviceView]) -> Result<(), VerificationError> {
        if self.reqs.len() != info.len() {
            return Err(VerificationError::InvalidDeviceAmount {
                need: self.reqs.len(),
                got: info.len(),
            });
        }

        for i in 0..self.reqs.len() {
            self.reqs[i].verify(i, info[i].info())?;
        }

        Ok(())
    }

    fn create_output(&self, engine: &Engine) -> Result<DeviceHandle, DeviceAlreadyExists> {
        macro_rules! set_info_fn {
            ($($cname:ident : $ckind:expr),* $(,)?) => {
                paste::paste! {
                    fn set_info(info: &mut DeviceInfo, req: &DeviceReq) {
                        $({
                            let comp_size = req.components.get(&$ckind)
                                .copied()
                                .unwrap_or(0);
                            
                            info.[< $cname s >] = vec![Default::default(); comp_size];
                        })*
                    }
                }
            };
        }

        components!(kind set_info_fn);

        let mut info = DeviceInfo::new(self.name.clone());
        info.id = Some(format!("virtual/{}", self.name));
        info.autoload_config = true;
        set_info(&mut info, &self.output);

        engine.new_device(info)
    }

    fn update(&self, view: &DeviceView, view_index: usize, out: &DeviceHandle) {
        self.updates[view_index].update(view, out);
    }
}

pub struct DeviceReq {
    components: HashMap<ComponentKind, usize>,
}

impl DeviceReq {
    fn verify(&self, index: usize, info: &DeviceInfo) -> Result<(), VerificationError> {
        macro_rules! verify_comps {
            ($($cname:ident : $ckind:expr),* $(,)?) => {
                paste::paste! {
                    $({
                        let comp_size = self.components.get(&$ckind)
                            .copied()
                            .unwrap_or(0);
                        
                        let info_len = info.[< $cname s >].len();
                        if comp_size != info_len {
                            return Err(VerificationError::InvalidComponentAmount {
                                device_index: index,
                                kind: $ckind,
                                need: comp_size,
                                got: info_len,
                            });
                        }
                    })*
                }
            };
        }

        components!(kind verify_comps);

        Ok(())
    }
}

pub struct Block {
    pub stmts: Vec<Stmt>,
}

impl Block {
    fn update(&self, view: &DeviceView, out: &DeviceHandle) {
        let device_in = view.device();
        out.update(|mut device_out| {
            for stmt in &self.stmts {
                stmt.execute(&device_in, &mut device_out);
            }
        });
    }
}

pub enum Stmt {
    AssignBoolean {
        setter: fn(&mut DeviceMut, value: bool),
        value: BoolExpr,
    },
    AssignNumber {
        setter: fn(&mut DeviceMut, value: f32),
        value: NumberExpr,
    },
}

impl Stmt {
    fn execute(&self, device_in: &Device, device_out: &mut DeviceMut) {
        match self {
            Stmt::AssignBoolean { setter, value } => {
                let value = value.eval(device_in);
                setter(device_out, value);
            }
            Stmt::AssignNumber { setter, value } => {
                let value = value.eval(device_in);
                setter(device_out, value);
            }
        }
    }
}

pub enum BoolExpr {
    True,
    False,
    Not(Box<BoolExpr>),
    Or(Box<BoolExpr>, Box<BoolExpr>),
    And(Box<BoolExpr>, Box<BoolExpr>),
    Get {
        getter: fn(&Device) -> bool,
    },
    Compare(NumberExpr, CmpOp, NumberExpr),
    Branch {
        cond: Box<BoolExpr>,
        yes: Box<BoolExpr>,
        no: Box<BoolExpr>,
    },
}

impl BoolExpr {
    fn eval(&self, device: &Device) -> bool {
        match self {
            BoolExpr::True => true,
            BoolExpr::False => false,
            BoolExpr::Not(expr) => !expr.eval(device),
            BoolExpr::Or(l, r) => l.eval(device) || r.eval(device),
            BoolExpr::And(l, r) => l.eval(device) || r.eval(device),
            BoolExpr::Get { getter } => getter(device),
            BoolExpr::Compare(l, op, r) => op.apply(l.eval(device), r.eval(device)),
            BoolExpr::Branch { cond, yes, no } => {
                if cond.eval(device) {
                    yes.eval(device)
                } else {
                    no.eval(device)
                }
            }
        }
    }
}

pub enum NumberExpr {
    Literal(f32),
    Clamp {
        value: Box<NumberExpr>,
        min: Box<NumberExpr>,
        max: Box<NumberExpr>,
    },
    Get {
        getter: fn(&Device) -> f32,
    },
    BinOp(Box<NumberExpr>, BinOp, Box<NumberExpr>),
    Branch {
        cond: Box<BoolExpr>,
        yes: Box<NumberExpr>,
        no: Box<NumberExpr>,
    },
}

impl NumberExpr {
    fn eval(&self, device: &Device) -> f32 {
        match self {
            NumberExpr::Literal(value) => *value,
            NumberExpr::Clamp { value, min, max } => {
                let min = min.eval(device);
                let max = max.eval(device);
                let value = value.eval(device);
                if min >= max {
                    value
                } else {
                    value.clamp(min, max)
                }
            }
            NumberExpr::Get { getter } => getter(device),
            NumberExpr::BinOp(l, op, r) => op.apply(l.eval(device), r.eval(device)),
            NumberExpr::Branch { cond, yes, no } => {
                if cond.eval(device) {
                    yes.eval(device)
                } else {
                    no.eval(device)
                }
            }
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Modulo,
}

impl BinOp {
    fn apply(&self, l: f32, r: f32) -> f32 {
        match self {
            BinOp::Add => l + r,
            BinOp::Sub => l - r,
            BinOp::Mul => l * r,
            BinOp::Div => l / r,
            BinOp::Modulo => l % r,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum CmpOp {
    Greater,
    GreaterEq,
    Less,
    LessEq,
    Equal,
    NotEqual,
}

impl CmpOp {
    fn apply(&self, l: f32, r: f32) -> bool {
        match self {
            CmpOp::Greater => l > r,
            CmpOp::GreaterEq => l >= r,
            CmpOp::Less => l < r,
            CmpOp::LessEq => l <= r,
            CmpOp::Equal => l == r,
            CmpOp::NotEqual => l != r,
        }
    }
}
