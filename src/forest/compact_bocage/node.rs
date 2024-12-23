use std::cell::Cell;
use std::hint;

use cfg::symbol::Symbol;

pub use self::Node::*;
use crate::forest::node_handle::{NodeHandle, NULL_HANDLE};

pub struct Graph {
    pub(crate) vec: Vec<Cell<u16>>,
}

impl Graph {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Graph {
            vec: Vec::with_capacity(capacity),
        }
    }

    pub(crate) fn push(&mut self, node: Node) -> NodeHandle {
        let position = self.vec.len() as u32;
        let (node_repr, size) = node.to_repr(position);
        unsafe {
            self.vec
                .extend(node_repr.fields[..size].iter().cloned().map(Cell::new));
        }
        NodeHandle(position)
    }

    pub(crate) fn set_up(&mut self, mut handle: NodeHandle, node: Node) {
        let (node_repr, size) = node.to_repr(handle.0);
        let mut current_handle = handle;
        while current_handle.usize() < handle.usize() + size {
            let current_node = self.get(current_handle);
            self.push(current_node);
            current_handle.0 += current_node.classify(current_handle.0).size() as u32;
        }
        for i in 0..size {
            unsafe {
                self.vec[handle.usize() + i].set(node_repr.fields[i]);
            }
        }
        handle.0 += size as u32;
        while handle.0 < current_handle.0 {
            self.vec[handle.usize()].set(Tag::Nop.to_u16());
            handle.0 += 1;
        }
    }

    pub(crate) fn get(&self, handle: NodeHandle) -> Node {
        self.iter_from(handle).next().unwrap()
    }

    pub(crate) fn iter_from(&self, handle: NodeHandle) -> Iter {
        Iter {
            vec: &self.vec[..],
            handle,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Iter<'a> {
    pub(crate) vec: &'a [Cell<u16>],
    pub(crate) handle: NodeHandle,
}

impl<'a> Iterator for Iter<'a> {
    type Item = Node;

    fn next(&mut self) -> Option<Node> {
        unsafe {
            let head = if let Some(head) = self.vec.get(self.handle.usize()).cloned() {
                head.get()
            } else {
                return None;
            };
            let (tag, head) = get_and_erase_tag(head);
            if let Tag::Nop = tag {
                self.handle.0 += 1;
                self.next()
            } else {
                let mut node_repr = NodeRepr { fields: [0; 6] };
                node_repr.fields[0] = head;
                let slice = &self.vec[self.handle.usize() + 1..self.handle.usize() + tag.size()];
                for (i, val) in slice.iter().enumerate() {
                    node_repr.fields[1 + i] = val.get();
                }
                let result = node_repr.expand(tag, self.handle.0);
                self.handle.0 += tag.size() as u32;
                Some(result)
            }
        }
    }
}

impl<'a> Iter<'a> {
    #[inline]
    pub(crate) fn peek(&mut self) -> Option<Node> {
        self.clone().next()
    }
}

// Node variants `Sum`/`Product` are better known in literature as `OR`/`AND`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Node {
    Sum {
        /// 8 bytes.
        /// Invariant: count > 1.
        /// Invariant: This node can only be directly followed by `Product`.
        count: u32,
        nonterminal: Symbol,
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
        /// 4 bytes.
        symbol: Symbol,
    },
}

#[derive(Clone, Copy)]
union NodeRepr {
    fields: [u16; 6],
    small_sum: SmallSumRepr,
    small_link: SmallLinkRepr,
    medium_link: MediumLinkRepr,
    small_product: SmallProductRepr,
    small_leaf: SmallLeafRepr,
    small_nulling_leaf: SmallNullingLeafRepr,
    sum: SumRepr,
    product: ProductRepr,
    leaf: LeafRepr,
    nop: NopRepr,
}

#[derive(Clone, Copy)]
struct SmallSumRepr {
    nonterminal: u8,
    // smaller (big end position)
    count: u8,
}

#[derive(Clone, Copy)]
struct SumRepr {
    count: u32,
    nonterminal: Symbol,
}

