use std::cell::Cell;
use std::hint;

use cfg::symbol::Symbol;

pub use self::Node::*;
use crate::forest::node_handle::{NodeHandle, NULL_HANDLE};

// Node variants `Sum`/`Product` are better known in literature as `OR`/`AND`.
#[derive(Copy, Clone, Debug)]
pub enum Node {
    Sum {
        /// 8 bytes.
        /// Invariant: count > 1.
        /// Invariant: This node can only be directly followed by `Product`.
        nonterminal: Symbol,
        count: u32,
    },
    Product {
        /// 12+ bytes.
        action: u32,
        left_factor: NodeHandle,
        right_factor: Option<NodeHandle>,
    },
    NullingLeaf {
        /// 4 bytes.
        symbol: Symbol,
    },
    Evaluated {
        /// 8 bytes.
        symbol: Symbol,
        values: u32,
    },
}

#[derive(Clone)]
pub struct CompactNode {
    cell: Cell<[CompactField; 3]>,
}

// Node variants `Sum`/`Product` are better known in literature as `OR`/`AND`.
#[derive(Copy, Clone)]
union CompactField {
    // sum
    nonterminal: Symbol,
    count: u32,

    // product
    action: u32,
    factor: NodeHandle,
    // right_factor: NodeHandle,

    // leaf
    symbol: Symbol,
    values: u32,

    // tag
    tag: u32,
}

#[derive(Copy, Clone)]
#[allow(clippy::enum_clike_unportable_variant)]
enum Tag {
    Leaf = 0b00 << TAG_BIT,
    Sum = 0b01 << TAG_BIT,
    Product = 0b10 << TAG_BIT,
}

impl Tag {
    #[inline]
    fn from_u32(n: u32) -> Option<Self> {
        let n = n & TAG_MASK;
        if n == Tag::Leaf.as_u32() {
            Some(Tag::Leaf)
        } else if n == Tag::Sum.as_u32() {
            Some(Tag::Sum)
        } else if n == Tag::Product.as_u32() {
            Some(Tag::Product)
        } else {
            None
        }
    }

    #[inline]
    fn as_u32(&self) -> u32 {
        match *self {
            Tag::Leaf => 0b00 << TAG_BIT,
            Tag::Sum => 0b01 << TAG_BIT,
            Tag::Product => 0b10 << TAG_BIT,
        }
    }
}

const TAG_BIT: usize = 30;
const TAG_MASK: u32 = 0b11 << TAG_BIT;
const NULL_VALUES: u32 = 0xFFFF_FFFF;
pub(super) const NULL_ACTION: u32 = !TAG_MASK;

impl Node {
    #[inline]
    pub(super) fn compact(self) -> CompactNode {
        let mut fields = match self {
            Product {
                left_factor,
                right_factor,
                action,
            } => {
                let right_factor = right_factor.unwrap_or(NULL_HANDLE);
                [
                    CompactField { action },
                    CompactField {
                        factor: left_factor,
                    },
                    CompactField {
                        factor: right_factor,
                    },
                ]
            }
            Sum { nonterminal, count } => [
                CompactField { nonterminal },
                CompactField { count },
                CompactField { tag: 0 },
            ],
            NullingLeaf { symbol } => [
                CompactField { symbol },
                CompactField {
                    values: NULL_VALUES,
                },
                CompactField { tag: 0 },
            ],
            Evaluated { symbol, values } => [
                CompactField { symbol },
                CompactField { values },
                CompactField { tag: 0 },
            ],
        };
        unsafe {
            set_tag(&mut fields, self.tag());
        }
        CompactNode {
            cell: Cell::new(fields),
        }
    }

    #[inline]
    fn tag(&self) -> Tag {
        match self {
            Product { .. } => Tag::Product,
            Sum { .. } => Tag::Sum,
            NullingLeaf { .. } | Evaluated { .. } => Tag::Leaf,
        }
    }
}

impl CompactNode {
    #[inline]
    pub(super) fn set(&self, node: Node) {
        self.cell.set(node.compact().cell.get());
    }

    #[inline]
    pub(super) fn expand(&self) -> Node {
        let mut fields = self.cell.get();
        unsafe {
            let tag = get_and_erase_tag(&mut fields);
            match tag {
                Tag::Leaf => {
                    if fields[1].values == NULL_VALUES {
                        NullingLeaf {
                            symbol: fields[0].symbol,
                        }
                    } else {
                        Evaluated {
                            symbol: fields[0].symbol,
                            values: fields[1].values,
                        }
                    }
                }
                Tag::Product => Product {
                    action: fields[0].action,
                    left_factor: fields[1].factor,
                    right_factor: fields[2].factor.to_option(),
                },
                Tag::Sum => Sum {
                    nonterminal: fields[0].nonterminal,
                    count: fields[1].count,
                },
            }
        }
    }
}

#[inline]
unsafe fn unwrap_unchecked<T>(opt: Option<T>) -> T {
    match opt {
        Some(val) => val,
        None => hint::unreachable_unchecked(),
    }
}

#[inline]
unsafe fn set_tag(fields: &mut [CompactField; 3], tag: Tag) {
    fields[0].tag |= tag.as_u32();
}

#[inline]
unsafe fn get_and_erase_tag(fields: &mut [CompactField; 3]) -> Tag {
    let &mut CompactField { ref mut tag } = &mut fields[0];
    let extract_tag = *tag;
    *tag &= !TAG_MASK;
    unwrap_unchecked(Tag::from_u32(extract_tag))
}
