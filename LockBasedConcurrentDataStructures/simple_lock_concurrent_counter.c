// So we are building and measuring 3 types of counters here and how do they perform 
// 1. A simple counter which counts up to a million 
// 2. A counter which supports multithreadind using locks
// 3. A sloppy counter using local and global threads

#include <stdio.h>
#include <pthread.h>
#include "measurement.h"

#define NUM_THREADS_MAX 64
static int TOTAL_OPS = 1000000;

long counter = 0;
pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
typedef struct {
    int tid;
    int ops;
} thread_arg;

void* worker(void* arg) {
    thread_arg *t=(thread_arg*) arg;
    for(int i = 0; i < t->ops; i++) {

        pthread_mutex_lock(&lock);
        counter++;
        pthread_mutex_unlock(&lock);

    }

    return NULL;
}

int main() {

    pthread_t threads[NUM_THREADS_MAX];
    thread_arg args[NUM_THREADS_MAX];

    for(int t = 1; t <= NUM_THREADS_MAX; t *= 2) {
        int ops = (int)(TOTAL_OPS/t);
        counter = 0;

        double start = now();

        for(int i = 0; i < t; i++){
            args[i].tid=i;
            args[i].ops = ops;
            pthread_create(&threads[i], NULL, worker, &args[i]);
        }
            

        for(int i = 0; i < t; i++)
            pthread_join(threads[i], NULL);

        double end = now();

        printf("threads=%d counter=%ld time=%f\n", t, counter, time_diff(start,end));
    }

    return 0;
}
