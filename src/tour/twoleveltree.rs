use getset::Getters;

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use crate::{
    node::{Container, Node},
    Scalar,
};

use super::{between, Tour, TourOrder, Vertex};

type RcVertex = Rc<RefCell<TltVertex>>;
type WeakVertex = Weak<RefCell<TltVertex>>;
type RcParent = Rc<RefCell<ParentVertex>>;
type WeakParent = Weak<RefCell<ParentVertex>>;

#[derive(Debug)]
pub struct TwoLevelTree<'a> {
    container: &'a Container,
    vertices: Vec<RcVertex>,
    parents: Vec<RcParent>,
    total_dist: Scalar,
}

impl<'a> TwoLevelTree<'a> {
    pub fn new(container: &'a Container, max_groupsize: usize) -> Self {
        let n_parents = (container.len() / max_groupsize) + 1;

        let vertices = container
            .into_iter()
            .map(|n| TltVertex::new(n).to_rc())
            .collect();

        let parents = (0..n_parents)
            .into_iter()
            .map(|id| ParentVertex::new(id, max_groupsize).to_rc())
            .collect();

        Self {
            container,
            vertices,
            parents,
            total_dist: 0.,
        }
    }
}

impl<'a> Tour for TwoLevelTree<'a> {
    type TourNode = TltVertex;

    fn apply(&mut self, tour: &TourOrder) {
        let tour = tour.order();

        let p_len = self.parents.len();
        let v_len = self.vertices.len();

        self.total_dist = 0.;

        for ip in 0..p_len {
            let p = self.parents.get(ip).unwrap();
            let next_p = self.parents.get((ip + 1) % p_len).unwrap();
            let prev_p = if ip == 0 {
                self.parents.last().unwrap()
            } else {
                self.parents.get(ip - 1).unwrap()
            };

            p.borrow_mut().size = 0;

            p.borrow_mut().next = Rc::downgrade(next_p);
            next_p.borrow_mut().prev = Rc::downgrade(p);
            p.borrow_mut().prev = Rc::downgrade(prev_p);
            prev_p.borrow_mut().next = Rc::downgrade(p);

            let beg_seg = ip * p.borrow().max_size;
            let end_seg = (beg_seg + p.borrow().max_size).min(v_len);

            for iv in beg_seg..end_seg {
                let v = self.vertices.get(tour[iv]).unwrap();
                v.borrow_mut().seq_id = iv - beg_seg;
                v.borrow_mut().parent = Rc::downgrade(p);

                if iv == beg_seg {
                    p.borrow_mut().first = Rc::downgrade(v);
                }

                if iv == end_seg - 1 {
                    p.borrow_mut().last = Rc::downgrade(v);
                }

                let next_v = self.vertices.get(tour[(iv + 1) % v_len]).unwrap();
                let prev_v = if iv == 0 {
                    self.vertices.last().unwrap()
                } else {
                    self.vertices.get(tour[iv - 1]).unwrap()
                };

                v.borrow_mut().next = Rc::downgrade(next_v);
                next_v.borrow_mut().prev = Rc::downgrade(v);
                v.borrow_mut().prev = Rc::downgrade(prev_v);
                prev_v.borrow_mut().next = Rc::downgrade(v);

                self.total_dist += self
                    .container
                    .distance(&v.borrow().node, &next_v.borrow().node);

                p.borrow_mut().size += 1;
                v.borrow_mut().visited = false;
            }
        }
    }

    fn between(&self, from: &Self::TourNode, mid: &Self::TourNode, to: &Self::TourNode) -> bool {
        let (fp, mp, tp) = (&from.parent, &mid.parent, &to.parent);
        match (
            Weak::ptr_eq(fp, mp),
            Weak::ptr_eq(mp, tp),
            Weak::ptr_eq(tp, fp),
        ) {
            (true, true, true) => between(from.seq_id, mid.seq_id, to.seq_id),
            (true, false, false) => {
                if let Some(p) = fp.upgrade() {
                    p.borrow().reverse ^ (from.seq_id <= mid.seq_id)
                } else {
                    false
                }
            }
            (false, true, false) => {
                if let Some(p) = mp.upgrade() {
                    p.borrow().reverse ^ (mid.seq_id <= to.seq_id)
                } else {
                    false
                }
            }
            (false, false, true) => {
                if let Some(p) = tp.upgrade() {
                    p.borrow().reverse ^ (to.seq_id <= from.seq_id)
                } else {
                    false
                }
            }
            (false, false, false) => unsafe {
                between(
                    (&*fp.as_ptr()).borrow().id,
                    (&*mp.as_ptr()).borrow().id,
                    (&*tp.as_ptr()).borrow().id,
                )
            },
            // (true, true, false)
            // (true, false, true)
            // (false, true, true)
            _ => panic!("The transitivity requirement is violated."),
        }
    }

