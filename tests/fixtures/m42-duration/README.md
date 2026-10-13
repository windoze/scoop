# Duration verification

`arithmetic` contains 60 addition/subtraction pairs and 80 multiplication/division
pairs (280 operation expectations), checked in both debug and release profiles.
`tests/generate_duration_vectors.py` calculates all expectations as Python integer
nanoseconds, checks the complete Duration range, and uses explicit truncation toward
zero for division. It does not copy the library's seconds/fraction arithmetic.
The generated Scoop sources are committed; formal fixtures require neither Python
vector generation nor an external time library.

The cases include both representation endpoints, cancellation across signed limits,
negative fractions, zero divisors, `Long.MIN_VALUE` scaling, and deterministic samples
throughout the range. Separate fixtures cover normalization, unit conversion overflow,
canonical ISO text, collection integration, and moving GC.
