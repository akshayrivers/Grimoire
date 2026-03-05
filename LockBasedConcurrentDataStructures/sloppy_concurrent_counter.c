// So we are building and measuring 3 types of counters here and how do they perform 
// 1. A simple counter which counts up to a million 
// 2. A counter which supports multithreadind using locks
// 3. A sloppy counter using local and global threads

#include <stdio.h>
#include <pthread.h>
#include "measurement.h"
#include <math.h>

#define NUM_THREADS_MAX 16
#define NUMCPUS 8
#define TOTAL_OPS 1000000

typedef struct __counter_t{
    int global;// global count
    pthread_mutex_t glock;// global lock
    int local[NUMCPUS];//local count (per cpu)
    pthread_mutex_t llock[NUMCPUS];// ... and locks
    int threshold;// update frequency 
} counter_t;

counter_t counter;

// init: record threshold, init locks, init values of all local counts and global count
void init(counter_t *c , int threshold){
    c->threshold = threshold;
    c->global = 0;

    pthread_mutex_init(&c->glock,NULL);

    for(int i=0;i<NUMCPUS;i++){
        c->local[i] = 0;
        pthread_mutex_init(&c->llock[i],NULL);
    }
}

// update : we just grab the local lock and update local amount once local count has risen
// by 'threshold', we grab global locak and transfer local values to it 
void update(counter_t *c, int threadID,int amt){
    int cpu = threadID % NUMCPUS;

    pthread_mutex_lock(&c->llock[cpu]);

    c->local[cpu] += amt;

    if(c->local[cpu] >= c->threshold){
        pthread_mutex_lock(&c->glock);

        c->global += c->local[cpu];

        pthread_mutex_unlock(&c->glock);

        c->local[cpu] = 0;
    }

    pthread_mutex_unlock(&c->llock[cpu]);
}

// get : we just return the global amount (whcich ofc may not be perfect)
int get(counter_t *c) {

    pthread_mutex_lock(&c->glock);
    int val = c->global;
    pthread_mutex_unlock(&c->glock);

    for(int i = 0; i < NUMCPUS; i++)
        val += c->local[i];

    return val;
}
int get_global(counter_t *c) {

    pthread_mutex_lock(&c->glock);
    int val = c->global;
    pthread_mutex_unlock(&c->glock);

    return val;
}

typedef struct {
    int tid;
    int ops;
} thread_arg;

void* worker(void* arg) {

    thread_arg *t = (thread_arg*)arg;

    for(int i = 0; i < t->ops; i++)
        update(&counter, t->tid, 1);

    return NULL;
}

int main() {

    pthread_t threads[NUM_THREADS_MAX];
    thread_arg args[NUM_THREADS_MAX];

    for(int i = 0; i < 5; i++){

        int threshold = (int)pow(8,i);

        printf("\n===== threshold = %d =====\n", threshold);

        for(int t = 1; t <= NUM_THREADS_MAX; t *= 2) {

            init(&counter, threshold);   // reseting counter for each run

            int ops_per_thread = TOTAL_OPS / t;

            double start = now();

            for(int j = 0; j < t; j++) {
                args[j].tid = j;
                args[j].ops = ops_per_thread;
                pthread_create(&threads[j], NULL, worker, &args[j]);
            }

            for(int j = 0; j < t; j++)
                pthread_join(threads[j], NULL);

            double end = now();
        int real = get(&counter);
        int global = get_global(&counter);
        int error = real - global;

        printf("%d,%d,%d,%d,%d,%f\n",
            threshold, t, real, global, error, time_diff(start,end));
                }
    }

    return 0;
}