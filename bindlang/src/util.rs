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

    pub(crate) fn size(&self) -> u8 {
        match self {
            Width::W8 => 1,
            Width::W16 => 2,
            Width::W32 => 4,
            Width::W64 => 8,
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

impl Into<u64> for Int {
    fn into(self) -> u64 {
        match self {
            Int::W8(v) => v.into(),
            Int::W16(v) => v.into(),
            Int::W32(v) => v.into(),
            Int::W64(v) => v.into(),
        }
    }
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