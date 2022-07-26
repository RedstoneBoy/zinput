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

    pub fn int_truncate(&self, val: u64) -> Int {
        match self {
            Width::W8 => Int::W8(val as _),
            Width::W16 => Int::W16(val as _),
            Width::W32 => Int::W32(val as _),
            Width::W64 => Int::W64(val),
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

    pub fn from_f32(val: f32, width: Width, signed: bool) -> Self {
        match (width, signed) {
            (Width::W8, false) => (val as u8).into(),
            (Width::W16, false) => (val as u16).into(),
            (Width::W32, false) => (val as u32).into(),
            (Width::W64, false) => (val as u64).into(),
            (Width::W8, true) => (val as i8).into(),
            (Width::W16, true) => (val as i16).into(),
            (Width::W32, true) => (val as i32).into(),
            (Width::W64, true) => (val as i64).into(),
        }
    }

    pub fn from_f64(val: f64, width: Width, signed: bool) -> Self {
        match (width, signed) {
            (Width::W8, false) => (val as u8).into(),
            (Width::W16, false) => (val as u16).into(),
            (Width::W32, false) => (val as u32).into(),
            (Width::W64, false) => (val as u64).into(),
            (Width::W8, true) => (val as i8).into(),
            (Width::W16, true) => (val as i16).into(),
            (Width::W32, true) => (val as i32).into(),
            (Width::W64, true) => (val as i64).into(),
        }
    }

    pub fn to_u64_zextend(self) -> u64 {
        match self {
            Int::W8(v) => v as u64,
            Int::W16(v) => v as u64,
            Int::W32(v) => v as u64,
            Int::W64(v) => v as u64,
        }
    }
}

impl Into<u64> for Int {
    fn into(self) -> u64 {
        match self {
            Int::W8(v) => v as _,
            Int::W16(v) => v as _,
            Int::W32(v) => v as _,
            Int::W64(v) => v as _,
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

trait ToSigned<S> {
    fn to_signed(self) -> S;
}

impl ToSigned<i8> for u8 {
    fn to_signed(self) -> i8 {
        self as _
    }
}
impl ToSigned<i16> for u16 {
    fn to_signed(self) -> i16 {
        self as _
    }
}
impl ToSigned<i32> for u32 {
    fn to_signed(self) -> i32 {
        self as _
    }
}
impl ToSigned<i64> for u64 {
    fn to_signed(self) -> i64 {
        self as _
    }
}
impl ToSigned<isize> for usize {
    fn to_signed(self) -> isize {
        self as _
    }
}
