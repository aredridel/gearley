use cfg_symbol::{Symbol, Symbolic};

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct NodeHandle(pub u32);

pub const NULL_HANDLE: NodeHandle = NodeHandle(0xFFFF_FFFF);

impl NodeHandle {
    #[inline]
    pub fn nulling(symbol: Symbol) -> Self {
        NodeHandle(symbol.usize() as u32)
    }

    #[inline]
    pub fn usize(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub fn to_option(self) -> Option<NodeHandle> {
        if self == NULL_HANDLE {
            None
        } else {
            Some(self)
        }
    }
}
