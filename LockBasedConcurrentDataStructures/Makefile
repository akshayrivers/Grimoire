CC      = gcc
CFLAGS  = -Wall -Wextra -pthread
LDFLAGS = -lm

MEASURE = measurement.c

TARGETS = simple_counter sloppy_counter hand_over_hand skip_list

TSAN_TARGETS = $(addsuffix _tsan, $(TARGETS))
ASAN_TARGETS = $(addsuffix _asan, $(TARGETS))

SRCS = simple_lock_concurrent_counter.c \
       sloppy_concurrent_counter.c      \
       hand_over_hand_Locking_LL.c      \
       skip_list.c

# ─── default ───────────────────────────────────────────────────────────────────
.PHONY: all clean debug tsan asan help

all: $(TARGETS)

simple_counter:   simple_lock_concurrent_counter.c $(MEASURE)
	$(CC) $(CFLAGS) $(LDFLAGS) $^ -o $@

sloppy_counter:   sloppy_concurrent_counter.c $(MEASURE)
	$(CC) $(CFLAGS) $(LDFLAGS) $^ -o $@

hand_over_hand:   hand_over_hand_Locking_LL.c $(MEASURE)
	$(CC) $(CFLAGS) $(LDFLAGS) $^ -o $@

skip_list:        skip_list.c $(MEASURE)
	$(CC) $(CFLAGS) $(LDFLAGS) $^ -o $@

# ─── debug ─────────────────────────────────────────────────────────────────────
debug: CFLAGS += -g -O0 -DDEBUG
debug: clean all

# ─── tsan ──────────────────────────────────────────────────────────────────────
tsan: CFLAGS += -g -O1 -fsanitize=thread
tsan: $(TSAN_TARGETS)

%_tsan: %.c $(MEASURE)
	$(CC) $(CFLAGS) $(LDFLAGS) $^ -o $@

# hand_over_hand and skip_list have non-matching stem so we spell them out
hand_over_hand_tsan: hand_over_hand_Locking_LL.c $(MEASURE)
	$(CC) $(CFLAGS) $(LDFLAGS) $^ -o $@

simple_counter_tsan: simple_lock_concurrent_counter.c $(MEASURE)
	$(CC) $(CFLAGS) $(LDFLAGS) $^ -o $@

sloppy_counter_tsan: sloppy_concurrent_counter.c $(MEASURE)
	$(CC) $(CFLAGS) $(LDFLAGS) $^ -o $@

skip_list_tsan: skip_list.c $(MEASURE)
	$(CC) $(CFLAGS) $(LDFLAGS) $^ -o $@

# ─── asan ──────────────────────────────────────────────────────────────────────
asan: CFLAGS += -g -O1 -fsanitize=address -fno-omit-frame-pointer
asan: $(ASAN_TARGETS)

hand_over_hand_asan: hand_over_hand_Locking_LL.c $(MEASURE)
	$(CC) $(CFLAGS) $(LDFLAGS) $^ -o $@

simple_counter_asan: simple_lock_concurrent_counter.c $(MEASURE)
	$(CC) $(CFLAGS) $(LDFLAGS) $^ -o $@

sloppy_counter_asan: sloppy_concurrent_counter.c $(MEASURE)
	$(CC) $(CFLAGS) $(LDFLAGS) $^ -o $@

skip_list_asan: skip_list.c $(MEASURE)
	$(CC) $(CFLAGS) $(LDFLAGS) $^ -o $@

# ─── clean ─────────────────────────────────────────────────────────────────────
clean:
	rm -f $(TARGETS) $(TSAN_TARGETS) $(ASAN_TARGETS)

# ─── help ──────────────────────────────────────────────────────────────────────
help:
	@echo "Targets:"
	@echo "  all     — build all binaries (optimized)"
	@echo "  debug   — rebuild with -g -O0 -DDEBUG"
	@echo "  tsan    — rebuild all with ThreadSanitizer"
	@echo "  asan    — rebuild all with AddressSanitizer"
	@echo "  clean   — remove all built binaries"
	@echo ""
	@echo "Examples:"
	@echo "  make tsan && ./skip_list_tsan"
	@echo "  make asan && ./hand_over_hand_asan"