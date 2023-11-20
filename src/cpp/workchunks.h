#ifndef WORKCHUNKS_H
#include "puzdef.h"
/*
 *   Sometimes we want to split a search tree among threads.  This
 *   routine calculates the work chunks and parcels them out as
 *   needed.
 */
extern int randomstart;
vector<ull> makeworkchunks(const puzdef &pd, int d, const setval symmreduce,
                           int microthreadcounts = 1);
#define WORKCHUNKS_H
#endif
