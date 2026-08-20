#define PASS(T,N) struct T N(struct T x) { return x; }
struct Tail { int x; char y; }; PASS(Tail,tail)
struct Three { char x,y,z; }; PASS(Three,three)
struct ThreeF { float x,y,z; }; PASS(ThreeF,threef)
struct __attribute__((aligned(16))) PadF { float x; }; PASS(PadF,padf)
struct __attribute__((packed)) Packed { char x; double y; }; PASS(Packed,packed)
struct __attribute__((aligned(16))) Pad2F { float x,y; }; PASS(Pad2F,pad2f)
struct __attribute__((aligned(16))) Wide { long long x; }; PASS(Wide,wide)
struct ThreeD { double x,y,z; }; PASS(ThreeD,threed)
struct FiveD { double x,y,z,w,v; }; PASS(FiveD,fived)
struct TwoI { long long x,y; }; PASS(TwoI,twoi)
struct ThreeF exf(float a,float b,float c,float d,float e,float f,float g, struct ThreeF x, float h) { return x; }
struct TwoI exi(long long a,long long b,long long c,long long d,long long e,long long f,long long g,struct TwoI x,long long h) { return x; }
