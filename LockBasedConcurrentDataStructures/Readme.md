This is my attempt at implementing the data sturctures supported in multithreaded environments and measuring their performance
drawing parallel between theoratical and practical observations
I am currently following the OSTEP Assignment on Lock-Based Concurrent Data Structures

It contains the following programs and how they are implemented along with their theoratical expectation and observed data:

1. A simple counter
2. A lock based counter
3. A sloppy counter which uses global counter to scale
4. Hand over hand Locking in Linked Lists

In Progress: 5. Skip Lists

```
Producer threads        Consumer threads
---------------        ----------------
 wait(empty)            wait(fill)
     |                      |
     v                      v

        +----------------+
        |     BUFFER     |
        +----------------+

 signal(fill)            signal(empty)

// different diagram
                 Mutex
                 |
                 v

      +-----------------------+
      |      BOUNDED BUFFER   |
      |                       |
      |  [ ][ ][ ][ ][ ]      |
      +-----------------------+
           ^             ^
           |             |

      wait(fill)    wait(empty)
      consumers     producers
```
