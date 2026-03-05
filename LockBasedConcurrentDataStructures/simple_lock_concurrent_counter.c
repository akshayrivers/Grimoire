// So we are building and measuring 3 types of counters here and how do they perform 
// 1. A simple counter which counts up to a million 
// 2. A counter which supports multithreadind using locks
// 3. A sloppy counter using local and global threads

#include <stdio.h>
#include <pthread.h>
#include "measurement.h"

#define NUM_THREADS_MAX 16
#define OPS_PER_THREAD 1000000

long counter = 0;
pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;

void* worker(void* arg) {

    for(int i = 0; i < OPS_PER_THREAD; i++) {

        pthread_mutex_lock(&lock);
        counter++;
        pthread_mutex_unlock(&lock);

    }

    return NULL;
}

int main() {

    pthread_t threads[NUM_THREADS_MAX];

    for(int t = 1; t <= NUM_THREADS_MAX; t *= 2) {

        counter = 0;

        double start = now();

        for(int i = 0; i < t; i++)
            pthread_create(&threads[i], NULL, worker, NULL);

        for(int i = 0; i < t; i++)
            pthread_join(threads[i], NULL);

        double end = now();

        printf("threads=%d counter=%ld time=%f\n", t, counter, time_diff(start,end));
    }

    return 0;
}