#[derive(Clone, Copy)]
struct SmallLinkRepr {
    action: u8,
    // smaller (big end position)
    distance: u8,
}

#[derive(Clone, Copy)]
struct MediumLinkRepr {
    distance: u16,
    action: u16,
}

#[derive(Clone, Copy)]
struct SmallProductRepr {
    left_distance: u8,
    // smaller (big end position)
    right_distance: u8,
    action: u16,
}

#[derive(Clone, Copy)]
#[repr(packed)]
struct ProductRepr {
    upper_action: u16,
    lower_action: u16,
    left_factor: NodeHandle,
    right_factor: NodeHandle,
}

#[derive(Clone, Copy)]
struct SmallNullingLeafRepr {
    symbol: u16,
}

#[derive(Clone, Copy)]
struct LeafRepr {
    symbol: Symbol,
}

#[derive(Clone, Copy)]
struct SmallLeafRepr {
    symbol: u16,
}

#[derive(Clone, Copy)]
struct NopRepr {
    nop: u16,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub(super) enum Tag {
    SmallSum = 0b000 << TAG_BIT,
    SmallLink = 0b001 << TAG_BIT,
    MediumLink = 0b010 << TAG_BIT,
    SmallProduct = 0b011 << TAG_BIT,
    SmallLeaf = 0b100 << TAG_BIT,
    // SmallNonnullingLeaf = 0b1000 << (TAG_BIT - 1),
    SmallNullingLeaf = 0b1001 << (TAG_BIT - 1),
    Leaf = 0b101 << TAG_BIT,
    Sum = 0b111 << TAG_BIT,
    Product = 0b110 << TAG_BIT,
    Nop = 0b1111_1111_1111_1111,
}

impl Tag {
    #[inline]
    fn from_u16(num: u16) -> Option<Self> {
        let n = num & TAG_MASK;
        if num == Tag::Nop.to_u16() {
            Some(Tag::Nop)
        } else if n == Tag::Leaf.to_u16() {
            Some(Tag::Leaf)
        } else if n == Tag::Sum.to_u16() {
            Some(Tag::Sum)
        } else if n == Tag::Product.to_u16() {
            Some(Tag::Product)
        } else if n == Tag::SmallSum.to_u16() {
            Some(Tag::SmallSum)
        } else if n == Tag::SmallLink.to_u16() {
            Some(Tag::SmallLink)
        } else if n == Tag::MediumLink.to_u16() {
            Some(Tag::MediumLink)
        } else if n == Tag::SmallProduct.to_u16() {
            Some(Tag::SmallProduct)
        } else if n == Tag::SmallLeaf.to_u16() {
            let n = num & SMALL_LEAF_TAG_MASK;
            if n == Tag::SmallLeaf.to_u16() {
                Some(Tag::SmallLeaf)
            } else if n == Tag::SmallNullingLeaf.to_u16() {
                Some(Tag::SmallNullingLeaf)
            } else {
                None
            }
        } else {
            None
        }
    }

    #[inline]
    pub(super) fn to_u16(self) -> u16 {
        match self {
            Tag::SmallSum => 0b000 << TAG_BIT,
            Tag::SmallLink => 0b001 << TAG_BIT,
            Tag::MediumLink => 0b010 << TAG_BIT,
            Tag::SmallProduct => 0b011 << TAG_BIT,
            Tag::SmallLeaf => 0b100 << TAG_BIT,
            // SmallNonnullingLeaf = 0b1000 << (TAG_BIT - 1),
            Tag::SmallNullingLeaf => 0b1001 << (TAG_BIT - 1),
            Tag::Leaf => 0b101 << TAG_BIT,
            Tag::Sum => 0b111 << TAG_BIT,
            Tag::Product => 0b110 << TAG_BIT,
            Tag::Nop => 0b1111_1111_1111_1111,
        }
    }

