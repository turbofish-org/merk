use crate::error::Result;

pub trait Node {
    fn link_to(&mut self, left: bool, child: Option<&Self>);
    // TODO: return result?

    // TODO: method to handle detaches? (maybe with default no-op impl)
}
