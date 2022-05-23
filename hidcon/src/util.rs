macro_rules! buttons {
    ($name:ident, $ename:ident : $bty:ty => $($but:ident = $bit:expr),* $(,)?) => {
        #[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
        pub enum $ename {
            $($but,)*
        }

        impl $ename {
            fn bit(self) -> $bty {
                match self {
                    $($ename::$but => $bit,)*
                }
            }
        }

        #[derive(Copy, Clone, Debug, Default, Hash, PartialEq, Eq)]
        pub struct $name($bty);

        impl $name {
            #[inline(always)]
            pub fn is_pressed(self, button: $ename) -> bool {
                (self.0 >> button.bit()) & 0b1 != 0
            }

            // pub fn set_pressed(&mut self, button: $ename, pressed: bool) {
            //     if pressed {
            //         self.0 = self.0 | (1 << button.bit());
            //     } else {
            //         self.0 = self.0 & !(1 << button.bit());
            //     }
            // }
        }
    }
}

pub(crate) use buttons;
