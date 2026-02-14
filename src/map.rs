use std::{marker::PhantomData, mem, ptr::NonNull};

type Link<K, V> = Option<NonNull<TreeNode<K, V>>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Colour {
    Red,
    Black
}

struct TreeNode<K, V> {
    key: K,
    value: V,
    colour: Colour,
    left: Link<K, V>,
    right: Link<K, V>,
    parent: Link<K, V>
}

impl<K, V> TreeNode<K, V> {
    fn left(&self) -> Option<&Self> {
        self.left.map(|left| unsafe { &*left.as_ptr() })
    }

    fn right(&self) -> Option<&Self> {
        self.right.map(|right| unsafe { &*right.as_ptr() })
    }

    fn parent(&self) -> Option<&Self> {
        self.parent.map(|parent| unsafe { &*parent.as_ptr() })
    }
    
    fn left_mut(&mut self) -> Option<&mut Self> {
        self.left.map(|left| unsafe { &mut *left.as_ptr() })
    }

    fn right_mut(&mut self) -> Option<&mut Self> {
        self.right.map(|right| unsafe { &mut *right.as_ptr() })
    }

    fn parent_mut(&mut self) -> Option<&mut Self> {
        self.parent.map(|parent| unsafe { &mut *parent.as_ptr() })
    }

    fn add_left(&mut self, left: Link<K, V>) {
        if let Some(left) = left {
            let left = unsafe {
                &mut *left.as_ptr()
            };
            left.parent = unsafe {
                Some(NonNull::new_unchecked(self))
            };
        }
        self.left = left;
    }
    
    fn add_right(&mut self, right: Link<K, V>) {
        if let Some(right) = right {
            let right = unsafe {
                &mut *right.as_ptr()
            };
            right.parent = unsafe {
                Some(NonNull::new_unchecked(self))
            };
        }
        self.right = right;
    }

    fn swap_contents(&mut self, other: &mut Self) {
        mem::swap(&mut self.key, &mut other.key);
        mem::swap(&mut self.value, &mut other.value);
        mem::swap(&mut self.colour, &mut other.colour);
    }

    fn unlink_parent(&mut self) {
        if let Some(parent) = self.parent {
            let parent = unsafe {
                &mut *parent.as_ptr()
            };
            if parent.left.map(|left| left.as_ptr() == self).unwrap_or(false) {
                parent.left = None;
            } else {
                parent.right = None;
            }
        }
    }

    fn next(&self) -> Option<NonNull<Self>> {
        let mut node = self;
        loop {
            if let Some(mut right) = node.right {
                loop {
                    let node = unsafe {
                        &*right.as_ptr()
                    };
                    if let Some(left) = node.left {
                        right = left;
                    } else {
                        break;
                    }
                }
                return Some(right);
            } else {
                if let Some(parent) = node.parent {
                    let parent = unsafe {
                        &*parent.as_ptr()
                    };
                    if parent.left.map(|left| left.as_ptr() as *const _ == node).unwrap_or(false) {
                         return unsafe {
                            Some(NonNull::new_unchecked(&raw const *parent as *mut _))
                        };
                    } else {
                        node = parent;
                    }
                } else {
                    return None;
                }
            }
        }
    }
    
    fn prev(&self) -> Option<NonNull<Self>> {
        let mut node = self;
        loop {
            if let Some(mut left) = node.left {
                loop {
                    let node = unsafe {
                        &*left.as_ptr()
                    };
                    if let Some(right) = node.right {
                        left = right;
                    } else {
                        break;
                    }
                }
                return Some(left);
            } else {
                if let Some(parent) = node.parent {
                    let parent = unsafe {
                        &*parent.as_ptr()
                    };
                    if parent.right.map(|left| left.as_ptr() as *const _ == node).unwrap_or(false) {
                         return unsafe {
                            Some(NonNull::new_unchecked(&raw const *parent as *mut _))
                        };
                    } else {
                        node = parent;
                    }
                } else {
                    return None;
                }
            }
        }
    }
}

pub struct NodeIter<K, V> {
    node: Link<K, V>
}

impl<K, V> NodeIter<K, V> {
    fn next(&mut self) {
        if let Some(node) = self.node {
            let node = unsafe {
                & *node.as_ptr()
            };
            self.node = node.next();
        }
    }
}

pub struct Iter<'a, K, V> {
    node_iter: NodeIter<K, V>,
    _phantom: PhantomData<(&'a K, &'a V)>
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        let prev = self.node_iter.node;
        self.node_iter.next();
        if let Some(prev) = prev {
            let prev = unsafe {
                & *prev.as_ptr()
            };
            Some((&prev.key, &prev.value))
        } else {
            None
        }
    }
}