    /// This implementation should compute in *O*(1) time, with some constants.
    fn between_at(&self, from_idx: usize, mid_idx: usize, to_idx: usize) -> bool {
        match (self.get(from_idx), self.get(mid_idx), self.get(to_idx)) {
            (Some(from), Some(mid), Some(to)) => self.between(from, mid, to),
            _ => false,
        }
    }

    #[inline]
    fn distance(&self, a: usize, b: usize) -> Scalar {
        // TODO: check if nodes belong to the group.
        self.container
            .distance(self.get(a).unwrap().node(), self.get(b).unwrap().node())
    }

    /// This implementation of the `flip` operation takes at least Sigma(N) time to compute.
    fn flip_at(&mut self, _from_a: usize, _to_a: usize, _from_b: usize, _to_b: usize) {
        todo!()
    }

    /// The operation should compute in *O*(1) time.
    #[inline]
    fn get(&self, node_idx: usize) -> Option<&Self::TourNode> {
        match self.vertices.get(node_idx) {
            Some(v) => unsafe { v.as_ref().as_ptr().as_ref() },
            None => None,
        }
    }

    #[inline]
    fn next(&self, node: &Self::TourNode) -> Option<&Self::TourNode> {
        match node.parent.upgrade() {
            Some(p) => {
                let kin = if p.borrow().reverse {
                    &node.prev
                } else {
                    &node.next
                };

                return match kin.upgrade() {
                    Some(node) => unsafe { node.as_ref().as_ptr().as_ref() },
                    None => None,
                };
            }
            None => None,
        }
    }

    /// The operation should compute in *O*(1) time.
    // Note: There might be hit in performance due to memory safeguarding. Need benchmark to verify.
    #[inline]
    fn next_at(&self, node_idx: usize) -> Option<&Self::TourNode> {
        if let Some(v) = self.vertices.get(node_idx) {
            let borrow_v = v.borrow();
            if let Some(p) = borrow_v.parent.upgrade() {
                let kin = if p.borrow().reverse {
                    &borrow_v.prev
                } else {
                    &borrow_v.next
                };

                return match kin.upgrade() {
                    Some(node) => unsafe { node.as_ref().as_ptr().as_ref() },
                    None => None,
                };
            }
        }

        None
    }

    #[inline]
    fn prev(&self, node: &Self::TourNode) -> Option<&Self::TourNode> {
        match node.parent.upgrade() {
            Some(p) => {
                let kin = if p.borrow().reverse {
                    &node.next
                } else {
                    &node.prev
                };

                return match kin.upgrade() {
                    Some(node) => unsafe { node.as_ref().as_ptr().as_ref() },
                    None => None,
                };
            }
            None => None,
        }
    }

    /// The operation should compute in *O*(1) time.
    // Note: There might be hit in performance due to memory safeguarding. Need benchmark to verify.
    #[inline]
    fn prev_at(&self, node_idx: usize) -> Option<&Self::TourNode> {
        if let Some(v) = self.vertices.get(node_idx) {
            let borrow_v = v.borrow();
            if let Some(p) = borrow_v.parent.upgrade() {
                let kin = if p.borrow().reverse {
                    &borrow_v.next
                } else {
                    &borrow_v.prev
                };

                return match kin.upgrade() {
                    Some(node) => unsafe { node.as_ref().as_ptr().as_ref() },
                    None => None,
                };
            }
        }

        None
    }

    #[inline]
    fn reset(&mut self) {
        for vt in &mut self.vertices {
            vt.borrow_mut().visited(false);
        }
    }

    #[inline]
    fn size(&self) -> usize {
        self.vertices.len()
    }

    fn total_distance(&self) -> Scalar {
        self.total_dist
    }

    fn visited_at(&mut self, kin_index: usize, flag: bool) {
        self.vertices[kin_index].borrow_mut().visited(flag);
    }
}

impl<'a> IntoIterator for TwoLevelTree<'a> {
    type Item = TltVertex;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        todo!()
    }
}

impl<'a, 's> IntoIterator for &'s TwoLevelTree<'a> {
    type Item = &'s TltVertex;
    type IntoIter = std::slice::Iter<'s, TltVertex>;

    fn into_iter(self) -> Self::IntoIter {
        todo!()
    }
}

#[derive(Debug, Getters)]
pub struct TltVertex {
    /// Sequential ID in the parent node.
    ///
    /// If a vertex is not attached to any parent node, `usize::MAX` will be assigned.
    seq_id: usize,
    #[getset(get = "pub")]
    node: Node,
    visited: bool,
    prev: WeakVertex,
    next: WeakVertex,
    parent: WeakParent,
}

