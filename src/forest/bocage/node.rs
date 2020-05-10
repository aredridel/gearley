use std::cell::Cell;
use std::hint;

use cfg::symbol::Symbol;

pub use self::Node::*;
use self::Tag::*;
use forest::node_handle::{NodeHandle, NULL_HANDLE};

// Node variants `Sum`/`Product` are better known in literature as `OR`/`AND`.
#[derive(Copy, Clone, Debug)]
pub enum Node {
    Product {
        /// 12+ bytes.
        action: u32,
        left_factor: NodeHandle,
        right_factor: Option<NodeHandle>,
        first: bool,
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

#[derive(Copy, Clone, Eq, PartialEq)]
enum Tag {
    ProductTagFirst = 0b00 << TAG_BIT,
    ProductTag = 0b01 << TAG_BIT,
    LeafTag = 0b10 << TAG_BIT,
}

impl Tag {
    #[inline]
    fn from_u32(n: u32) -> Option<Self> {
        let n = n & TAG_MASK;
        if n == LeafTag.to_u32() {
            Some(LeafTag)
        } else if n == ProductTagFirst.to_u32() {
            Some(ProductTagFirst)
        } else if n == ProductTag.to_u32() {
            Some(ProductTag)
        } else {
            None
        }
    }

    #[inline]
    fn to_u32(&self) -> u32 {
        match *self {
            ProductTagFirst => 0b00 << TAG_BIT,
            ProductTag => 0b01 << TAG_BIT,
            LeafTag => 0b10 << TAG_BIT,
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
                ..
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
            &Product { first, .. } => {
                if first {
                    ProductTagFirst
                } else {
                    ProductTag
                }
            }
            NullingLeaf { .. } | Evaluated { .. } => LeafTag,
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
                LeafTag => {
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
                ProductTagFirst => Product {
                    action: fields[0].action,
                    left_factor: fields[1].factor,
                    right_factor: fields[2].factor.to_option(),
                    first: true,
                },
                ProductTag => Product {
                    action: fields[0].action,
                    left_factor: fields[1].factor,
                    right_factor: fields[2].factor.to_option(),
                    first: false,
                },
            }
        }
    }

    #[inline]
    pub(super) fn is_consecutive_product(&self) -> bool {
        unsafe {
            let fields = self.cell.get();
            let tag = get_tag(&fields);
            tag == ProductTag
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
    fields[0].tag |= tag.to_u32();
}

#[inline]
unsafe fn get_and_erase_tag(fields: &mut [CompactField; 3]) -> Tag {
    let &mut CompactField { ref mut tag } = &mut fields[0];
    let extract_tag = *tag;
    *tag = *tag & !TAG_MASK;
    unwrap_unchecked(Tag::from_u32(extract_tag))
}

#[inline]
unsafe fn get_tag(fields: &[CompactField; 3]) -> Tag {
    let CompactField { tag } = fields[0];
    unwrap_unchecked(Tag::from_u32(tag))
}