pub struct IterMut<'a, K, V> {
    node_iter: NodeIter<K, V>,
    _phantom: PhantomData<(&'a K, &'a mut V)>
}

impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    fn next(&mut self) -> Option<Self::Item> {
        let prev = self.node_iter.node;
        self.node_iter.next();
        if let Some(prev) = prev {
            let prev = unsafe {
                &mut *prev.as_ptr()
            };
            Some((&prev.key, &mut prev.value))
        } else {
            None
        }
    }
}

pub struct RBTree<K, V> 
where 
    K: Ord
{
    root: Link<K, V>,
    smallest: Link<K, V>,
    largest: Link<K, V>,
}

impl<K: Ord, V> RBTree<K, V> {
    pub fn new() -> Self {
        Self {
            root: None,
            smallest: None,
            largest: None
        }
    }

    fn get_link_colour(link: &Link<K, V>) -> Colour {
        if let Some(link) = link {
            let link = unsafe {
                &*link.as_ptr()
            };
            link.colour
        } else {
            Colour::Black
        }
    }

    fn search_parent(&self, key: &K) -> (bool, Link<K, V>) {
        let mut last_root = None;
        let mut left = false;
        let mut root = self.root;
        while let Some(parent) = root {
            last_root = root;
            let parent = unsafe {
                & *parent.as_ptr()
            };
            if *key < parent.key {
                root = parent.left;
                left = true;
            } else {
                root = parent.right;
                left = false;
            }
        }
        (left, last_root)
    }

    fn search(&self, key: &K) -> Link<K, V> {
        let mut current = self.root;
        while let Some(val) = current {
            let val = unsafe {
                & *val.as_ptr()
            };
            if *key == val.key {
                return current;
            } else if *key < val.key {
                current = val.left;
            } else {
                current = val.right;
            }
        }
        None
    }

    fn left_rotate(parent: &mut TreeNode<K, V>, child: &mut TreeNode<K, V>) {
        parent.right = child.left;
        if let Some(grand_parent) = parent.parent {
            let grand_parent = unsafe {
                &mut *grand_parent.as_ptr()
            };
            if grand_parent.left.map(|left| left.as_ptr() == parent).unwrap_or(false) {
                unsafe {
                    grand_parent.left = Some(NonNull::new_unchecked(child));
                }
            } else {
                unsafe {
                    grand_parent.right = Some(NonNull::new_unchecked(child));
                }
            }
        }
        unsafe {
            parent.parent = Some(NonNull::new_unchecked(child));
            child.left = Some(NonNull::new_unchecked(parent));
        }
        child.parent = parent.parent;
    }
    
    fn right_rotate(parent: &mut TreeNode<K, V>, child: &mut TreeNode<K, V>) {
        parent.left = child.right;
        if let Some(grand_parent) = parent.parent {
            let grand_parent = unsafe {
                &mut *grand_parent.as_ptr()
            };
            if grand_parent.left.map(|left| left.as_ptr() == parent).unwrap_or(false) {
                unsafe {
                    grand_parent.left = Some(NonNull::new_unchecked(child));
                }
            } else {
                unsafe {
                    grand_parent.right = Some(NonNull::new_unchecked(child));
                }
            }
        }
        unsafe {
            parent.parent = Some(NonNull::new_unchecked(child));
            child.right = Some(NonNull::new_unchecked(parent));
        }
        child.parent = parent.parent;
    }

