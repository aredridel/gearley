use cfg::symbol::Symbol;

use forest::Forest;
use item::CompletedItem;

/// An empty forest.
pub struct NullForest;

impl Forest for NullForest {
    type NodeRef = ();
    type LeafValue = ();

    #[inline(always)] fn leaf(&mut self, _: Symbol, _: u32, _: ()) {}
    #[inline(always)] fn nulling(&self, _: Symbol) {}
    #[inline(always)] fn push_summand(&mut self, _item: CompletedItem<Self::NodeRef>) {}
    #[inline(always)] fn sum(&mut self, _lhs_sym: Symbol, _origin: u32) -> Self::NodeRef { () }
}
