// To use gettimeofday(), one must include the <sys/time.h> header file. 

// int gettimeofday(struct timeval *when, void *not_used);
// The function takes two arguments:
// when: A pointer to a struct timeval where the function stores the time.
// not_used: This argument is obsolete and should be specified as NULL. 

// The struct timeval is defined as:

// struct timeval {
//     time_t       tv_sec;   /* seconds since the Unix Epoch (Jan 1, 1970 UTC) */
//     suseconds_t tv_usec;  /* microseconds (0 to 999999) */
// };
// The function returns 0 on success, or -1 if an error occurs (and errno is set)


#include <sys/time.h>
#include "measurement.h"
#include <stdio.h>

double now() {
    struct timeval tv;
    gettimeofday(&tv, NULL);

    return tv.tv_sec + tv.tv_usec / 1000000.0;
}
#include <stdio.h>
#include <time.h>

void print_time() {

    struct timeval tv;
    gettimeofday(&tv, NULL);

    time_t curtime = tv.tv_sec;

    char buffer[30];
    strftime(buffer, 30, "%m-%d-%Y %T", localtime(&curtime));

    printf("Formatted time: %s.%06d\n", buffer, tv.tv_usec);
}

double time_diff(double start, double end) {
    return end - start;
}

// int main(){
//     print_time();
//     return 0;
// }