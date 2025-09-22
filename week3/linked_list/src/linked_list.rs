use std::fmt;
use std::option::Option;

pub struct LinkedList<T> {
    head: Option<Box<Node<T>>>,
    size: usize,
}

struct Node<T> {
    value: T,
    next: Option<Box<Node<T>>>,
}

impl<T> Node<T> {
    pub fn new(value: T, next: Option<Box<Node<T>>>) -> Node<T> {
        Node {value: value, next: next}
    }
}

impl<T> LinkedList<T> {
    pub fn new() -> LinkedList<T> {
        LinkedList::<T> {head: None, size: 0}
    }
    
    pub fn get_size(&self) -> usize {
        self.size
    }
    
    pub fn is_empty(&self) -> bool {
        self.get_size() == 0
    }
    
    pub fn push_front(&mut self, value: T) {
        let new_node: Box<Node<T>> = Box::new(Node::<T>::new(value, self.head.take()));
        self.head = Some(new_node);
        self.size += 1;
    }
    
    pub fn pop_front(&mut self) -> Option<T> {
        let node: Box<Node<T>> = self.head.take()?;
        self.head = node.next;
        self.size -= 1;
        Some(node.value)
    }
}


impl<T: fmt::Display> fmt::Display for LinkedList<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut current: &Option<Box<Node<T>>> = &self.head;
        let mut result = String::new();
        loop {
            match current {
                Some(node) => {
                    result = format!("{} {}", result, node.value);
                    current = &node.next;
                },
                None => break,
            }
        }
        write!(f, "{}", result)
    }
}

impl<T> Drop for LinkedList<T> {
    fn drop(&mut self) {
        let mut current = self.head.take();
        while let Some(mut node) = current {
            current = node.next.take();
        }
    }
}

impl<T: Clone> Clone for Node<T> {
    fn clone(&self) -> Self {
        Node::<T> {
            value: self.value.clone(), 
            next: self.next.clone()
        }
    }
}

impl<T: Clone> Clone for LinkedList<T> {
    fn clone(&self) -> Self {
        LinkedList::<T> {
            head: self.head.clone(),
            size: self.size.clone(),
        }
    }
}

impl<T: PartialEq> PartialEq for LinkedList<T> {
    fn eq(&self, other: &Self) -> bool {
        let mut a = &self.head;
        let mut b = &other.head;
        loop {
            match (a.as_ref(), b.as_ref()) {
                (Some(a_node), Some(b_node)) => {
                    if a_node.value != b_node.value {
                        return false;
                    }
                    a = &a_node.next;
                    b = &b_node.next;
                }
                (None, None) => return true,
                _ => return false,
            }
        }
    }
}

pub struct LinkedListIterator<T> {
    current: Option<Box<Node<T>>>,
}

pub struct LinkedListIter<'a, T> {
    current: &'a Option<Box<Node<T>>>,
}

impl<T> Iterator for LinkedListIterator<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        let mut node = self.current.take()?;
        self.current = node.next.take();
        Some(node.value)
    }
}

impl<T: Clone> Iterator for LinkedListIter<'_, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        match self.current {
            Some(node) => {
                self.current = &node.next;
                Some(node.value.clone())
            },
            None => None,
        }
    }
}

impl<T> IntoIterator for LinkedList<T> {
    type Item = T;
    type IntoIter = LinkedListIterator<T>;

    fn into_iter(mut self) -> LinkedListIterator<T> {
        LinkedListIterator::<T> {
            current: self.head.take()
        }
    }
}

impl<'a, T: Clone> IntoIterator for &'a LinkedList<T> {
    type Item = T;
    type IntoIter = LinkedListIter<'a, T>;

    fn into_iter(self) -> LinkedListIter<'a, T> {
        LinkedListIter { current: &self.head }
    }
}
