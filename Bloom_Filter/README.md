>A bloom filter is a probablistic data structure that answers a very specific question: Have I seen this thing before? using very little memory.

A Bloom filter can always correctly tell you when something is not present. However, when it says something might be present, it can be wrong, a false positive. Therefore the key property is:

- Definitely not present → guaranteed correct.
- Possibly present → might be a false positive; you need to check the actual storage.

For example, if a Bloom filter says "manglu" is not present, then "manglu" definitely wasn't added. If it says "manglu" might be present, it could be there - or the filter could have produced a false positive.

Now before getting in much more details I would like to thank Mr. Arpit Bhayani for his amazing blog explaining Bloom Filters do check it out here https://arpitbhayani.me/blogs/bloom-filters/

Bloom filters are so simple yet so cool.
So, Ultimately they have two fundamental structures: 
1. Bit Array of size m
2. K hash functions to get the positions for each unique input 

Now I will not go deep into the probaility mathematics, you can read that in the blog.
What I will be focusing in here is the execution of bloom filters.

We have multiple non cryotographic hashing techniques we can use: 
1. xxHash
2. MurmurHash3
3. FNV(Fowler Noll vo)

And we will be using Kirsch Mitzenmacher optimization or double hashing optimisation which only uses the 2 hash functions to derive all the k positions using a linear combination
```
g_i(x) = h1(x) + i * h2(x)  for i = 0, 1, ..., k-1
```
And we will also be using the enhanced version for more accuracy as shown in the blog

I am including the scope to make the Bloom filter Deleteable too
Specifically The Deleteable Bloom filter (probabilistic deletability).
```
                 Deletable Bloom Filter
                         │
             ┌───────────┴───────────┐
             │                       │
        Bloom bit array         Collision bitmap
             │                       │
          m bits                   r bits
             │                       │
       ┌─────┴─────┐          region collision
       │           │             tracking
       ▼           ▼
    hash        positions
       │
       ▼
 h1(x) + i × h2(x)
```

Benchmarks will be added as mentioned in the blog. 