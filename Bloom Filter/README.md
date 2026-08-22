>A bloom filter is a probablistic data structure that answers a very specific question: Have I seen this thing before? using very little memory.

A Bloom filter can always correctly tell you when something is not present. However, when it says something might be present, it can be wrong, a false positive. Therefore the key property is:

- Definitely not present → guaranteed correct.
- Possibly present → might be a false positive; you need to check the actual storage.

For example, if a Bloom filter says "manglu" is not present, then "manglu" definitely wasn't added. If it says "manglu" might be present, it could be there - or the filter could have produced a false positive.
