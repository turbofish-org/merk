#![macro_use]

#[macro_export]
macro_rules! deref {
    ($outer:ty, $inner:ty, $name:ident) => {
        impl Deref for $outer {
            type Target = $inner;

            fn deref(&self) -> &$inner {
                &self.$name
            }
        }

        impl DerefMut for $outer {
            fn deref_mut(&mut self) -> &mut $inner {
                &mut self.$name
            }
        }
    };
}
