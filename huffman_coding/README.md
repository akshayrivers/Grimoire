# Huffman Coding

Huffman coding is a lossless data compression algorithm that reduces data size by assigning variable-length codes to characters based on their frequency. More frequent characters get shorter codes while less frequent ones get longer codes.

## Step-by-Step Example: `"abacabad"`

### Step 1: Count the Frequency of Characters
In the string `"abacabad"`, the character frequencies are:
```
a: 4 times
b: 2 times
c: 1 time
d: 1 time
```

### Step 2: Build the Huffman Tree
We build a binary tree using a min-heap priority queue, combining the least frequent nodes until we have a single tree.

Initial Priority Queue (ordered by frequency):
```
c: 1
d: 1
b: 2
a: 4
```

Building process:
1. Combine `c(1)` and `d(1)` → New node `cd(2)`
2. Combine `cd(2)` and `b(2)` → New node `bcd(4)`
3. Combine `bcd(4)` and `a(4)` → Root node `abcd(8)`

The correct tree structure:
```
     (8)
    /   \
   /     \
 (4)     (4)
  |      /  \
  a    (2)   b
      /   \
     c     d
```

### Step 3: Assign Binary Codes
Traverse the tree to assign codes:
- Going left adds '0'
- Going right adds '1'

Resulting codes:
```
a: 0
b: 11
c: 100
d: 101
```

### Step 4: Encode the Data
Encoding `"abacabad"`:
```
Original: a   b   a   c   a   b   a   d
Encoded:  0   11  0   100 0   11  0   101
```

Combined encoded string:
```
011010001101101
```

### Step 5: Decoding Process
To decode, read bits and traverse the tree:
1. Read '0' → Found 'a'
2. Read '11' → Found 'b'
3. Read '0' → Found 'a'
4. Read '100' → Found 'c'
5. Read '0' → Found 'a'
6. Read '11' → Found 'b'
7. Read '0' → Found 'a'
8. Read '101' → Found 'd'

Result: "abacabad"

## Space Savings Analysis

- Original data (8 characters × 8 bits): 64 bits
- Compressed data: 15 bits
- Compression ratio: 76.6% reduction

Note: In practice, we would also need to store the Huffman tree structure or frequency table for decoding, which affects the actual compression ratio for small inputs like this example.

## Implementation Considerations

1. **Priority Queue**: A min-heap is typically used to efficiently find the two nodes with lowest frequencies.
2. **Tree Storage**: The tree structure needs to be transmitted along with the compressed data.
3. **Padding**: The compressed bit string often needs padding to fit byte boundaries.
4. **Headers**: Real implementations include headers with metadata about the compression.

## Common Applications

1. Text compression
2. File compression (part of algorithms like ZIP)
3. Data transmission
4. Image and video compression (as part of larger compression schemes)

## Time Complexity

- Building frequency table: O(n)
- Building Huffman tree: O(k log k) where k is the number of unique characters
- Encoding: O(n)
- Decoding: O(n)

## Space Complexity

- Frequency table: O(k)
- Huffman tree: O(k)
- Encoded data: O(n)

Where:
- n is the length of the input string
- k is the number of unique characters