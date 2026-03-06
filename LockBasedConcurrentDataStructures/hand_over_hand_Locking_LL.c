// SO there are a few places where race conditions can happen over a Linked List such as insert, update, delete etc
// Our objective here would be write and compare two different implementation of locking strategies

#include <stdio.h>
#include <pthread.h>
#include <stdlib.h>
#include "measurement.h"

#define NUM_THREADS_MAX 16
#define TOTAL_OPS 20000
#define PREPOP_SIZE 1000


// Approach I [coarse-grained locking] We Lock the full list no matter what expression so we define the lock over the entire linked list


// basic node struct
typedef struct __node_t{
    int key;
    struct __node_t *next;
} node_t;


// basic list struct
typedef struct __list_t{
    node_t *head;
    pthread_mutex_t lock;
} list_t;


void list_init(list_t *L){
    L->head = NULL;
    pthread_mutex_init(&L->lock,NULL);
}


// we lock the whole list for insert
int list_insert(list_t *L,int key){

    // malloc can happen before as it is assumed to be thread safe
    node_t* new = malloc(sizeof(node_t));
    if(new == NULL){
        perror("malloc");
        return -1;
    }

    new->key = key;

    // just locking the critical section
    pthread_mutex_lock(&L->lock);

    new->next = L->head;
    L->head = new;

    pthread_mutex_unlock(&L->lock);

    return 0;
}


// to avoid dirty reads
int list_lookup(list_t *L , int key){

    int rv = -1;

    pthread_mutex_lock(&L->lock);

    node_t* curr = L->head;

    while(curr){
        if(curr->key == key){
            rv = 0;
            break;
        }
        curr = curr->next;
    }

    pthread_mutex_unlock(&L->lock);

    return rv;
}



// Approach II [fine-grained locking] : Here each node carries its own lock (hand-over-hand locking)

// node struct with its own mutex
typedef struct node_fg{
    int key;
    struct node_fg *next;
    pthread_mutex_t lock;
} node_fg;


// list struct
typedef struct list_fg{
    node_fg *head;
} list_fg;


// initializing list with sentinel head node
void List_Init(list_fg *L){

    node_fg *sentinel = malloc(sizeof(node_fg));
    if(sentinel == NULL){
        perror("malloc");
        exit(1);
    }

    sentinel->key = -1;   
    sentinel->next = NULL;

    pthread_mutex_init(&sentinel->lock,NULL);

    L->head = sentinel;
}


//insert using hand-over-hand traversal
int List_Insert(list_fg *L,int key){

    node_fg *new = malloc(sizeof(node_fg));
    if(new == NULL){
        perror("malloc");
        return -1;
    }

    new->key = key;
    new->next = NULL;
    pthread_mutex_init(&new->lock,NULL);

    node_fg *prev = L->head;

    pthread_mutex_lock(&prev->lock);

    node_fg *curr = prev->next;

    if(curr != NULL)
        pthread_mutex_lock(&curr->lock);

    while(curr && curr->key < key){

        pthread_mutex_unlock(&prev->lock);

        prev = curr;
        curr = curr->next;

        if(curr != NULL)
            pthread_mutex_lock(&curr->lock);
    }

    new->next = curr;
    prev->next = new;

    if(curr != NULL)
        pthread_mutex_unlock(&curr->lock);

    pthread_mutex_unlock(&prev->lock);

    return 0;
}


// hand-over-hand traversal
int List_Lookup(list_fg *L,int key){

    node_fg *prev = L->head;

    pthread_mutex_lock(&prev->lock);

    node_fg *curr = prev->next;

    if(curr != NULL)
        pthread_mutex_lock(&curr->lock);

    while(curr){

        if(curr->key == key){
            pthread_mutex_unlock(&curr->lock);
            pthread_mutex_unlock(&prev->lock);
            return 0;
        }

        pthread_mutex_unlock(&prev->lock);

        prev = curr;
        curr = curr->next;

        if(curr != NULL)
            pthread_mutex_lock(&curr->lock);
    }

    pthread_mutex_unlock(&prev->lock);

    return -1;
}



// thread arguments
typedef struct {

    int tid;
    int ops;

    list_t *coarse_list;
    list_fg *fine_list;

    int mode; // 0 = coarse insert
              // 1 = fine insert
              // 2 = coarse lookup
              // 3 = fine lookup

} thread_arg;



