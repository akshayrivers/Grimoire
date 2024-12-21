use std::collections::{HashMap, BinaryHeap};
use std::cmp::Ordering;
use std::fs;

#[derive(Debug, Eq, PartialEq, Clone)]
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

fn encode_data(text: &str, codes: &HashMap<char, String>) -> Vec<u8> {
    let mut encoded_bits = String::new();
    
    // Create the bit string
    for c in text.chars() {
        encoded_bits.push_str(codes.get(&c).unwrap());
    }
    
    // Convert bit string to bytes
    let mut encoded_bytes = Vec::new();
    let mut byte = 0u8;
    let mut bit_count = 0;

    for (_i, bit) in encoded_bits.chars().enumerate() {
        if bit == '1' {
            byte |= 1 << (7 - bit_count);
        }
        bit_count += 1;

        // Once we have 8 bits, push the byte to the vector and reset
        if bit_count == 8 {
            encoded_bytes.push(byte);
            byte = 0;
            bit_count = 0;
        }
    }
    
    // Handle leftover bits
    if bit_count > 0 {
        encoded_bytes.push(byte);
    }

    encoded_bytes
}

fn decode_data(encoded: &Vec<u8>, root: &Option<Box<Node>>) -> String {
    let mut decoded = String::new();
    let mut current_node = root;
    let mut bit_iter = encoded.iter().flat_map(|&byte| (0..8).map(move |i| (byte >> (7 - i)) & 1));

    while let Some(bit) = bit_iter.next() {
        if let Some(node) = current_node {
            current_node = match bit {
                0 => &node.left,
                1 => &node.right,
                _ => panic!("Invalid bit in encoded string"),
            };

            // If we reach a leaf node, append the character to the result
            if let Some(n) = current_node {
                if let Some(c) = n.char {
                    decoded.push(c);
                    current_node = root; // Reset to root for the next character
                }
            }
        } else {
            panic!("Invalid traversal in Huffman tree");
        }
    }

    decoded
}

fn main() -> std::io::Result<()> {
    //For encoding the data
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
        fs::write("src/encoded.txt", encoded_data)?;

        println!("Compression complete! Encoded data saved to 'encoded.txt'.");
    //For decoding data
        // Step 1: Read the encoded data
        let encoded_data = fs::read("src/encoded.txt")?;

    
        // Step 2: Decode the data
        let decoded_data = decode_data(&encoded_data, &Some(huffman_tree));
    
        // Step 3: Write the decoded data to a file
        fs::write("src/decoded.txt", decoded_data)?;
    
        println!("Decoding complete! Decoded data saved to 'decoded.txt'.");
        Ok(())
}
