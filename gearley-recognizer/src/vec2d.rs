use std::ops;

use serde_derive::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct Vec2d<I> {
    pub(crate) chart: Vec<I>,
    pub(crate) indices: Vec<usize>,
    pub(crate) current_start: usize,
}

#[derive(Default)]
pub(crate) struct Vec2dCapacity {
    pub(crate) chart_capacity: usize,
    pub(crate) indices_capacity: usize,
}

impl<I> Vec2d<I> {
    pub fn with_capacity(capacity: Vec2dCapacity) -> Self {
        // The first Earley set begins at 0 and ends at 0. The second Earley set begins at 0.
        let mut indices = Vec::with_capacity(capacity.indices_capacity);
        indices.push(0);
        Self {
            chart: Vec::with_capacity(capacity.chart_capacity),
            indices,
            current_start: 0,
        }
    }

    pub fn clear(&mut self) {
        self.chart.clear();
        // Indices reset to [0, 0].
        self.indices.clear();
        self.indices.push(0);
        // Current medial start reset to 0.
        self.current_start = 0;
    }

    pub fn truncate(&mut self, new_len: usize) where I: Copy {
        let new_medial_start = self.indices[new_len - 1];
        for i in 0..self.current_start {
            self.chart[new_medial_start as usize + i] = self.chart[self.current_start + i];
        }
        self.chart
            .truncate(new_medial_start as usize + self.current_start);
        self.current_start = new_medial_start as usize;
    }

    #[inline]
    pub fn last(&self) -> &[I] {
        &self.chart[self.current_start..]
    }

    #[inline]
    pub fn last_mut(&mut self) -> &mut [I] {
        &mut self.chart[self.current_start..]
    }

    #[inline]
    pub fn last_item(&self) -> Option<&I> {
        self.chart.last()
    }

    #[inline]
    pub fn get_item(&self, index: usize) -> Option<&I> {
        self.chart.get(index)
    }

    #[inline]
    pub fn push_item(&mut self, item: I) {
        self.chart.push(item);
    }

    #[inline]
    pub fn pop_item(&mut self) -> Option<I> {
        self.chart.pop()
    }

    #[inline]
    pub fn item_count(&self) -> usize {
        self.chart.len()
    }

    pub fn index_at(&self, set_id: usize) -> usize {
        self.indices[set_id]
    }

    pub fn extend_with_set(&mut self, set: impl Iterator<Item = I>) {
        self.chart.extend(set);
        self.next_set();
    }

    pub fn next_set(&mut self) {
        self.current_start = self.chart.len();
        self.indices.push(self.current_start);
    }

    pub fn len(&self) -> usize {
        self.indices.len() - 1
    }
}

impl<I> ops::Index<usize> for Vec2d<I> {
    type Output = [I];

    fn index(&self, index: usize) -> &Self::Output {
        &self.chart[self.indices[index] .. self.indices[index + 1]]
    }
}

impl<I, A: IntoIterator<Item = I>> FromIterator<A> for Vec2d<I> {
    fn from_iter<T: IntoIterator<Item = A>>(iter: T) -> Self {
        let mut result = Vec2d::with_capacity(Default::default());
        result.extend(iter);
        result
    }
}

impl<I> Default for Vec2d<I> {
    fn default() -> Self {
        Self {
            chart: vec![],
            indices: vec![0],
            current_start: 0,
        }
    }
}

impl<I, A: IntoIterator<Item = I>> Extend<A> for Vec2d<I> {
    fn extend<T: IntoIterator<Item = A>>(&mut self, iter: T) {
        for set in iter {
            self.extend_with_set(set.into_iter());
        }
    }
}