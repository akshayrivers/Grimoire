/*
LSM Implementation in Rust
LSM- Log structured merge tree is a Self balancing tree like it can be Red-Black Tree , AVL Tree etc.
we will implement LSM using AVL Tree.
Basic design of AVL Tree:
1. It is a self balancing binary search tree.(It is a height balanced binary search tree)
2. The balance factor of a node is the height of the right subtree minus the height of the left subtree.It must be in the range of -1, 0, 1.
   [ balanceFactor = height(rightSubTree) - height(leftSubTree) ]
*/
// MemTable  
struct MemTable {

}

struct Node {

}
fn insert() {

}
fn delete() {

}

fn search() {

}
fn balance() {

}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert() {
        let mut memtable = MemTable::new();
        memtable.insert(1, "one");
        memtable.insert(2, "two");
        memtable.insert(3, "three");
        memtable.insert(4, "four");
        memtable.insert(5, "five");
        memtable.insert(6, "six");
        memtable.insert(7, "seven");
        memtable.insert(8, "eight");
        memtable.insert(9, "nine");
        memtable.insert(10, "ten");
        assert_eq!(memtable.search(1), Some("one"));
        assert_eq!(memtable.search(2), Some("two"));
        assert_eq!(memtable.search(3), Some("three"));
        assert_eq!(memtable.search(4), Some("four"));
        assert_eq!(memtable.search(5), Some("five"));
        assert_eq!(memtable.search(6), Some("six"));
        assert_eq!(memtable.search(7), Some("seven"));
        assert_eq!(memtable.search(8), Some("eight"));
        assert_eq!(memtable.search(9), Some("nine"));
        assert_eq!(memtable.search(10), Some("ten"));
    }

    #[test]
    fn test_delete() {
        let mut memtable = MemTable::new();
        memtable.insert(1, "one");
        memtable.insert(2, "two");
        memtable.insert(3, "three");
        memtable.insert(4, "four");
        memtable.insert(5, "five");
        memtable.insert(6, "six");
        memtable.insert(7, "seven");
        memtable.insert(8, "eight");
        memtable.insert(9, "nine");
        memtable.insert(10, "ten");
        memtable.delete(1);
        memtable.delete(2);
        memtable.delete(3);
        memtable.delete(4);
        memtable.delete(5);
        memtable.delete(6);
        memtable.delete(7);
        memtable.delete(8);
        memtable.delete(9);
        memtable.delete(10);
        assert_eq!(memtable.search(1), None);
        assert_eq!(memtable.search(2), None);
        assert_eq!(memtable.search(3), None);
    }
}
fn main() {
    println!("Implementaion of basic LSM");
}
