extern int missing(int);
int common;
const int readonly = 7;
_Thread_local int tls = 9;
__declspec(selectany) int merged = 3;
int function(void) { return missing(tls + merged + common + readonly); }
