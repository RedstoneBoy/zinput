pub struct Module {
    pub inputs: Vec<Body>,
}

pub type VarIndex = u8;
pub type PtrOffset = i8;

pub struct Body {
    pub block: Block,
    pub max_var_index: VarIndex,
}

pub struct Block(pub Vec<Instruction>);

pub enum Instruction {
    /// Push constant on to stack
    Push8(u8),
    /// Push constant on to stack
    Push16(u16),
    /// Push constant on to stack
    Push32(u32),
    /// Push constant on to stack
    Push64(u64),

    /// Discards value from stack
    Pop(Width),

    /// Pushes value from var index on to stack
    VarGet(Width, VarIndex),

    /// Pops value from stack and stores it at VarIndex
    VarPut(Width, VarIndex),

    /// Pops pointer from stack, adds constant to pointer, and loads value at pointer on to stack
    Load(Width, PtrOffset),

    /// Pops pointer from stack, then value from stack, and stores value at pointer
    Store(Width, PtrOffset),

    // Booleans

    /// Not the boolean on the stack
    BoolNot,
    /// Or two booleans on the stack
    BoolOr,
    /// And two booleans on the stack
    BoolAnd,
    
    // Integers and bits

    /// Negate signed integer on stack
    Neg(Width),
    Not(Width),

    Or(Width),
    And(Width),
    Xor(Width),

    /// Pops left, than right from stack, and pushes result to stack
    Add(Width),
    /// Pops left, than right from stack, and pushes result to stack
    Sub(Width),
    /// Pops left, than right from stack, and pushes result to stack
    Mul {
        width: Width,
        signed: bool,
    },
    /// Pops left, than right from stack, and pushes result to stack
    Div {
        width: Width,
        signed: bool,
    },

    /// Pop byte from stack, shifts <width> integer on stack by byte
    ShiftLeft(Width),
    /// Pop byte from stack, shifts <width> integer on stack by byte
    ShiftRight(Width),

    /// Pops left, than right from stack, and pushes result to stack
    IntCompare(Width, Cmp),

    // Floats

    FloatNeg(Float),
    FloatAdd(Float),
    FloatSub(Float),
    FloatMul(Float),
    FloatDiv(Float),
    FloatCompare(Float, Cmp),

    // Conversions

    /// Signed/Zero extend integer on stack
    Extend {
        from: Width,
        to: Width,
        signed: bool,
    },

    F32To64,
    F64To32,

    /// Naturally convert an integer to a float
    IntToFloat {
        width: Width,
        signed: bool,
        float: Float,
    },
    /// Convert a float to an integer with floor and clamping
    FloatToInt {
        width: Width,
        signed: bool,
        float: Float,
    },

    // Branching

    If {
        yes: Block,
        no: Block,
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Width {
    W8,
    W16,
    W32,
    W64,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Float {
    F32,
    F64,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Cmp {
    Eq,
    Neq,
    Greater,
    GreaterEq,
    Less,
    LessEq,
}