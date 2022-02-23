#[derive(Clone, Debug)]
pub enum Type {
    Struct(Box<Struct>),
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
    Array(Box<Array>),
}

impl Type {
    pub fn size(&self) -> usize {
        match self {
            Type::Struct(tstruct) => tstruct.size,
            Type::U8 | Type::I8 => 1,
            Type::U16 | Type::I16 => 2,
            Type::U32 | Type::I32 | Type::F32 => 4,
            Type::U64 | Type::I64 | Type::F64 => 8,
            Type::Array(array) => array.ty.size() * array.len,
        }
    }

    pub fn align(&self) -> usize {
        match self {
            Type::Struct(tstruct) => tstruct.align,
            Type::U8 | Type::U16 | Type::U32 | Type::U64
                | Type::I8 | Type::I16 | Type::I32 | Type::I64
                | Type::F32 | Type::F64
                => self.size(),
            Type::Array(array) => array.ty.align(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Struct {
    pub name: &'static str,
    pub fields: Vec<(&'static str, Field)>,
    pub size: usize,
    pub align: usize,
}

#[derive(Clone, Debug)]
pub struct Field {
    pub ty: Type,
    pub offset: usize,
}

#[derive(Clone, Debug)]
pub struct Array {
    pub ty: Type,
    pub len: usize,
}