impl TltVertex {
    pub fn new(node: &Node) -> Self {
        Self {
            node: node.clone(),
            seq_id: usize::MAX,
            visited: false,
            prev: Weak::new(),
            next: Weak::new(),
            parent: Weak::new(),
        }
    }

    fn to_rc(self) -> RcVertex {
        Rc::new(RefCell::new(self))
    }
}

impl Vertex for TltVertex {
    fn index(&self) -> usize {
        self.node.index()
    }

    fn is_visited(&self) -> bool {
        self.visited
    }

    fn visited(&mut self, flag: bool) {
        self.visited = flag;
    }
}

impl PartialEq for TltVertex {
    fn eq(&self, other: &Self) -> bool {
        // TODO: expand comparison to pointer.
        self.node == other.node && self.visited == other.visited
    }
}

#[derive(Debug)]
struct ParentVertex {
    id: usize,
    size: usize,
    max_size: usize,
    reverse: bool,
    prev: WeakParent,
    next: WeakParent,
    first: WeakVertex,
    last: WeakVertex,
}

impl ParentVertex {
    fn new(id: usize, max_size: usize) -> Self {
        Self {
            id,
            size: 0,
            max_size,
            reverse: false,
            prev: Weak::new(),
            next: Weak::new(),
            first: Weak::new(),
            last: Weak::new(),
        }
    }

    fn to_rc(self) -> RcParent {
        Rc::new(RefCell::new(self))
    }

    #[allow(dead_code)]
    fn reverse(&mut self) {
        // TODO: two inner ifs have the same structure => potential refractor.
        if let (Some(first), Some(last)) = (self.first.upgrade(), self.last.upgrade()) {
            let (tmp_prev, tmp_next) = if self.reverse {
                (last.borrow().next.clone(), first.borrow().prev.clone())
            } else {
                (first.borrow().prev.clone(), last.borrow().next.clone())
            };

            if let (Some(prev_c), Some(prev_p)) = (tmp_prev.upgrade(), self.prev.upgrade()) {
                match (prev_p.borrow().reverse, self.reverse) {
                    (true, true) => {
                        // reverse, reverse => reverse, forward
                        prev_c.borrow_mut().prev = Rc::downgrade(&first);
                        first.borrow_mut().prev = Rc::downgrade(&prev_c);
                    }
                    (true, false) => {
                        // reverse, forward => reverse, reverse
                        prev_c.borrow_mut().prev = Rc::downgrade(&last);
                        last.borrow_mut().next = Rc::downgrade(&prev_c);
                    }
                    (false, true) => {
                        // forward, reverse => forward, forward
                        prev_c.borrow_mut().next = Rc::downgrade(&first);
                        first.borrow_mut().prev = Rc::downgrade(&prev_c);
                    }
                    (false, false) => {
                        // forward, forward => forward, reverse
                        prev_c.borrow_mut().next = Rc::downgrade(&last);
                        last.borrow_mut().next = Rc::downgrade(&prev_c);
                    }
                }
            }

            if let (Some(next_c), Some(next_p)) = (tmp_next.upgrade(), self.next.upgrade()) {
                match (self.reverse, next_p.borrow().reverse) {
                    (true, true) => {
                        // reverse, reverse => forward, reverse
                        last.borrow_mut().next = Rc::downgrade(&next_c);
                        next_c.borrow_mut().next = Rc::downgrade(&last);
                    }
                    (true, false) => {
                        // reverse, forward => forward, forward
                        last.borrow_mut().next = Rc::downgrade(&next_c);
                        next_c.borrow_mut().prev = Rc::downgrade(&last);
                    }
                    (false, true) => {
                        // forward, reverse => reverse, reverse
                        first.borrow_mut().prev = Rc::downgrade(&next_c);
                        next_c.borrow_mut().next = Rc::downgrade(&first);
                    }
                    (false, false) => {
                        // forward, forward => reverse, forward
                        first.borrow_mut().prev = Rc::downgrade(&next_c);
                        next_c.borrow_mut().prev = Rc::downgrade(&first);
                    }
                }
            }
        }

        self.reverse ^= true;
    }
}

impl PartialEq for ParentVertex {
    fn eq(&self, other: &Self) -> bool {
        // TODO: expand comparison to pointer.
        self.id == other.id && self.size == other.size && self.max_size == other.max_size
    }
}

#[allow(dead_code, unused_imports)]
mod tests {
    use crate::tour::tests::test_tree_order;

    use super::super::tests::create_container;
    use super::*;