    #[inline]
    fn mask(self) -> u16 {
        match self {
            Tag::SmallSum => TAG_MASK,
            Tag::SmallLink => TAG_MASK,
            Tag::MediumLink => TAG_MASK,
            Tag::SmallProduct => TAG_MASK,
            Tag::SmallLeaf => SMALL_LEAF_TAG_MASK,
            // SmallNonnullingLeaf = 0b1000 << (TAG_BIT - 1),
            Tag::SmallNullingLeaf => SMALL_LEAF_TAG_MASK,
            Tag::Leaf => TAG_MASK,
            Tag::Sum => TAG_MASK,
            Tag::Product => TAG_MASK,
            Tag::Nop => 0b1111_1111_1111_1111,
        }
    }

    #[inline]
    pub(super) fn size(self) -> usize {
        match self {
            Tag::SmallSum => 1,
            Tag::SmallLink => 1,
            Tag::MediumLink => 2,
            Tag::SmallProduct => 2,
            Tag::SmallLeaf => 1,
            Tag::SmallNullingLeaf => 1,
            Tag::Leaf => 4,
            Tag::Sum => 4,
            Tag::Product => 6,
            Tag::Nop => 1,
        }
    }
}

const TAG_BIT: usize = 5 + 8;
const TAG_MASK: u16 = 0b111 << TAG_BIT;
const SMALL_LEAF_TAG_MASK: u16 = 0b1111 << (TAG_BIT - 1);
pub(super) const NULL_ACTION: u32 = !((TAG_MASK as u32) << 16);

impl NodeRepr {
    fn expand(self, tag: Tag, position: u32) -> Node {
        unsafe {
            match (self, tag) {
                (
                    NodeRepr {
                        small_sum: SmallSumRepr { nonterminal, count },
                    },
                    Tag::SmallSum,
                ) => Sum {
                    nonterminal: Symbol::from(nonterminal as u32),
                    count: count as u32,
                },
                (
                    NodeRepr {
                        sum: SumRepr { nonterminal, count },
                    },
                    Tag::Sum,
                ) => Sum { nonterminal, count },
                (
                    NodeRepr {
                        small_link: SmallLinkRepr { distance, action },
                    },
                    Tag::SmallLink,
                ) => Product {
                    action: action as u32,
                    left_factor: NodeHandle(position - distance as u32),
                    right_factor: None,
                },
                (
                    NodeRepr {
                        medium_link: MediumLinkRepr { distance, action },
                    },
                    Tag::MediumLink,
                ) => Product {
                    action: action as u32,
                    left_factor: NodeHandle(position - distance as u32),
                    right_factor: None,
                },
                (
                    NodeRepr {
                        small_product:
                            SmallProductRepr {
                                right_distance,
                                left_distance,
                                action,
                            },
                    },
                    Tag::SmallProduct,
                ) => Product {
                    action: action as u32,
                    left_factor: NodeHandle(position - left_distance as u32),
                    right_factor: Some(NodeHandle(position - right_distance as u32)),
                },
                (
                    NodeRepr {
                        product:
                            ProductRepr {
                                upper_action,
                                lower_action,
                                left_factor,
                                right_factor,
                            },
                    },
                    Tag::Product,
                ) => Product {
                    action: (upper_action as u32) << 16 | (lower_action as u32),
                    left_factor,
                    right_factor: right_factor.to_option(),
                },
                (
                    NodeRepr {
                        small_nulling_leaf: SmallNullingLeafRepr { symbol },
                    },
                    Tag::SmallNullingLeaf,
                ) => NullingLeaf {
                    symbol: Symbol::from(symbol as u32),
                },
                (
                    NodeRepr {
                        small_leaf: SmallLeafRepr { symbol },
                    },
                    Tag::SmallLeaf,
                ) => Evaluated {
                    symbol: Symbol::from(symbol as u32),
                },
                (
                    NodeRepr {
                        leaf: LeafRepr { symbol },
                    },
                    Tag::Leaf,
                ) => Evaluated { symbol },
                _ => unreachable!(),
            }
        }
    }
}

impl Node {
    #[inline]
    fn to_repr(self, position: u32) -> (NodeRepr, usize) {
        let tag = self.classify(position);
        unsafe {
            let mut result = match (self, tag) {
                (Sum { nonterminal, count }, Tag::SmallSum) => NodeRepr {
                    small_sum: SmallSumRepr {
                        nonterminal: nonterminal.usize() as u8,
                        count: count as u8,
                    },
                },
                (Sum { nonterminal, count }, Tag::Sum) => NodeRepr {
                    sum: SumRepr { nonterminal, count },
                },
                (
                    Product {
                        left_factor,
                        right_factor: None,
                        action,
                    },
                    Tag::SmallLink,
                ) => NodeRepr {
                    small_link: SmallLinkRepr {
                        distance: (position - left_factor.0) as u8,
                        action: action as u8,
                    },
                },
                (
                    Product {
                        left_factor,
                        right_factor: None,
                        action,
                    },
                    Tag::MediumLink,
                ) => NodeRepr {
                    medium_link: MediumLinkRepr {
                        distance: (position - left_factor.0) as u16,
                        action: action as u16,
                    },
                },
                (
                    Product {
                        left_factor,
                        right_factor: Some(right),
                        action,
                    },
                    Tag::SmallProduct,
                ) => NodeRepr {
                    small_product: SmallProductRepr {
                        right_distance: (position - right.0) as u8,
                        left_distance: (position - left_factor.0) as u8,
                        action: action as u16,
                    },
                },
                (
                    Product {
                        left_factor,
                        right_factor,
                        action,
                    },
                    Tag::Product,
                ) => NodeRepr {
                    product: ProductRepr {
                        upper_action: (action >> 16) as u16,
                        lower_action: action as u16,
                        left_factor,
                        right_factor: right_factor.unwrap_or(NULL_HANDLE),
                    },
                },
                (NullingLeaf { symbol }, Tag::SmallNullingLeaf) => NodeRepr {
                    small_nulling_leaf: SmallNullingLeafRepr {
                        symbol: symbol.usize() as u16,
                    },
                },
                (NullingLeaf { symbol }, Tag::Leaf) => NodeRepr {
                    leaf: LeafRepr { symbol },
                },
                (Evaluated { symbol }, Tag::SmallLeaf) => NodeRepr {
                    small_leaf: SmallLeafRepr {
                        symbol: symbol.usize() as u16,
                    },
                },
                (Evaluated { symbol }, Tag::Leaf) => NodeRepr {
                    leaf: LeafRepr { symbol },
                },
                _ => unreachable!(),
            };
            result.fields[0] |= tag.to_u16();
            (result, tag.size())
        }
    }