void* worker(void* arg){

    thread_arg *t = (thread_arg*)arg;

    for(int i=0;i<t->ops;i++){

        int key = rand() % PREPOP_SIZE;

        if(t->mode == 0){
            list_insert(t->coarse_list,key);
        }

        else if(t->mode == 1){
            List_Insert(t->fine_list,key);
        }

        else if(t->mode == 2){
            list_lookup(t->coarse_list,key);
        }

        else{
            List_Lookup(t->fine_list,key);
        }
    }

    return NULL;
}



// prepopulating lists so lookup benchmark has constant list size
void prepopulate_coarse(list_t *L){

    for(int i=0;i<PREPOP_SIZE;i++)
        list_insert(L,i);
}


void prepopulate_fine(list_fg *L){

    for(int i=0;i<PREPOP_SIZE;i++)
        List_Insert(L,i);
}



// Insert benchmark
void run_insert_coarse(){

    printf("\n==== Coarse Locking INSERT Benchmark ====\n");

    pthread_t threads[NUM_THREADS_MAX];
    thread_arg args[NUM_THREADS_MAX];

    for(int t = 1; t <= NUM_THREADS_MAX; t*=2){

        list_t *LL = malloc(sizeof(list_t));
        list_init(LL);

        int ops_per_thread = TOTAL_OPS / t;

        double start = now();

        for(int i=0;i<t;i++){

            args[i].tid = i;
            args[i].ops = ops_per_thread;
            args[i].coarse_list = LL;
            args[i].mode = 0;

            pthread_create(&threads[i],NULL,worker,&args[i]);
        }

        for(int i=0;i<t;i++)
            pthread_join(threads[i],NULL);

        double end = now();

        printf("threads=%d time=%f\n",
                t,time_diff(start,end));
    }
}



void run_insert_fine(){

    printf("\n==== Fine Locking INSERT Benchmark ====\n");

    pthread_t threads[NUM_THREADS_MAX];
    thread_arg args[NUM_THREADS_MAX];

    for(int t = 1; t <= NUM_THREADS_MAX; t*=2){

        list_fg *LL = malloc(sizeof(list_fg));
        List_Init(LL);

        int ops_per_thread = TOTAL_OPS / t;

        double start = now();

        for(int i=0;i<t;i++){

            args[i].tid = i;
            args[i].ops = ops_per_thread;
            args[i].fine_list = LL;
            args[i].mode = 1;

            pthread_create(&threads[i],NULL,worker,&args[i]);
        }

        for(int i=0;i<t;i++)
            pthread_join(threads[i],NULL);

        double end = now();

        printf("threads=%d time=%f\n",
                t,time_diff(start,end));
    }
}



// Lookup benchmark
void run_lookup_coarse(){

    printf("\n==== Coarse Locking LOOKUP Benchmark ====\n");

    pthread_t threads[NUM_THREADS_MAX];
    thread_arg args[NUM_THREADS_MAX];

    list_t *LL = malloc(sizeof(list_t));
    list_init(LL);

    prepopulate_coarse(LL);

    for(int t = 1; t <= NUM_THREADS_MAX; t*=2){

        int ops_per_thread = TOTAL_OPS / t;

        double start = now();

        for(int i=0;i<t;i++){

            args[i].tid = i;
            args[i].ops = ops_per_thread;
            args[i].coarse_list = LL;
            args[i].mode = 2;

            pthread_create(&threads[i],NULL,worker,&args[i]);
        }

        for(int i=0;i<t;i++)
            pthread_join(threads[i],NULL);

        double end = now();

        printf("threads=%d time=%f\n",
                t,time_diff(start,end));
    }
}



void run_lookup_fine(){

    printf("\n==== Fine Locking LOOKUP Benchmark ====\n");

    pthread_t threads[NUM_THREADS_MAX];
    thread_arg args[NUM_THREADS_MAX];

    list_fg *LL = malloc(sizeof(list_fg));
    List_Init(LL);

    prepopulate_fine(LL);

    for(int t = 1; t <= NUM_THREADS_MAX; t*=2){

        int ops_per_thread = TOTAL_OPS / t;

        double start = now();

        for(int i=0;i<t;i++){

            args[i].tid = i;
            args[i].ops = ops_per_thread;
            args[i].fine_list = LL;
            args[i].mode = 3;

            pthread_create(&threads[i],NULL,worker,&args[i]);
        }

        for(int i=0;i<t;i++)
            pthread_join(threads[i],NULL);

        double end = now();

        printf("threads=%d time=%f\n",
                t,time_diff(start,end));
    }
}

int main(){

    run_insert_coarse();
    run_insert_fine();

    run_lookup_coarse();
    run_lookup_fine();

    return 0;
}