    #[test]
    fn test_apply() {
        let n_nodes = 10;
        let container = create_container(n_nodes);
        let mut tree = TwoLevelTree::new(&container, 3);
        tree.apply(&TourOrder::new((0..n_nodes).collect()));

        assert_eq!(4, tree.parents.len());
        assert_eq!(n_nodes, tree.vertices.len());

        // First parent group
        let first_p = tree.parents[0].borrow();
        assert_eq!(3, first_p.size);
        assert!(first_p.first.upgrade().is_some());
        assert!(first_p.last.upgrade().is_some());

        assert!(Weak::ptr_eq(
            &Rc::downgrade(&tree.vertices[0]),
            &first_p.first
        ));
        assert!(Weak::ptr_eq(
            &Rc::downgrade(&tree.vertices[2]),
            &first_p.last
        ));

        // Last parent group
        let last_p = tree.parents.last().unwrap().borrow();
        assert_eq!(1, last_p.size);
        assert!(last_p.first.upgrade().is_some());
        assert!(last_p.last.upgrade().is_some());
        assert!(Weak::ptr_eq(
            &Rc::downgrade(&tree.vertices.last().unwrap()),
            &last_p.first
        ));
        assert!(Weak::ptr_eq(
            &Rc::downgrade(&tree.vertices.last().unwrap()),
            &last_p.last
        ));

        assert!(Weak::ptr_eq(
            &Rc::downgrade(&tree.parents.last().unwrap()),
            &tree.parents.first().unwrap().borrow().prev
        ));

        assert!(Weak::ptr_eq(
            &Rc::downgrade(&tree.parents.first().unwrap()),
            &tree.parents.last().unwrap().borrow().next
        ));
    }

    #[test]
    fn test_total_dist() {
        let n_nodes = 4;
        let container = create_container(n_nodes);
        let mut tree = TwoLevelTree::new(&container, 3);

        tree.apply(&TourOrder::new(vec![0, 1, 2, 3]));
        assert_eq!(6. * (2. as Scalar).sqrt(), tree.total_distance());

        tree.apply(&TourOrder::new(vec![1, 3, 0, 2]));
        assert_eq!(8. * (2. as Scalar).sqrt(), tree.total_distance());
    }

    #[test]
    fn test_next_and_prev() {
        let n_nodes = 10;
        let container = create_container(n_nodes);
        let mut tree = TwoLevelTree::new(&container, 3);
        let expected = TourOrder::new((0..n_nodes).collect());
        tree.apply(&expected);
        test_tree_order(&tree, &expected);

        let expected = TourOrder::new(vec![9, 1, 2, 4, 6, 3, 5, 8, 0, 7]);
        tree.apply(&expected);
        test_tree_order(&tree, &expected);
    }

    #[test]
    fn test_between_forward() {
        let n_nodes = 10;
        let container = create_container(n_nodes);
        let mut tree = TwoLevelTree::new(&container, 3);
        tree.apply(&TourOrder::new((0..n_nodes).collect()));

        //  0 -> 1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7 -> 8 -> 9

        // All vertices reside under the same parent node.
        assert!(tree.between_at(0, 1, 2)); // true
        assert!(!tree.between_at(0, 2, 1)); // false
        assert!(!tree.between_at(2, 1, 0)); // false
        assert!(tree.between_at(2, 0, 1)); // true

        // All vertices reside under distinct parent node.
        assert!(tree.between_at(2, 3, 7)); // true
        assert!(!tree.between_at(2, 7, 3)); // true
        assert!(!tree.between_at(7, 3, 2)); // false
        assert!(tree.between_at(7, 2, 3)); // true

        // Two out of three vertices reside under the same parent node.
        assert!(tree.between_at(3, 5, 8)); // true
        assert!(!tree.between_at(3, 8, 5)); // false
        assert!(!tree.between_at(8, 5, 3)); // false
        assert!(tree.between_at(8, 3, 5)); // true
    }

    #[test]
    fn test_parent_reverse() {
        let n_nodes = 10;
        let container = create_container(n_nodes);
        let mut tree = TwoLevelTree::new(&container, 3);

        tree.apply(&TourOrder::new((0..n_nodes).collect()));

        // 0 -> 1 -> 2 -> 5 -> 4 -> 3 -> 6 -> 7 -> 8 -> 9
        tree.parents[1].borrow_mut().reverse();
        test_tree_order(&tree, &TourOrder::new(vec![0, 1, 2, 5, 4, 3, 6, 7, 8, 9]));

        // 0 -> 1 -> 2 -> 5 -> 4 -> 3 -> 8 -> 7 -> 6 -> 9
        tree.parents[2].borrow_mut().reverse();
        let order = TourOrder::new(vec![0, 1, 2, 5, 4, 3, 8, 7, 6, 9]);
        test_tree_order(&tree, &order);

        tree.parents[3].borrow_mut().reverse();
        test_tree_order(&tree, &order);

        // 0 -> 1 -> 2 -> 3 -> 4 -> 5 -> 6 -> 7 -> 8 -> 9
        tree.parents[1].borrow_mut().reverse();
        tree.parents[2].borrow_mut().reverse();
        test_tree_order(&tree, &TourOrder::new((0..10).collect()));
    }
}
