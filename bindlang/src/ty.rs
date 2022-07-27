use std::collections::HashMap;

use crate::util::{Signed, Width};

#[derive(Clone, Debug, PartialEq)]
pub struct RefData(pub(crate) ());

#[derive(Clone, Debug, PartialEq)]
pub enum Type {
    Reference(Box<Type>, RefData),
    Int(Width, Signed),
    F32,
    F64,
    Bool,
    Slice(Box<Type>),
    Bitfield(&'static str, Width, BitNames),
    Struct(Struct),
}

impl Type {
    pub(crate) fn stack_size(&self) -> u8 {
        let ptr_size = std::mem::size_of::<usize>();
        assert!(ptr_size < (u8::MAX / 2) as usize, "pointer size is too large");
        let ptr_size = ptr_size as u8;

        match self {
            Type::Reference(_, _) => ptr_size,
            Type::Int(w, _) => w.size(),
            Type::F32 => 4,
            Type::F64 => 8,
            Type::Bool => 1,
            Type::Slice(_) => ptr_size * 2,
            Type::Bitfield(_, w, _) => w.size(),
            Type::Struct(_) => ptr_size,
        }
    }

    pub(crate) fn is_num(&self) -> bool {
        match self {
            Type::Int(_, _) => true,
            Type::F32 => true,
            Type::F64 => true,
            _ => false,
        }
    }

    pub(crate) fn width(&self) -> Option<Width> {
        match self {
            Type::Int(width, _) => Some(*width),
            Type::F32 => Some(Width::W32),
            Type::F64 => Some(Width::W64),
            Type::Bool => Some(Width::W8),
            Type::Bitfield(_, width, _) => Some(*width),
            _ => None,
        }
    }

    pub(crate) fn dereferenced(self) -> Self {
        let mut this = self;

        while let Type::Reference(ty, _) = this {
            this = *ty;
        }

        this
    }

    pub(crate) fn assignable_from(&self, from: &Type) -> bool {
        match self {
            Type::Reference(_, _) => false,
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
            Type::Reference(ty, _) => write!(f, "{ty}"),
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
                write!(f, "u")?;
                match w {
                    Width::W8 => write!(f, "8")?,
                    Width::W16 => write!(f, "16")?,
                    Width::W32 => write!(f, "32")?,
                    Width::W64 => write!(f, "64")?,
                }
                write!(f, "({name})")
            }
            Type::Struct(s) => write!(f, "{}", s.name),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BitNames(pub HashMap<&'static str, u8>);

/// # Safety
///
/// The struct this is representing must have a defined abi.
/// This can usually be acheived by marking the struct as #[repr(C)].
///
/// `size` must be equal to [`std::mem::size_of`] of the type being represented.
///
/// Every field must be aligned correctly.
#[derive(Clone, Debug, PartialEq)]
pub struct Struct {
    pub name: &'static str,
    pub fields: HashMap<&'static str, Field>,
    pub size: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub ty: Type,
    pub byte_offset: i32,
}

/// A trait for types that can be represented as BL types.
///
/// This function is unsafe since returning incorrect type data
/// can lead to undefined behaviour in BL.
///
/// See [`Struct`] for safety information
pub unsafe trait BLType {
    fn bl_type() -> Type;
}

macro_rules! impl_bl_type {
    ($($typ:ty = $e:expr;)*) => {
        $(unsafe impl BLType for $typ {
            fn bl_type() -> Type {
                $e
            }
        })*
    }
}

impl_bl_type! {
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
                ty: <$typ as $crate::ty::BLType>::bl_type(),
                byte_offset: $offset,
            });
        })*

        $crate::ty::Type::Struct($crate::ty::Struct {
            name: stringify!($name),
            fields,
            size: {
                let size = std::mem::size_of::<$name>();
                if size <= i32::MAX as usize {
                    size as i32
                } else {
                    panic!("Size of '{}' too large, BL struct sizes must be less than {} bytes", stringify!($name), size);
                }
            },
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
