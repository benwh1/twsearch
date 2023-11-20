#ifndef CMDLINEOPS_H
#include "generatingset.h"
#include "prunetable.h"
#include "puzdef.h"
#include "twsearch.h"
#include <cstdio>
#include <functional>
#include <set>
/*
 *   The twsearch program also includes a number of utility operations,
 *   such as uniquifying a set of positions.  These routines support
 *   streaming a sequence of positions through various operations.
 */
void solvecmdline(puzdef &pd, const char *scr, generatingset *gs);
extern vector<loosetype> uniqwork;
extern set<vector<loosetype>> uniqseen;
void uniqit(const puzdef &pd, setval p, const char *s);
void wrongit(const puzdef &pd, setval p, const char *s);
void uniqitsymm(const puzdef &pd, setval p, const char *s);
void invertit(const puzdef &pd, vector<int> &v, const char *s);
void cancelit(const puzdef &pd, vector<int> &v, const char *s);
void mergeit(const puzdef &pd, vector<int> &v, const char *s);
void unrotateit(const puzdef &pd, vector<int> &v, const char *s);
void shortenit(const puzdef &pd, vector<int> &v, const char *s);
void symsit(const puzdef &pd, setval p, const char *s);
void orderit(const puzdef &pd, setval p, const char *s);
void emitmove(const puzdef &pd, setval p, const char *s);
void emitposition(const puzdef &pd, setval p, const char *s);
void showrandompos(const puzdef &pd);
void processlines(const puzdef &pd,
                  function<void(const puzdef &, setval, const char *)> f);
void processlines2(const puzdef &pd,
                   function<void(const puzdef &, setval, const char *)> f);
void processlines3(
    const puzdef &pd,
    function<void(const puzdef &, vector<int> &v, const char *)> f);
void processlines4(
    const puzdef &pd,
    function<void(const puzdef &, vector<int> &v, const char *)> f);
extern int compact;
#define CMDLINEOPS_H
#endif
