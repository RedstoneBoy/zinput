use std::collections::HashMap;

use crate::util::{Signed, Width};

#[derive(Clone, Debug, PartialEq)]
pub enum Type {
    Reference(Box<Type>),
    Int(Width, Signed),
    F32,
    F64,
    Bool,
    Slice(Box<Type>),
    Bitfield(&'static str, Width, BitNames),
    Struct(Struct),
}

impl Type {
    pub fn stack_size(&self) -> usize {
        match self {
            Type::Reference(_) => std::mem::size_of::<usize>(),
            Type::Int(w, _) => w.size(),
            Type::F32 => 4,
            Type::F64 => 8,
            Type::Bool => 1,
            Type::Slice(_) => std::mem::size_of::<usize>() * 2,
            Type::Bitfield(_, w, _) => w.size(),
            Type::Struct(_) => std::mem::size_of::<usize>(),
        }
    }

    pub fn is_num(&self) -> bool {
        match self {
            Type::Int(_, _) => true,
            Type::F32 => true,
            Type::F64 => true,
            _ => false,
        }
    }

    pub fn width(&self) -> Option<Width> {
        match self {
            Type::Int(width, _) => Some(*width),
            Type::F32 => Some(Width::W32),
            Type::F64 => Some(Width::W64),
            Type::Bool => Some(Width::W8),
            Type::Bitfield(_, width, _) => Some(*width),
            _ => None,
        }
    }

    pub fn dereferenced(self) -> Self {
        let mut this = self;

        while let Type::Reference(ty) = this {
            this = *ty;
        }

        this
    }

    pub fn assignable_from(&self, from: &Type) -> bool {
        match self {
            Type::Reference(_) => false,
            Type::Int(width, signed) => match from {
                Type::Int(owidth, Signed::No) => {
                    if signed == &Signed::Yes {
                        owidth < width
                    } else {
                        owidth <= width
                    }
                }
                Type::Int(owidth, Signed::Yes) => signed == &Signed::Yes && owidth <= width,
                Type::Bool => true,
                Type::Bitfield(_, owidth, _) => owidth <= width,
                _ => false,
            },
            Type::F32 => {
                matches!(from, Type::F32 | Type::F64)
                    || matches!(from, Type::Int(width, _) if width <= &Width::W32)
            }
            Type::F64 => matches!(from, Type::F32 | Type::F64 | Type::Int(_, _)),
            Type::Bool => matches!(from, Type::Bool),
            Type::Slice(inner) => matches!(from, Type::Slice(oinner) if inner == oinner),
            Type::Bitfield(_, width, _) => {
                matches!(from, Type::Int(owidth, _) | Type::Bitfield(_, owidth, _) if owidth == width)
            }
            Type::Struct(s) => matches!(from, Type::Struct(os) if s == os),
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Reference(ty) => write!(f, "{ty}"),
            Type::Int(w, s) => {
                match s {
                    Signed::No => write!(f, "u")?,
                    Signed::Yes => write!(f, "i")?,
                }

                match w {
                    Width::W8 => write!(f, "8")?,
                    Width::W16 => write!(f, "16")?,
                    Width::W32 => write!(f, "32")?,
                    Width::W64 => write!(f, "64")?,
                }

                Ok(())
            }
            Type::F32 => write!(f, "f32"),
            Type::F64 => write!(f, "f64"),
            Type::Bool => write!(f, "bool"),
            Type::Slice(ty) => write!(f, "&[{ty}]"),
            Type::Bitfield(name, w, _) => {
                write!(f, "bitfield(u")?;
                match w {
                    Width::W8 => write!(f, "8")?,
                    Width::W16 => write!(f, "16")?,
                    Width::W32 => write!(f, "32")?,
                    Width::W64 => write!(f, "64")?,
                }
                write!(f, ") {name}")
            }
            Type::Struct(s) => write!(f, "{}", s.name),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BitNames(pub HashMap<&'static str, u8>);

#[derive(Clone, Debug, PartialEq)]
pub struct Struct {
    pub name: &'static str,
    pub fields: HashMap<&'static str, Field>,
    pub size: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub ty: Type,
    pub byte_offset: usize,
}

pub trait ToType {
    fn to_type() -> Type;
}

macro_rules! impl_to_type {
    ($($typ:ty = $e:expr;)*) => {
        $(impl ToType for $typ {
            fn to_type() -> Type {
                $e
            }
        })*
    }
}

impl_to_type! {
    u8  = Type::Int(Width::W8,  Signed::No);
    u16 = Type::Int(Width::W16, Signed::No);
    u32 = Type::Int(Width::W32, Signed::No);
    u64 = Type::Int(Width::W64, Signed::No);
    i8  = Type::Int(Width::W8,  Signed::Yes);
    i16 = Type::Int(Width::W16, Signed::Yes);
    i32 = Type::Int(Width::W32, Signed::Yes);
    i64 = Type::Int(Width::W64, Signed::Yes);
    f32 = Type::F32;
    f64 = Type::F64;
    bool = Type::Bool;
}

#[macro_export]
macro_rules! to_struct {
    ( name = $name:ident; $( $offset:literal : $fname:ident : $typ:ty ;)* ) => {{
        let mut fields = std::collections::HashMap::new();

        $({
            fields.insert(stringify!($fname), $crate::ty::Field {
                ty: <$typ as $crate::ty::ToType>::to_type(),
                byte_offset: $offset,
            });
        })*

        $crate::ty::Type::Struct($crate::ty::Struct {
            name: stringify!($name),
            fields,
            size: std::mem::size_of::<$name>(),
        })
    }};
}

#[macro_export]
macro_rules! to_bitfield {
    ( name = $name:ident; size = $size:expr; $( $bname:ident = $bit:literal ;)* ) => {{
        let mut names = std::collections::HashMap::new();

        $(
            names.insert(stringify!($bname), $bit);
        )*

        $crate::ty::Type::Bitfield(stringify!($name), $size, $crate::ty::BitNames(names))
    }};
}