    fn get_uncle(grand_parent: &TreeNode<K, V>, parent: &TreeNode<K, V>) -> (bool, Link<K, V>) {
        if grand_parent.left.map(|left| left.as_ptr() as *const _ == parent).unwrap_or(false) {
            (true, grand_parent.right)
        } else {
            (false, grand_parent.left)
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        let parent = self.search_parent(&key);
        if let (left, Some(parent)) = parent {
            let parent = unsafe {
                &mut *parent.as_ptr()
            };
            let mut leaf = unsafe {
                NonNull::new_unchecked(Box::into_raw(Box::new(TreeNode {
                    key,
                    value,
                    colour: Colour::Red,
                    left: None,
                    right: None,
                    parent: Some(NonNull::new_unchecked(parent))
                })))
            };
            if left {
                parent.left = Some(leaf);
                if self.smallest.unwrap().as_ptr() == parent {
                    self.smallest = Some(leaf);
                }
            } else {
                parent.right = Some(leaf);
                if self.largest.unwrap().as_ptr() == parent {
                    self.largest = Some(leaf);
                }
            };
            loop {
                let child = unsafe {
                    &mut *leaf.as_ptr()
                };
                if let Some(parent) = child.parent {
                    let parent = unsafe {
                        &mut *parent.as_ptr()
                    };
                    let left = parent.left.map(|left| left.as_ptr() == child).unwrap_or(false);
                    // if parent is black or tree root, finished
                    if parent.colour == Colour::Black {
                        break;
                    } else if let Some(grand_parent) = parent.parent {
                        let grand_parent = unsafe {
                            &mut *grand_parent.as_ptr()
                        };
                        let (parent_left, uncle) = Self::get_uncle(grand_parent, parent);
                        let uncle_colour = Self::get_link_colour(&uncle);
                        if uncle_colour == Colour::Red {
                            let uncle = unsafe {
                                &mut *uncle.unwrap().as_ptr()
                            };
                            // if uncle red, recolour parent and uncle to black and grand parent to
                            // red and move up to grand parent
                            parent.colour = Colour::Black;
                            uncle.colour = Colour::Black;
                            grand_parent.colour = Colour::Red;
                            unsafe {
                                leaf = NonNull::new_unchecked(grand_parent)
                            };
                        } else {
                            if left == parent_left {
                                Self::left_rotate(parent, child); 
                            }
                            Self::right_rotate(grand_parent, parent);
                            parent.colour = Colour::Black;
                            grand_parent.colour = Colour::Red;
                            break;
                        }
                    } else {
                        // if parent is root, recolour to black and finish
                        parent.colour = Colour::Black;
                        break;
                    }
                } else {
                    // if parent is none, at root so finished
                    break;
                }
            }
        } else {
            let leaf = unsafe {
                NonNull::new_unchecked(Box::into_raw(Box::new(TreeNode {
                    key,
                    value,
                    colour: Colour::Black,
                    left: None,
                    right: None,
                    parent: None
                })))
            };
            self.root = Some(leaf);
            self.smallest = Some(leaf);
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        let node = self.search(key);
        node.map(|node| unsafe { &(*node.as_ptr()).value })
    }
    
    pub fn get_mut(&self, key: &K) -> Option<&mut V> {
        let node = self.search(key);
        node.map(|node| unsafe { &mut (*node.as_ptr()).value })
    }

    fn remove_node(&mut self, node: NonNull<TreeNode<K, V>>) -> NonNull<TreeNode<K, V>> {
        if self.smallest.unwrap() == node {
            self.smallest = unsafe { (*self.smallest.unwrap().as_ptr()).next() }
        }
        if self.largest.unwrap() == node {
            self.largest = unsafe { (*self.largest.unwrap().as_ptr()).prev() }
        }
        let mut node = unsafe {
            &mut *node.as_ptr()
        };
        loop {
            if let Some(left) = node.left {
                if let Some(right) = node.right {
                    // swap with in order successor (left most right node)
                    let mut next_node = unsafe {
                        &mut *right.as_ptr()
                    };
                    while let Some(left) = next_node.left {
                        next_node = unsafe {
                            &mut *left.as_ptr()
                        };
                    }
                    node.swap_contents(next_node);
                    node = next_node;
                } else {
                    // swap with child and set child black
                    let left = unsafe {
                        &mut *left.as_ptr()
                    };
                    left.colour = Colour::Black;
                    node.swap_contents(left);
                    node = left;
                }
            } else if let Some(right) = node.right {
                // swap with child and set child black
                let right = unsafe {
                    &mut *right.as_ptr()
                };
                right.colour = Colour::Black;
                node.swap_contents(right);
                node = right;
            } else {
                // if root replace with None
                if node.parent.is_none() {
                    self.root = None;
                    break;
                } else if node.colour == Colour::Red {
                    // can just remove
                    node.unlink_parent();
                    break;
                } else {
                    // rebalance
                    node.unlink_parent();
                    let mut current = node;
                    loop {
                        if let Some(parent) = current.parent {
                        } else {
                            // at root so exit
                            break;
                        }
                    }
                    break;
                }
            }
        }
        let res = unsafe {
            NonNull::new_unchecked(node)
        };
        res
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        let node = self.search(key);
        if let Some(node) = node {
            // remove node from tree
            {
                let node = unsafe {
                    &mut *node.as_ptr()
                };
                loop {
                    if let Some(left) = node.left {
                        if let Some(right) = node.right {

                        } else {
                            if let Some(parent) = node.parent {

                            }
                        }
                    }
                }
            }
            let node = unsafe {
                Box::from_raw(node.as_ptr())
            };
            Some(node.value)
        } else {
            None
        }
    }
}

impl<'a, K: Ord, V> IntoIterator for &'a RBTree<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        Iter {
            node_iter: NodeIter { node: self.smallest },
            _phantom: PhantomData
        }
    }
}

impl<'a, K: Ord, V> IntoIterator for &'a mut RBTree<K, V> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = IterMut<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        IterMut {
            node_iter: NodeIter { node: self.smallest },
            _phantom: PhantomData
        }
    }
}
