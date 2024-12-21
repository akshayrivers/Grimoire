use std::collections::{HashMap, BinaryHeap};
use std::cmp::Ordering;
use std::fs;

#[derive(Debug, Eq, PartialEq,Clone)]
struct Node {
    freq: usize,
    char: Option<char>,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        other.freq.cmp(&self.freq) // Reverse for min-heap
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn build_frequency_table(text: &str) -> HashMap<char, usize> {
    let mut freq_table = HashMap::new();
    for c in text.chars() {
        *freq_table.entry(c).or_insert(0) += 1;
    }
    freq_table
}

fn build_huffman_tree(freq_table: HashMap<char, usize>) -> Option<Box<Node>> {
    let mut heap = BinaryHeap::new();

    for (char, freq) in freq_table {
        heap.push(Node {
            freq,
            char: Some(char),
            left: None,
            right: None,
        });
    }

    while heap.len() > 1 {
        let left = heap.pop().unwrap();
        let right = heap.pop().unwrap();
        let parent = Node {
            freq: left.freq + right.freq,
            char: None,
            left: Some(Box::new(left)),
            right: Some(Box::new(right)),
        };
        heap.push(parent);
    }
    heap.pop().map(Box::new)
}

fn generate_codes(node: &Option<Box<Node>>, prefix: String, codes: &mut HashMap<char, String>) {
    if let Some(n) = node {
        if let Some(c) = n.char {
            codes.insert(c, prefix);
        } else {
            generate_codes(&n.left, format!("{}0", prefix), codes);
            generate_codes(&n.right, format!("{}1", prefix), codes);
        }
    }
}

fn encode_data(text: &str, codes: &HashMap<char, String>) -> String {
    text.chars().map(|c| codes.get(&c).unwrap().clone()).collect()
}

fn main() -> std::io::Result<()> {
    // Step 1: Read the input text file
    let text = fs::read_to_string("src/input.txt")?;

    // Step 2: Build the frequency table
    let freq_table = build_frequency_table(&text);

    // Step 3: Build the Huffman tree
    let huffman_tree = build_huffman_tree(freq_table).expect("Failed to build tree");

    // Step 4: Generate Huffman codes
    let mut codes = HashMap::new();
    generate_codes(&Some(huffman_tree.clone()), String::new(), &mut codes);

    // Step 5: Encode the data
    let encoded_data = encode_data(&text, &codes);

    // Step 6: Write the encoded data to a file
    fs::write("encoded.txt", encoded_data)?;

    println!("Compression complete! Encoded data saved to 'encoded.txt'.");
    Ok(())
}
