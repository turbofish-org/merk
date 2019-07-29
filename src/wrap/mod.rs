pub trait Wrap<T> {
    fn inner(&self) -> &T;
    fn inner_mut(&mut self) -> &mut T;
    fn unwrap(self) -> T;
}

// TODO: derive macro (pass in field name)

#[cfg(test)]
mod test {
    use super::*;

    struct Foo { n: usize }
    impl Foo {
        fn increment(&mut self) { self.n += 1; }
        fn unwrap(self) -> usize { self.n }
    }

    struct Wrapper { foo: Foo }
    impl Wrap<Foo> for Wrapper {
        fn inner(&self) -> &Foo { &self.foo }
        fn inner_mut(&mut self) -> &mut Foo { &mut self.foo }
        fn unwrap(self) -> Foo { self.foo }
    }

    #[test]
    fn simple_wrap() {
        let mut w = Wrapper {
            foo: Foo { n: 123 }
        };
        assert_eq!(w.inner().n, 123);
        w.inner_mut().increment();
        assert_eq!(w.inner().n, 124);
        assert_eq!(w.unwrap().n, 124);
    }

    #[test]
    fn wrap_trait_methods() {
        pub trait Inc {
            fn increment(&mut self);
        }
        impl<T: Wrap<Foo>> Inc for T {
            fn increment(&mut self) {
                self.inner_mut().increment();
            }
        }

        let mut w = Wrapper {
            foo: Foo { n: 123 }
        };
        w.increment();
        assert_eq!(w.inner().n, 124);
    }
}