    #[inline]
    pub(super) fn classify(self, position: u32) -> Tag {
        match self {
            Product {
                left_factor,
                right_factor,
                action,
            } => match right_factor {
                Some(handle) => {
                    if position >= handle.0
                        && position >= left_factor.0
                        && position - handle.0 < (1 << 5)
                        && position - left_factor.0 < (1 << 8)
                        && action < (1 << 16)
                    {
                        Tag::SmallProduct
                    } else {
                        Tag::Product
                    }
                }
                None => {
                    if position >= left_factor.0
                        && position - left_factor.0 < (1 << 5)
                        && action < (1 << 8)
                    {
                        Tag::SmallLink
                    } else if position >= left_factor.0
                        && position - left_factor.0 < (1 << (5 + 8))
                        && action < (1 << 16)
                    {
                        Tag::MediumLink
                    } else {
                        Tag::Product
                    }
                }
            },
            NullingLeaf { symbol } => {
                if symbol.usize() < (1 << (4 + 8)) {
                    Tag::SmallNullingLeaf
                } else {
                    Tag::Leaf
                }
            }
            Evaluated { symbol } => {
                if symbol.usize() < (1 << (4 + 8)) {
                    Tag::SmallLeaf
                } else {
                    Tag::Leaf
                }
            }
            Sum { nonterminal, count } => {
                if count < (1 << 5) && nonterminal.usize() < (1 << 8) {
                    Tag::SmallSum
                } else {
                    Tag::Sum
                }
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
unsafe fn get_and_erase_tag(field: u16) -> (Tag, u16) {
    let tag = unwrap_unchecked(Tag::from_u16(field));
    (tag, field & !tag.mask())
}
