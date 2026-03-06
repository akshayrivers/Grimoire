CC = gcc
CFLAGS = -Wall -pthread
DEBUGFLAGS = -g -O0

MEASURE = measurement.c

TARGETS = simple_counter sloppy_counter hand_over_hand skip_list

all: $(TARGETS)

simple_counter:
	$(CC) $(CFLAGS) simple_lock_concurrent_counter.c $(MEASURE) -o $@

sloppy_counter:
	$(CC) $(CFLAGS) sloppy_concurrent_counter.c $(MEASURE) -o $@

hand_over_hand:
	$(CC) $(CFLAGS) hand_over_hand_Locking_LL.c $(MEASURE) -o $@

skip_list:
	$(CC) $(CFLAGS) skip_list.c $(MEASURE) -o $@

debug: CFLAGS += $(DEBUGFLAGS)
debug: clean all

clean:
	rm -f $(TARGETS)

.PHONY: all clean debug