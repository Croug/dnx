use std::{marker::PhantomData, collections::HashMap, hash::Hash};

use hickory_server::proto::rr::{LowerName, Name};
use serde::{Serialize, Deserialize};

pub trait TreeSortable<T: PartialEq> {
    fn get_path(&self) -> Vec<T>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tree<V, T>
where
    V: Eq + Hash,
    T: TreeSortable<V>
{
    root: TreeNode<V, T>,
}

impl<V, T> Tree<V, T>
where
    V: Eq + Hash,
    T: TreeSortable<V>
{
    pub fn new() -> Self {
        Tree {
            root: TreeNode {
                value: None,
                children: HashMap::new(),
                _phantom: PhantomData,
            },
        }
    }

    pub fn insert(&mut self, value: T) -> Option<T> {
        self.root.add_child(value, None)
    }

    pub fn find<U>(&self, path: U) -> Option<&T>
    where
        U: TreeSortable<V>
    {
        let path = path.get_path();
        self.root.find(path)
    }

    pub fn get<U>(&self, path: U) -> Option<&T>
    where
        U: TreeSortable<V>
    {
        let path = path.get_path();
        self.root.get(path)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct TreeNode<V, T> 
where
    V: Eq + Hash,
    T: TreeSortable<V>
{
    value: Option<T>,
    children: HashMap<V, TreeNode<V, T>>,
    _phantom: PhantomData<V>,
}

impl<V, T> TreeNode<V, T>
where
    V: Eq + Hash,
    T: TreeSortable<V>
{
    fn add_child(&mut self, value: T, path: Option<Vec<V>>) -> Option<T> {
        let mut path = path.unwrap_or_else(|| value.get_path());

        if path.is_empty() {
            let last = self.value.take();
            self.value = Some(value);
            return last;
        }

        let next = path.pop().unwrap();
        self.children.entry(next).or_insert_with(|| {
            TreeNode {
                value: None,
                children: HashMap::new(),
                _phantom: PhantomData,
            }
        }).add_child(value, Some(path))
    }

    fn find(&self, mut path: Vec<V>) -> Option<&T> {
        if path.is_empty() {
            return self.value.as_ref();
        }

        let next = path.pop().unwrap();
        let next = self.children.get(&next);

        match next {
            None => self.value.as_ref(),
            Some(next) => next.find(path),
        }
    }

    fn get(&self, mut path: Vec<V>) -> Option<&T> {
        if path.is_empty() {
            return self.value.as_ref();
        }

        let next = path.pop().unwrap();
        self.children.get(&next)?.get(path)
    }
}

impl TreeSortable<String> for &str {
    fn get_path(&self) -> Vec<String> {
        self.split(".").map(|s| s.to_owned()).collect()
    }
}

impl TreeSortable<String> for String {
    fn get_path(&self) -> Vec<String> {
        self.split(".").map(|s| s.to_owned()).collect()
    }
}

impl TreeSortable<String> for &LowerName {
    fn get_path(&self) -> Vec<String> {
        self.to_string().get_path()
    }
}

impl TreeSortable<String> for &Name {
    fn get_path(&self) -> Vec<String> {
        self.to_string().get_path()
    }
}
