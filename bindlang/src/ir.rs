use zinput_device::component::ComponentKind;

/// Stack operations are defined to use the top-most value on the stack as source or primary, and the below value as destination or secondary
pub enum Instruction {
    /// Set the current working component to the index (primary), component (secondary) and device (tertiary) on the stack
    SetComponent,
    /// Copy component between top two components on the stack
    CopyComponent,
    /// Read a component field onto the value stack from the current working component
    ReadField {
        offset: u16,
        size: u16,
        align: u8,
    },
    /// Write a stack value to a component field on the current working component
    WriteField {
        offset: u16,
        size: u16,
        align: u8,
    },

    PushValue(Value),
    Pop {
        size: u16,
    },
    Clone {
        size: u16,
    },
    
    // math ops

    /// Negate the current number on the stack
    Negate(NumType),
    /// Use a binary operator on the top two numbers on the stack
    MathBinary(MathBinOp, NumType),
    /// Use a bit operator on the number and the u8 on the stack
    MathBit(MathBitOp, NumType),
    /// Use a comparison operator on the top two numbers on the stack
    MathCmp(MathCmpOp, NumType),
    /// Convert a number naturally, saturating to minimums and maximums
    ConvertNumber {
        from: NumType,
        to: NumType,
    },

    // bool ops

    /// Flip the current boolean on the stack
    Not,

    // branching

    /// Skip `n` instructions if boolean on stack is true
    If(isize),
    
    // bit manipulation

    /// Reinterpret a number
    /// unsigned -> unsigned, shorten with 'bit and', extend with zeroes
    /// signed -> signed, shorten with sign, extend with sign
    /// unsigned[n] <-> signed[n], reintepret as twos complement
    /// unsigned[a] -> signed[b], first reinterpret unsigned[a] as unsigned[b] as above, then reinterpret as above
    /// signed[a] -> unsigned[b], first reinterpret signed[a] as signed[b] as above, then reinterpret as above
    /// int[a] -> float[b], first reinterpret int[a] as int[b] as above, then reinterpet the bits as a float
    /// float[a] -> int[b], first reinterpret the bits of float[a] as int[a], then reinterpret int[a] as int[b] as above
    /// float[a] -> float[b], natural conversion
    ReinterpretNumber {
        from: NumType,
        to: NumType,
    },
    /// Compare two sets of bytes on the stack, pushing a boolean
    BitEquals {
        size: u16,
        align: u8,
    },
}

pub enum Value {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Device(usize),
    Component(ComponentKind),
}

pub enum MathCmpOp {
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
    Equals,
}

pub enum MathBitOp {
    ShiftLeft,
    ShiftRight,
}

pub enum MathBinOp {
    Add,
    Sub,
    Mul,
    Div,

    And,
    Or,
    Xor,
}

pub enum NumType {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
}