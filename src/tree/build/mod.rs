use super::walk::unreachable_got_none;

pub trait Build {
    fn detach(self, left: bool) -> (Self, Option<Self>);

    fn detach_expect(self, left: bool) -> (Self, Self) {
        self.detach(left).unwrap_or_else(
            unreachable_got_none(left)
        )
    }

    fn attach(self, left: bool, child: Option<Self>) -> Self;
}