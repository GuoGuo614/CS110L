use linked_list::LinkedList;
pub mod linked_list;

fn main() {
    let list0: LinkedList<String> = LinkedList::new();
    let mut list = list0.clone();
    println!("{}", list0.eq(&list));
    assert!(list.is_empty());
    assert_eq!(list.get_size(), 0);
    for i in 1..12 {
        list.push_front(i.to_string());
    }
    for i in &list {
        print!("{} ", i);
    }
    println!("{}", list);
    println!("list size: {}", list.get_size());
    println!("top element: {}", list.pop_front().unwrap());
    println!("{}", list);
    println!("size: {}", list.get_size());
    println!("{}", list.to_string()); // ToString impl for anything impl Display
    println!("{}", list0.eq(&list));

    // If you implement iterator trait:
    //for val in &list {
    //    println!("{}", val);
    //}
}
