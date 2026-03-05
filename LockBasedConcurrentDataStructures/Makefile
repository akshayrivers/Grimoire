CC = gcc
CFLAGS = -Wall -O2 -pthread

TARGETS = simple_counter sloppy_counter hand_over_hand skip_list

all: $(TARGETS)

simple_counter: simple_lock_concurrent_counter.c measurement.c
	$(CC) $(CFLAGS) simple_lock_concurrent_counter.c measurement.c -o simple_counter

sloppy_counter: sloppy_concurrent_counter.c measurement.c
	$(CC) $(CFLAGS) sloppy_concurrent_counter.c measurement.c -o sloppy_counter

hand_over_hand: hand_over_hand_Locking_LL.c measurement.c
	$(CC) $(CFLAGS) hand_over_hand_Locking_LL.c measurement.c -o hand_over_hand

skip_list: skip_list.c measurement.c
	$(CC) $(CFLAGS) skip_list.c measurement.c -o skip_list

clean:
	rm -f $(TARGETS)