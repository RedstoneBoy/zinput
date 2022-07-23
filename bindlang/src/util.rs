#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Width {
    W8,
    W16,
    W32,
    W64,
}

impl Width {
    #[allow(non_upper_case_globals)]
    pub const WSize: Width = Int::WSize(0).width();

    pub fn size(&self) -> usize {
        match self {
            Width::W8 => 1,
            Width::W16 => 2,
            Width::W32 => 4,
            Width::W64 => 8,
        }
    }

    pub fn int(&self, val: u8) -> Int {
        match self {
            Width::W8 => Int::W8(val),
            Width::W16 => Int::W16(val as _),
            Width::W32 => Int::W32(val as _),
            Width::W64 => Int::W64(val as _),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Signed {
    Yes,
    No,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Int {
    W8(u8),
    W16(u16),
    W32(u32),
    W64(u64),
}

impl Int {
    #[allow(non_snake_case)]
    pub const fn WSize(int: usize) -> Self {
        match std::mem::size_of::<usize>() {
            1 => Int::W8(int as _),
            2 => Int::W16(int as _),
            4 => Int::W32(int as _),
            8 => Int::W64(int as _),
            _ => panic!("ICE: unsupported pointer size"),
        }
    }

    pub const fn width(&self) -> Width {
        match self {
            Int::W8(_) => Width::W8,
            Int::W16(_) => Width::W16,
            Int::W32(_) => Width::W32,
            Int::W64(_) => Width::W64,
        }
    }
}

macro_rules! impl_from_int {
    ( $( $typ:ty = $int:ident ;)* ) => {
        $(
            impl From<$typ> for Int {
                fn from(v: $typ) -> Int {
                    Int::$int(v as _)
                }
            }
        )*
    }
}

impl_from_int! {
    bool = W8;
    u8 = W8;
    u16 = W16;
    u32 = W32;
    u64 = W64;
    i8 = W8;
    i16 = W16;
    i32 = W32;
    i64 = W64;
    usize = WSize;
    isize = WSize;
}

impl std::fmt::Display for Int {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Int::W8(val) => write!(f, "{val}"),
            Int::W16(val) => write!(f, "{val}"),
            Int::W32(val) => write!(f, "{val}"),
            Int::W64(val) => write!(f, "{val}"),
        }
    }
}

