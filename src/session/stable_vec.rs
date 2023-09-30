use std::{cmp::Reverse, collections::BinaryHeap, fmt::Debug};

#[derive(Debug, Clone)]
pub(crate) struct StableVec<T> {
    items: Vec<Option<T>>,
    available_indices: BinaryHeap<Reverse<usize>>,
}

#[allow(dead_code)]
impl<T> StableVec<T> {
    pub(crate) fn new() -> Self {
        Self {
            items: Vec::new(),
            available_indices: BinaryHeap::new(),
        }
    }

    pub(crate) fn next_index(&self) -> usize {
        if let Some(Reverse(index)) = self.available_indices.peek() {
            *index
        } else {
            self.items.len()
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub(crate) fn push(&mut self, item: T) -> usize {
        if let Some(Reverse(index)) = self.available_indices.pop() {
            self.items[index].replace(item);
            index
        } else {
            let index = self.items.len();
            self.items.push(Some(item));
            index
        }
    }

    pub(crate) fn get(&self, index: usize) -> Option<&T> {
        self.items.get(index).map(|maybe| maybe.as_ref()).flatten()
    }

    pub(crate) fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.items
            .get_mut(index)
            .map(|maybe| maybe.as_mut())
            .flatten()
    }

    pub(crate) fn remove(&mut self, index: usize) -> Option<T> {
        if let Some(item) = self.items.get_mut(index).map(|elem| elem.take()).flatten() {
            self.available_indices.push(Reverse(index));
            Some(item)
        } else {
            None
        }
    }

    pub(crate) fn iter(&self) -> std::slice::Iter<'_, Option<T>> {
        self.items.iter()
    }

    pub(crate) fn iter_mut(&mut self) -> std::slice::IterMut<'_, Option<T>> {
        self.items.iter_mut()
    }
}

impl<T, Collection> From<Collection> for StableVec<T>
where
    Collection: IntoIterator<Item = T>,
{
    fn from(items: Collection) -> Self {
        Self {
            items: items.into_iter().map(|item| Some(item)).collect(),
            available_indices: BinaryHeap::new(),
        }
    }
}

impl<T> PartialEq for StableVec<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.items == other.items
    }
}

impl<T> Eq for StableVec<T> where T: Eq {}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    struct Foo(usize);

    #[test]
    fn push_and_get() {
        let mut stable_vec = StableVec::new();

        assert_eq!(stable_vec.push(Foo(1)), 0);
        assert_eq!(stable_vec.push(Foo(2)), 1);
        assert_eq!(stable_vec.push(Foo(3)), 2);

        assert_eq!(stable_vec.get(0), Some(&Foo(1)));
        assert_eq!(stable_vec.get(1), Some(&Foo(2)));
        assert_eq!(stable_vec.get(2), Some(&Foo(3)));

        assert_eq!(stable_vec.remove(1), Some(Foo(2)));
        assert_eq!(stable_vec.get(0), Some(&Foo(1)));
        assert_eq!(stable_vec.get(1), None);
        assert_eq!(stable_vec.get(2), Some(&Foo(3)));
    }

    #[test]
    fn from() {
        let mut vec1 = StableVec::new();
        vec1.push(Foo(1));
        vec1.push(Foo(2));
        vec1.push(Foo(3));

        let vec2 = StableVec::from([Foo(1), Foo(2), Foo(3)]);

        assert_eq!(vec1, vec2);
    }

    #[test]
    fn remove() {
        let mut stable_vec = StableVec::from([Foo(1), Foo(2), Foo(3)]);

        assert_eq!(stable_vec.remove(1), Some(Foo(2)));
        assert_eq!(stable_vec.get(0), Some(&Foo(1)));
        assert_eq!(stable_vec.get(1), None);
        assert_eq!(stable_vec.get(2), Some(&Foo(3)));
    }

    #[test]
    fn push_after_remove() {
        let mut stable_vec = StableVec::from([Foo(1), Foo(2), Foo(3)]);
        stable_vec.remove(1);

        assert_eq!(stable_vec.push(Foo(4)), 1);
        assert_eq!(stable_vec.get(1), Some(&Foo(4)));
    }

    #[test]
    fn consecutive_removes() {
        let mut stable_vec = StableVec::from([Foo(1), Foo(2), Foo(3), Foo(4)]);
        stable_vec.remove(1);
        stable_vec.remove(2);

        assert_eq!(stable_vec.push(Foo(5)), 1);
        assert_eq!(stable_vec.push(Foo(6)), 2);

        assert_eq!(stable_vec.get(0), Some(&Foo(1)));
        assert_eq!(stable_vec.get(1), Some(&Foo(5)));
        assert_eq!(stable_vec.get(2), Some(&Foo(6)));
        assert_eq!(stable_vec.get(3), Some(&Foo(4)));
    }
}
