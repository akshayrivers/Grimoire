## Log Strcutured Merge Tree 
LSM tree, short for Log-Structured-Merge Tree, is a clever algorithm design that helps us store massive amounts of data without making us wait forever to write it. It stores data in memory first, which is lightning fast. But since we can’t keep everything in memory, the LSM Tree periodically flushes data to disk.
## How write works in LSM Tree?
When you want to write data to the LSM Tree the first stop is the in-memory layer of the LSM Tree, which is like the top part of the tree and is super fast as well, because it’s stored in memory!
However, keeping all the data in memory indefinitely is not practical, so the LSM tree periodically flushes the data from the in-memory layer to disk.
But here’s the clever part: before the data is flushed to disk and organized into SSTables, it is also written to the Write-Ahead Log (WAL).
The Write-Ahead Log acts as a backup, ensuring the durability of the data. It serves as a log of all the changes made to the database, acting as a safety net in case of system failures or crashes.


![Alt text](image.png)