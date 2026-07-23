# Scoop Surface Grammar — EBNF (Normative)

**Status:** NORMATIVE. This document is the single source of truth for the new frontend parser (`crates/scoop2_syntax`). It was extracted from the legacy parser and cross-checked against the language spec; the review rulings applied on top of that extraction are summarized in §14 (Changelog vs legacy parser).

**Sources (provenance / informative references):**

- `crates/scoopc_ast/src/syntax/token.rs`, `syntax/lexer.rs`, `syntax/{int,float,char}_literal.rs`
- `crates/scoopc_ast/src/parser/{mod,cursor,file,decls,expr,stmt,pattern,types}.rs` (legacy parser this grammar was extracted from)
- `crates/scoopc_ast/src/ast/mod.rs` (AST shapes)
- `SCOOP_FULL_SPEC.md` (language spec; constructs present in the spec but out of scope for this grammar are collected in §11)

**Notation:**

- `X ::= ...` production; `X?` optional; `X*` zero-or-more; `X+` one-or-more; `A | B` alternation; `(...)` grouping.
- Terminals in single quotes: `'fun'`, `'+'`. Keywords are quoted with their source spelling.
- Lexer-level rules are in CAPS (`IDENT`, `INT_LIT`, ...). See §1.
- `⌊...⌋` marks a *contextual keyword* — lexed as `IDENT`, recognized by the parser only in specific positions by source-text comparison.
- Parser-level lookahead/ambiguity rules that cannot be expressed in pure EBNF are given as **Notes** attached to each production. These notes are normative.

---

## 1. Lexical grammar

### 1.1 Trivia & statement termination

- Whitespace (any Unicode `char::is_whitespace`) is skipped. **Newlines are not tokens.** There is no automatic semicolon insertion and no line-sensitivity anywhere except one lookahead heuristic (see Note on `typeApplyExpr`, §8.4).
- Line comment: `//` ... end-of-line. Block comment: `/*` ... `*/`, **non-nested**; unterminated block comment is a lex error.
- Statements are terminated by *structure* (the expression parser stops where the grammar stops) or by an optional `';'`. A `';'` may follow any statement/declaration; stray `';'` inside blocks/type bodies/when-arm lists/handler-arm lists are consumed as empty statements or skipped.

### 1.2 Identifiers & literals

```ebnf
IDENT      ::= ('_' | ASCII_ALPHA) ('_' | ASCII_ALPHA | ASCII_DIGIT)*
                (* identifiers are ASCII-only; `_` alone is a valid IDENT *)
INT_LIT    ::= DEC_INT | HEX_INT | BIN_INT
DEC_INT    ::= DIGIT (DIGIT | '_')* INT_SUFFIX?
HEX_INT    ::= '0' ('x'|'X') (ALNUM | '_')+          (* validated: hex digits, '_' only between digits *)
BIN_INT    ::= '0' ('b'|'B') (ALNUM | '_')+          (* validated: binary digits *)
INT_SUFFIX ::= ('u'|'U') ('l'|'L')? | ('l'|'L')
FLOAT_LIT  ::= (DIGIT (DIGIT|'_')* '.' (DIGIT|'_')+ | DIGIT (DIGIT|'_')* EXP) FLOAT_SUFFIX?
EXP        ::= ('e'|'E') ('+'|'-')? (ALNUM|'_')+     (* validated: decimal digits *)
FLOAT_SUFFIX ::= 'f32' | 'f'
CHAR_LIT   ::= "'" ( ESC | ~['\\\n] ) "'"            (* exactly one char; escapes incl. \uXXXX validated *)
STRING_LIT ::= NORMAL_STR | RAW_STR
NORMAL_STR ::= '"' (ESC | ~["\\\n])* '"'             (* raw newline forbidden *)
RAW_STR    ::= '"""' ... '"""'                       (* no escape processing; may span lines *)
FSTRING    ::= 'f' (NORMAL_STR | RAW_STR)            (* single token; `${ ... }` holes split by parser *)
ESC        ::= '\' ...                               (* validated per char_literal/string rules *)
```

- Lex errors are hard failures (no recovery): invalid char, unterminated block comment/string/char, invalid char/int/float literal (bad digits, misplaced `_`, missing exponent digits, overflow).
- **Lexical quirk 1 — `as?`:** the lexer produces a single keyword token `as?` when `?` immediately follows `as`. `as ? T` is two tokens and will not parse as a safe cast.
- **Lexical quirk 2 — `.` before digits:** after `.` or `?.`, a digit-run is lexed as `INT_LIT` (never `FLOAT_LIT`), so `x.1.2` lexes as `x . 1 . 2` and `1.toString()` / `1..2` do not become float literals. Consequence: a float literal can never directly follow `.`/`?.` (see `with`-update field paths, §8.4).
- **Lexical quirk 3 — `f"..."`:** `f` immediately followed by `"` is one string token (kind = interpolated). Inside, only `${ expr }` is a hole; `$name` is **not** supported (spec §8.2 agrees). The hole expression is re-lexed/re-parsed as a sub-expression by a fresh sub-parser; `{`/`}` balance inside the hole ignores nested strings/comments/chars.
- **Booleans:** `true`/`false` are **not** literals or keywords — plain `IDENT`s (special-cased only in `when` patterns, §9.2, and resolved by typecheck). `null` likewise does not exist (nullability is `Option`).
- No octal literals, no backtick identifiers, no shebang, no `0o` prefix.

### 1.3 Keywords (hard keywords)

```
public internal private open abstract sealed inline override operator vararg annotation
package import typealias fun val var class interface struct enum effect object companion
handle on with perform try catch finally
do return if else when for in out where while break continue is as as?
```

Notes:

- `inline`, `perform` — lexed as keywords but **removed from the language**; parser emits dedicated errors (§10).
- `vararg` — keyword, used only as parameter modifier.
- `annotation` — parsed as an ordinary declaration modifier; the "only `annotation class`" contract is enforced in typecheck, not in the parser.
- `in`, `out` — hard keywords (also used in type-parameter variance, §5.1).
- `is`, `as`, `as?` — keyword operators (§8.3).
- `until`, `downTo`, `step` are **not** keywords; they are contextual infix identifiers (§8.1.1).

### 1.4 Contextual keywords (lexed as `IDENT`)

| spelling | recognized in |
|---|---|
| `eff` | type parameter lists `<...>` (declaration) and type argument lists `<...>` (use site); must be last entry |
| `ref`, `value` | generic bound position only (`<T: ref>`, `where T: value`); hard error in any other type position |
| `init` | class body member position (init block) |
| `constructor` | class body member position (secondary constructor) |
| `this`, `super` | constructor delegation call after `:` |
| `get`, `set` | property accessor position (`get() ...`, `set(v) ...`) |
| `by` | delegated property after type annotation |
| `file` | file-annotation use-site target `@file:...` (only before `package`/`import`) |
| `resume` | only to *reject* `-> resume { ... }` in handle arms (removed syntax, §10) |
| `Pure` | effect row term: `Pure` alone = empty row (path comparison by text) |
| `Unsafe`, `Safe` | expression-position annotation names `@Unsafe do {}`, `@Safe ...` |
| `until`, `downTo`, `step` | infix position between two expressions (contextual infix operators, §8.1.1) |

### 1.5 Symbols / punctuation (longest match first)

```
@ ( ) { } [ ] , : ; . .. + - * / % & | ^ ~ = < > ! ?
-> == != <= >= << >> && || !! ?. ?:
```

Not lexed (and not part of the grammar): `::` (parsed as two `:` tokens, only in `::class`), `..<`, `=>`, `+=` and other compound-assignments, `++`/`--`, `?.[` etc.

---

## 2. File

```ebnf
file           ::= fileAnnotation* packageDecl? importDecl* item* EOF
fileAnnotation ::= '@' 'file' ':' identPath ('(' annotationArgs? ')')?
packageDecl    ::= 'package' identPath ';'?
importDecl     ::= 'import' IDENT ('.' IDENT)* ('.' '*')? ('as' IDENT)? ';'?
identPath      ::= IDENT ('.' IDENT)*
```

Notes:

- File annotations are only recognized at the very start of the file and only with the literal target `file` (`@` IDENT `"file"` `:` lookahead, by text). Other `@...` at file start belong to the first declaration.
- Wildcard import `import a.b.*` **cannot** have an alias (dedicated parse error).
- At most one `package`; imports must all precede items (enforced by loop order).
- Error recovery at top level: skip to the next token that looks like a top-level item start at brace-depth 0 (top-level sync set = `package`/`import` plus modifier/annotation-prefixed `typealias fun val var class interface struct enum effect object companion`). Each recovery is preceded by a recorded error (see §3 note on recovery below).

## 3. Declarations

```ebnf
item ::= typealiasDecl | funDecl | valDecl | extensionPropertyDecl
       | objectDecl | typeDecl

declPrefix ::= (annotationUse | modifier)*
modifier   ::= 'public' | 'internal' | 'private' | 'open' | 'abstract'
             | 'sealed' | 'override' | 'operator' | 'annotation'
             | 'inline'        (* consumed but records "removed" error *)
```

Notes:

- Annotations and modifiers may be freely interleaved; modifiers are sorted & deduplicated by the parser (source order irrelevant).
- **Error model (normative):** the parser records every malformed construct as a diagnostic *first*, then recovers by synchronizing to the next item/statement boundary. The new parser has **no `Missing` placeholder nodes** in its AST: recovery produces partial-but-valid AST nodes plus diagnostics. The legacy "skip trailing junk and downgrade to `Missing`" behaviors are rejected (see §3.3, §3.4, §3.6).

### 3.1 Type alias (spec App. B.10)

```ebnf
typealiasDecl ::= declPrefix 'typealias' IDENT typeParamList? '=' typeRef
```

- `eff` effect-row params are rejected in typealias type-param lists (dedicated error).

### 3.2 Functions

```ebnf
funDecl     ::= declPrefix 'fun' typeParamList? receiverAndName paramList
                (':' typeRef)? effectAnn? whereClause? funBody?
              (* effectAnn and whereClause may appear in EITHER order; each at most once *)
receiverAndName ::= (typeRef '.')? IDENT typeParamList?
                (* type params allowed EITHER before name (`fun <T> f`) OR after name
                   (`fun f<T>`); if the pre-name list exists the post-name list is not parsed *)
effectAnn   ::= '/' effectRowExpr
funBody     ::= block | '=' expr
```

- **Expression bodies** (`fun f(): T = expr`, spec §7.1 / App. B.5.1) are part of the grammar, for top-level functions **and** member functions alike (`funDecl` is the shared production used in type bodies). The `= expr` body must end at a statement/item boundary per the same rules as property accessor `= expr` bodies (§3.6 rules apply analogously): the expression ends at `;`, `}`, the next member/item start, or EOF; after a complete expression body, any other unexpected token is a hard parse error (recorded, then recovery to the next item/member boundary).
- **Body omission:** the body may be omitted only where a body-less declaration is legal — abstract/interface members and effect operations (effect operations additionally use the dedicated production §3.5, which forbids a body). A missing body anywhere else is a recorded error; the declaration is retained as a partial-but-valid AST node (no `Missing` body node).
- Receiver detection (`fun Receiver.Name(...)`): token-scan lookahead — the parser scans for the parameter-list `(` at depth 0, walks back over an optional `<...>` to the name, and requires a `.` before it; the receiver slice is then parsed as a `typeRef` by a sub-parser. Generic receivers (`fun <T> List<T>.f()`), nullable receivers, etc. all flow through `typeRef`.
- `operator` functions are just functions with the `operator` modifier and ordinary identifier names (`plus`, `get`, ...); there is no special name syntax.

### 3.3 Top-level `val` / `var`

```ebnf
valDecl  ::= declPrefix ('val'|'var') (IDENT | valPattern) (':' typeRef)? ('=' expr)? ';'?
valPattern ::= pattern      (* only under 'val'; only when it looks like tuple/struct/variant *)
```

Notes:

- A destructuring pattern is attempted only for `val` and only if the next tokens look like `(`, `Path {`, or `Path (`; otherwise a plain `IDENT` binding is parsed. `var` with a pattern is a dedicated parse error (parser consumes the balanced group for recovery).
- Destructuring `val` requires `= expr` (error otherwise). A `:` type annotation after a pattern binding is **not** parsed (pattern binding goes straight to `=`).
- **Initializer termination (normative):** a top-level initializer expression must end cleanly — the next token must be `;`, EOF, or a top-level item start. After a complete initializer, any unexpected trailing token is a **hard parse error** with a targeted diagnostic; the parser then recovers by synchronizing (with balanced-bracket tracking) to the next top-level item boundary. The legacy behavior of silently skipping trailing junk and downgrading the initializer to a `Missing` placeholder is rejected.
- A top-level `val/var` whose header contains `Receiver . name :` (token scan) is rerouted to `extensionPropertyDecl` (§3.7).

### 3.4 Type declarations

```ebnf
typeDecl    ::= declPrefix typeKind IDENT typeParamList? primaryCtor?
                (':' superTypeList)? whereClause? typeBody?
typeKind    ::= 'class' | 'interface' | 'struct' | 'enum' | 'effect'
primaryCtor ::= '(' ctorParam (',' ctorParam)* ','? ')'
ctorParam   ::= annotationUse* ('val'|'var'|'vararg')* IDENT (':' typeRef)? ('=' expr)?
superTypeList ::= superType (',' superType)* ','?
superType   ::= typeRef callArgList?      (* e.g. `: Base(args)`, `: Iface` *)
typeBody    ::= '{' typeMember* '}'
```

Notes:

- `annotation class` is just `annotation` modifier + `class`; none of the spec §15.2 restrictions (no type params, all-`val` params, no body, ...) are checked by the parser.
- **Header termination (normative):** after a complete header, the next token must be `{` (body), the enclosing `}`, or the next item start. Any other unexpected token is a **hard parse error** with a targeted diagnostic, followed by recovery to the enclosing `}` or the next item start. The legacy "consume unknown tokens until `{`" header-tail tolerance is rejected.
- Missing `:` before a supertype (`class C Base`) is a targeted error for class/interface.
- `enum E : Int { ... }` works via the ordinary supertype list (underlying type per spec §2.3.2.1).
- `effect` declarations share the class grammar; `fun` members inside an `effect` body are parsed as *effect operations* (§3.5).

Type-body members and per-kind context restrictions (enforced by the parser):

```ebnf
typeMember     ::= ';'                      (* empty member *)
                 | initBlock                (* class, object only *)
                 | secondaryCtor            (* class only *)
                 | enumVariantDecl          (* enum only *)
                 | companionObjectDecl      (* class only; error elsewhere *)
                 | objectDecl
                 | propertyDecl
                 | funDecl                  (* member fun; in effect body → effect op *)
                 | typeDecl                 (* nested type *)
initBlock      ::= declPrefix ⌊init⌋ block
secondaryCtor  ::= declPrefix ⌊constructor⌋ typeParamList? paramList whereClause?
                   (':' ('this'|'super') callArgList)? block
companionObjectDecl ::= declPrefix 'companion' 'object' IDENT?
                        (':' superTypeList)? typeBody?
objectDecl     ::= declPrefix 'object' IDENT (':' superTypeList)? typeBody?
enumVariantDecl ::= annotationUse* IDENT enumVariantFields? ('=' expr)?
enumVariantFields ::= '(' 'val' IDENT ':' typeRef (',' 'val' IDENT ':' typeRef)* ','? ')'
```

- `init`/`constructor`/`this`/`super` are contextual (`IDENT` text match).
- Secondary constructor: `eff` params rejected (recorded error); body block is mandatory; delegation target must be exactly `this` or `super`.
- Enum variants are `,`-separated (comma optional after each variant — the parser eats at most one); variant fields require `val name: T` (no defaults, no `var`); `= expr` discriminant allowed after the name/field list.
- Unknown member shapes: a **hard parse error is recorded first**, then the parser recovers with balanced-bracket skipping to the next member boundary (recovery produces partial-but-valid AST plus diagnostics; no `Missing` nodes).
- Named `object` declarations are allowed both at top level and inside any type body. **Anonymous object expressions** (`object : Foo { ... }`) are not part of the language: `object` in expression position is a dedicated hard error (§10, §11).

### 3.5 Effect operation (member of an `effect` body)

```ebnf
effectOpDecl ::= declPrefix 'fun' typeParamList? receiverAndName paramList
                 (':' typeRef)? effectAnn? whereClause?
               (* NO body allowed: a following '{' records a dedicated error and is skipped *)
```

### 3.6 Properties (type-body members)

```ebnf
propertyDecl ::= declPrefix ('val'|'var') IDENT
                 ( ':' typeRef ( 'by' expr                  (* delegated property; 'by' contextual *)
                               | ('=' expr)? accessor* )
                 | '=' expr accessor* ) ';'?
              (* ': typeRef' may be omitted ONLY when an '=' initializer is present
                 (type inferred, spec §10.1) *)
accessor     ::= 'get' '(' ')'        accessorBody
               | 'set' '(' IDENT (':' typeRef)? ')' accessorBody
accessorBody ::= '=' expr | block
```

Notes:

- **Type annotation (normative, per spec §10.1):** a type-body property MAY omit `: typeRef` when it has an `=` initializer (the type is inferred); `: typeRef` remains mandatory when there is no initializer, and mandatory for delegated (`by`) properties.
- `by` delegation is mutually exclusive with `= init` and with accessors (dedicated error if accessors follow a delegate).
- `get`/`set` are contextual: recognized only at `get ( ) (=|{)` / `set ( IDENT ) (=|{)` lookahead shapes.
- **Accessor body termination (normative):** accessor `= expr` bodies must end at `;`, `}`, a member start, or another accessor. After a complete accessor body, any other unexpected token is a **hard parse error** with a targeted diagnostic, followed by recovery to the next member boundary. The legacy "skip trailing junk + downgrade to `Missing`" behavior is rejected. These same boundary rules apply analogously to function `= expr` bodies (§3.2).

### 3.7 Extension properties (top level)

```ebnf
extensionPropertyDecl ::= declPrefix ('val'|'var') typeParamList? typeRef '.' IDENT
                          ':' typeRef ('=' expr)? accessor* ';'?
```

- Routed from a top-level `val/var` only when the pre-`=`/`;` header contains `... . name :` at bracket-depth 0 (token scan, then receiver parsed as `typeRef` by a sub-parser).
- Explicit `: typeRef` is **mandatory** for extension properties (unchanged): they cannot have initializers or backing fields, so the §3.6 inference rule does not apply.
- There is **no** member-position extension property and no extension `val` without receiver; extension *functions* are covered by `funDecl` (§3.2).

### 3.8 Parameters

```ebnf
paramList ::= '(' (param (',' param)* ','?)? ')'
param     ::= annotationUse* 'vararg'? paramName (':' typeRef)? ('=' expr)?
paramName ::= IDENT | 'var'        (* `var` allowed as a name for sysroot intrinsics like addressOf(var: T) *)
```

- Constructor params (§3.4) additionally allow `val`/`var` in any order with `vararg`.

---

## 4. Annotations & modifiers

```ebnf
annotationUse  ::= '@' (IDENT ':')? identPath ('(' annotationArg (',' annotationArg)* ','? ')')?
annotationArg  ::= (IDENT (':'|'='))? expr
```

- The optional `IDENT ':'` prefix is a *use-site target* (e.g. `@property:Foo`, `@param:Bar`, `@file:...`); targets are not validated by the parser.
- Named-arg detection in annotation args: `IDENT '='` always; `IDENT ':'` only when **not** followed by another `:` (guards `String::class`).
- Argument *values* parse as full expressions; compile-time-constness is a typecheck concern.

---

## 5. Generics

### 5.1 Type parameters (declaration site)

```ebnf
typeParamList ::= '<' (typeParam (',' typeParam)* ','? effRowParam? ','? | effRowParam ','?)? '>'
typeParam     ::= variance? IDENT (':' genericBound)?
variance      ::= 'in' | 'out'
effRowParam   ::= ⌊eff⌋ IDENT ('=' effectRowExpr)?     (* at most one, must be last *)
genericBound  ::= ⌊ref⌋ | ⌊value⌋ | typeRef
whereClause   ::= 'where' whereConstraint (',' whereConstraint)* ','?
whereConstraint ::= IDENT ':' genericBound
```

- Empty `<>` is accepted.
- `ref`/`value` anywhere outside bound position (any `typeRef` start) is a **hard parse error** (`bound_keyword_type_position`, §10), even though they lex as `IDENT`.
- Where-clause trailing comma is allowed before `{`/`;`/`}`/EOF.

### 5.2 Type arguments (use site)

```ebnf
typeArgs ::= '<' (typeArg (',' typeArg)* ','?)? ('>' | '>>'-split | '>='-split)
typeArg  ::= typeRef | '*' | '⌊eff⌋' effectRowExpr    (* `eff` arg: at most one, must be last *)
```

- **`*` star projection** exists only in type-argument position.
- **`>>` splitting:** when closing nested type arguments, a `>>` token is split in-place into two `>` tokens (the second keeps the span of the right half). This handles `Continuation<Continuation<Int, Unit>>`.
- **`>=` splitting (normative):** analogously, when closing type arguments, a `>=` (`GtEq`) token is split into `>` + `>=`, so `A<B<C>> >= x` parses as a comparison of `A<B<C>>` with `x` (§12 item 3 — resolved).
- Effect-row *defaults* on declaration: `<eff E = Pure>` (spec §3.4).

---

## 6. Types

```ebnf
typeRef       ::= (parenType functionTail? | pathType) '?'* receiverFnTail?
                (* parse base; ZERO OR MORE postfix '?' (each wraps one Option layer,
                   spec §2.4 — nesting is not flattened); then optional receiver-function
                   tail which may itself be followed by '?'* *)
functionTail  ::= '->' typeRef effectAnn?            (* only when base was '(' ... ')' *)
receiverFnTail ::= '.' parenTypeList '->' typeRef effectAnn?
parenType     ::= '(' ')'                            (* Unit / 0-tuple type *)
                | '(' typeRef ')'                    (* grouping → transparent *)
                | '(' typeRef (',' typeRef)+ ','? ')'  (* tuple type *)
parenTypeList ::= '(' (typeRef (',' typeRef)* ','?)? ')'
pathType      ::= identPath typeArgs?                (* path segments stop before '.(' *)
```

Notes:

- Grouping `(T)` is transparent (returns `T`); `()` is the Unit tuple type; `(T,)` is a 1-tuple.
- **Nested nullable `T??` is part of the grammar:** any number of postfix `?` is accepted; each `?` wraps exactly one `Option` layer (spec §2.4 — nesting is not flattened).
- Function types require parenthesized parameter lists: `(A, B) -> R`; there are no named/optional parameter notations in function types. Effect annotation comes **after** the return type: `(A) -> R / Row`.
- Receiver function type `T.(A) -> R` (also `T?.(A) -> R`); path parsing stops at `.(` to enable this.
- `typeRef` start tokens: `IDENT` (but not `ref`/`value`) or `(`.

### 6.1 Effect row expressions

```ebnf
effectRowExpr ::= ('(' effectRowExpr ')' | effectRowTerm ('+' effectRowTerm)*) '!'?
effectRowTerm ::= pathType            (* path with optional type args, e.g. Raise<IOError> *)
```

- `Pure` as a bare single-segment term denotes the empty row (text match, not a keyword).
- Trailing `!` = *closed row* (spec §5.8.4); it binds to the **whole** row, not the last term (lower precedence than `+`).

---

## 7. Statements & blocks

```ebnf
block         ::= '{' stmt* '}'
stmt          ::= ';'                                  (* empty statement *)
                | localValDecl ';'?
                | 'return' expr? ';'?
                | 'while' '(' expr ')' block ';'?
                | 'for' '(' IDENT 'in' expr ')' block ';'?
                | 'break' ';'?
                | 'continue' ';'?
                | exprStmt ';'?
exprStmt      ::= expr ( '=' expr )?                   (* assignment allowed ONLY here *)
localValDecl  ::= annotationUse* ('val'|'var') (IDENT | valPattern)
                  (':' typeRef)? ('=' expr)?
```

Notes:

- **Newline is insignificant.** Statements need no terminator; `;` is an optional separator everywhere and is folded into the preceding statement's span.
- Local `val/var` rules mirror top-level (§3.3) minus extension-property routing, plus annotations; `var` destructuring rejected; pattern binding requires `=`; `:` type only on plain name bindings.
- `return`: no value if next token is `;`, `}`, EOF; if the next token cannot start an expression but *can* start a statement, it is treated as no-value `return` followed by that statement (recovery heuristic); otherwise a hard error.
- `while`/`for` bodies must be blocks (`{ ... }`), no single-statement bodies.
- `for` binder is a **single identifier** — no destructuring in `for` (matches spec §16.2.2 desugaring; §11).
- **Assignment:** parsed only in statement position. Legal LHS forms: `IDENT` (plain name), `expr.member` (member access), and `expr '[' expr (',' expr)* ']'` (index assignment `a[i] = v` / `a[i, j] = v`, spec App. B.8 — an `IndexAssign` node). Resolution of an index read/assignment to `operator get` / `operator set` is typecheck's concern, not the parser's. Not allowed as LHS: `?.`-chains, splice targets, calls, literals. An `=` after an expression in any other context is the hard error `assignment_expression_not_allowed` (§10). There are no compound assignments (`+=` etc. don't lex).
- There is no local `fun`/`class` statement, no do-while, no labeled break/continue/return (spec §7.2 agrees non-local return is unsupported; §11).

---

## 8. Expressions

### 8.0 Entry points (parser-internal but normative)

| entry | used at | assignment `=` |
|---|---|---|
| `expr` | everywhere | rejected after a full expression (`assignment_expression_not_allowed`) |
| `stmtExpr` | statement position only | one `=` allowed (§7) |
| `whenArmExpr` | non-block `when` arm bodies | rejected; plus the `is`-arm lookahead (§9.2 note) |

### 8.1 Precedence & associativity (Pratt binding powers)

Higher bp = tighter binding. `(l, r)` are the Pratt binding powers; left-assoc uses `(p, p+1)`, right-assoc uses `(p, p)`. Parse starts at `min_bp = 0`.

| level (l,r) | operators | assoc | note |
|---|---|---|---|
| postfix (implicit, > all) | `f(x)`, `x.m`, `x?.m`, `x[i]`, `x!!`, `x<T>`, `T::class`, `x { .. }` trailing lambda, `x with { .. }`, `x.[f]` splice | left | §8.4 |
| prefix (implicit) | `!x`, `-x`, `~x` | right (recursive) | operand is a prefix expression, so prefix binds tighter than every binary op |
| (11,12) | `*` `/` `%` | left | |
| (10,11) | `+` `-` | left | |
| (9,10) | `<<` `>>` | left | |
| (8,9) | `..` `<` `<=` `>` `>=` | left | range `..` shares the comparison level — **normative for scoop2** (see below) |
| (8,9) | `until` `downTo` `step` | left | contextual infix identifiers (§8.1.1); same level as `..` |
| (8,9) | `is` `!is` `as` `as?` | left | keyword infix; RHS is a `typeRef`, not an expr |
| (7,8) | `==` `!=` | left | |
| (6,7) | `&` | left | |
| (5,6) | `^` | left | |
| (4,5) | `|` | left | |
| (3,4) | `&&` | left | |
| (2,3) | `||` | left | |
| (1,1) | `?:` | **right** | only right-associative binary op |

Consequences (verified against the table): `a..b < c` parses `(a..b) < c`; `a is T == b` is invalid (RHS of `is` is a type); `!x is T` parses `(!x) is T`; `a ?: b ?: c` = `a ?: (b ?: c)`; there is **no unary `+`** (only `!`, `-`, `~`).

**`..` precedence (normative):** the range operator stays at the comparison level (8,9) for scoop2. This deliberately differs from Kotlin (which places range formation below Elvis); here `a + b .. c` parses as `(a+b)..c`.

#### 8.1.1 Contextual infix operators `until` / `downTo` / `step` (spec App. B.12)

```ebnf
contextualInfixOp ::= 'until' | 'downTo' | 'step'
```

- These are **contextual infix identifiers**: lexed as ordinary `IDENT`, recognized as operators only in infix position — i.e. when an `IDENT` whose text is `until`, `downTo`, or `step` appears between two expressions where the grammar expects either an infix operator or the end of the expression. Everywhere else they are plain identifiers.
- Precedence: same level as `..` — binding powers (8,9), **left-associative**.
- Desugaring (normative): `a until b` ≡ `a.until(b)`; `a downTo b` ≡ `a.downTo(b)`; `x step n` ≡ `x.step(n)` — ordinary method-call sugar. The parser records an **AST infix-call node** (operator text + LHS + RHS) which typecheck resolves like operator overloads. The `operator`-modifier requirement of spec App. B.8 does **not** apply to these three (the spec defines no `infix` modifier); typecheck resolves them as ordinary method calls.

### 8.2 Primary (atomic) expressions

```ebnf
atom ::= IDENT                            (* plain reference; note `Ident {` disambiguation below *)
       | structLit                        (* IDENT '{' ... '}' when classified as struct literal *)
       | INT_LIT | FLOAT_LIT | CHAR_LIT
       | STRING_LIT | FSTRING             (* interpolated → Text/Expr part list *)
       | ifExpr | whenExpr | handleExpr | tryExpr
       | doBlock
       | lambda                           (* bare '{' in expression position is ALWAYS a lambda *)
       | parenExpr
       | arrayLit
structLit  ::= IDENT '{' (structField (',' structField)* ','?)? '}'
structField ::= IDENT ':' expr
parenExpr  ::= '(' ')'                    (* Unit literal *)
             | '(' expr ')'               (* grouping: transparent; for non-literal inner exprs
                                             the parens' span is adopted — AST-visible quirk *)
             | '(' expr ',' (expr (',' expr)* ','?)? ')'   (* tuple literal; (a,) is 1-tuple *)
arrayLit   ::= '[' (expr (',' expr)* ','?)? ']'
doBlock    ::= 'do' block
```

- **Struct literal restrictions:** single-segment `IDENT` name only (no qualified path, no type args); fields only `name: expr` (no shorthand, no spread).
- **`object` in expression position** is a dedicated hard error (`anonymous_object_unsupported`, §10); anonymous object expressions are not part of the language (§11).
- **`Ident {` disambiguation** (normative): with tokens `Ident { t2 t3`:
  - `t2` = `}` or `->` → lambda (i.e. trailing-lambda path; atom stays `Ident`).
  - `t2` = IDENT: `t3` = `->` or `,` or `}` → lambda; `t3` = `:` → struct literal **unless** the brace group contains a top-level `->` (scan at paren/brace/bracket depth 0), in which case lambda; `t3` = any other symbol or keyword → lambda; otherwise (e.g. `Point { x 1 }`) → struct literal (to produce the "missing `:`" diagnostic).
  - `t2` = anything else → lambda.
- **Lambda:**

```ebnf
lambda    ::= '{' lambdaParams? '->'? blockBody '}'
          (* forms: { -> b }, { a, b: T -> b }, { a, b, -> b } (trailing comma allowed), { b } *)
lambdaParams ::= (IDENT (':' typeRef)? ',')* IDENT (':' typeRef)?
```

  Param parsing is speculative: `IDENT (':' typeRef)? (',' ...)` followed by `->`; if no `->` is found, the parser backtracks and the brace content is parsed as statements. A lambda body that is a single expression statement **without** trailing `;` is unwrapped to that expression (its value); otherwise the body is a block (value `Unit`).
- **`{` after control keywords:** `if`/`when`/etc. parse their own block bodies first, so `if (c) { ... }` is never a lambda.
- Interpolated strings: `${ expr }` holes are parsed by a fresh sub-parser over the hole's source slice; the hole must contain exactly one full expression.

### 8.3 Prefix expressions

```ebnf
prefixExpr ::= ('!' | '-' | '~') prefixExpr
             | '@Unsafe' 'do' block                     (* unsafe block *)
             | '@Safe' ('do' block | lambda)            (* safe block / safe closure *)
             | annotationUse+ prefixExpr                (* general annotated expression *)
             | postfixExpr
```

- `@Unsafe { ... }` (without `do`) is a **dedicated hard error** (`unsafe_block_requires_do`, §10); `@Safe` accepts both `do { }` and bare `{ }` (closure). Other annotation names prefix any prefix-expression (AST `Annotated`).
- `perform` at expression start → dedicated error (§10).
- `*` at expression start → `spread_arg_outside_call` error (spread exists only in call args).
- No unary `+`.

### 8.4 Postfix expressions (bind tightest; looped until no suffix matches)

```ebnf
postfixExpr ::= atom postfix*
postfix     ::= callArgList                          (* call *)
              | '.' memberSeg                        (* member access *)
              | '?.' memberSeg                       (* safe member access *)
              | '.' '[' expr ']'                     (* splice field access, spec §6.4 *)
              | indexPostfix                         (* index/subscript, spec App. B.8 *)
              | '!!'                                 (* not-null assertion *)
              | typeArgs                             (* explicit type application, see note *)
              | ':' ':' 'class'                      (* class literal `T::class` *)
              | lambda                               (* trailing lambda, Kotlin style *)
              | 'with' '{' withField (',' withField)* ','? '}'   (* value-type update, spec §2.6 *)
indexPostfix ::= '[' expr (',' expr)* ']'            (* multi-index supported *)
memberSeg   ::= IDENT | INT_LIT                      (* INT for tuple indexing: t.0, t.1.2 *)
callArgList ::= '(' (callArg (',' callArg)* ','?)? ')'
callArg     ::= (IDENT '=')? ('*' expr | expr)       (* named arg; spread; named spread `n = *xs` *)
withField   ::= fieldPath ':' expr
fieldPath   ::= (IDENT | INT_LIT | FLOAT_LIT-as-two-ints) ('.' memberSeg)*
```

Notes:

- **Indexing `a[i]` / `a[i, j]` (spec App. B.8) is part of the grammar:** a `[`-suffix directly after a postfix expression (not after `.`, which is the splice form) parses as an index expression. Multiple comma-separated indices are supported for multi-parameter `get`/`set`. Resolution to `operator get` (read) / `operator set` (assignment, §7) is typecheck's concern; the parser only builds the index / `IndexAssign` nodes.
- **Call args:** `name = expr` is a named argument **only** when `IDENT '='` appears directly in an argument list; elsewhere `name = value` is either a stmt-level assignment or an error. Named args may mix with positional (ordering rules are typecheck's). Spread `*expr` and named spread `name = *expr` only in argument lists. Named-arg-in-array-literal is a dedicated error (`named_arg_outside_call`, §10); other positions fall out as generic errors or assignments.
- **Type application `expr<T>`:** `<` is also a binary operator, so a token-scan lookahead (`scan_type_args_end`, which understands nested generics, `eff` rows, trailing commas) must succeed **and** the token after the closing `>` must be one of `( { . ?. !! , ) ] } ; : as as? is` or EOF — **or there must be a line break between `>` and the next token** (the only newline-sensitive rule in the grammar). This makes `f<T>(x)` and `f<T>` as a value work while keeping `a < b` a comparison.
- **`::class`:** `::` is two `:` tokens; only exactly `:: 'class'` matches; the receiver must reduce to a type path of `Ident(.Ident)*` — `(expr)::class`, `f<T>()::class` etc. are dedicated errors (`class_literal_receiver_invalid`, §10). Note `a.b.C::class` works because `a.b.C` parses as nested member access.
- **Trailing lambda:** `expr { ... }` wraps as `Call { callee: expr, args: [lambda] }`; if `expr` is already a `Call`, the lambda is appended to its args. Repeating gives multiple trailing lambdas: `combine(1) { .. } { .. }` parses as one call with two lambda args (spec App. B.5.4 ✓). The atom-level `Ident {` disambiguation (§8.2) decides struct-literal vs trailing-lambda *before* this loop.
- **`with` update:** `expr with { path: value, ... }`; field paths may use integer segments for tuples; a `FLOAT_LIT` like `0.1` immediately after `{`/`.` is split into two integer segments (the first segment may be a float token split at its `.`). Note: `with` here is a real keyword; the *handler* use of `with` is removed (§10). `with` binds as a postfix at the same level as member access.
- **Splice:** `receiver.[fieldExpr]` — only when `[` directly follows `.` (distinct from the index postfix, where `[` directly follows the receiver expression).

### 8.5 Control-flow expressions

```ebnf
ifExpr   ::= 'if' '(' expr ')' controlBody ('else' controlBody)?
controlBody ::= block | expr                     (* block or single expression *)
whenExpr ::= 'when' '(' expr ')' '{' whenArm* '}'
whenArm  ::= whenPat ('if' expr)? '->' controlBody ';'*
```

- Parenthesized conditions are mandatory (`if (c)`, `when (x)`).
- `else` binds to the innermost `if` naturally through recursive `controlBody` parsing (an `if` without `else` is legal).
- `when` always requires a subject (no subject-less `when { cond -> }` form; §11).
- Arm bodies: block or a single expression; stray `;` between arms allowed. In a **non-block** arm body, the token sequence `is TypeRef ->` is treated as the start of the *next* arm, not as an infix `is` on the current arm body (lookahead `looks_like_when_is_arm_start`: `is` + scannable type ref + `->`). This is how arm boundaries are found without newlines.

### 8.6 Effects: handle / try

```ebnf
handleExpr ::= 'handle' block 'on' '{' handleArm* '}' ('finally' block)?
handleArm  ::= handleOp (',' IDENT)? '->' controlBody ';'*
handleOp   ::= identPath typeArgs? '.' IDENT typeArgs? '(' handleBinders? ')'
             (* effect path with optional type args only in `Path<Args>.op(...)` form;
                op may have its own type args: `Query.ask<Int>(...)` *)
handleBinders ::= IDENT (':' typeRef)? (',' IDENT (':' typeRef)?)* ','?
tryExpr    ::= 'try' block catchArm+ ('finally' block)?
catchArm   ::= 'catch' '(' IDENT ':' typeRef ')' block
```

- Non-resuming arm: `Effect.op(binders) -> body`. Escape-continuation (resuming) arm: `Effect.op(binders), k -> body` (the `, k` form). `-> resume { ... }` is removed → dedicated error (§10).
- Handler keyword is `on`; `handle { .. } with { .. }` is consumed and rejected with a dedicated error (§10).
- Handle-op effect path requires at least `X.op` (a bare `op(...)` is rejected); the type-args-before-dot form (`Pair<String, Int>.ping(...)`) is recognized by a scan (`type_args_followed_by_dot_ident_at`).
- Arm bodies: block or single expression; `;` separators optional; arm-boundary error recovery syncs on `}`/`;`/next `Effect.op(...) ->` shape (each recovery preceded by a recorded error).
- `try/catch` is **desugared in the parser** to a `handle` over `scoop.core.Raise.raise` (synthetic identifiers at the `catch` keyword span). At least one `catch`; bodies must be blocks; binder must have an explicit `: Type`. `try` with expression (non-block) bodies is not part of the language (§11).

---

## 9. Patterns

### 9.1 Destructuring patterns (`val` bindings, §3.3/§7)

```ebnf
pattern       ::= tuplePattern | structPattern | variantPattern | '_' | IDENT
tuplePattern  ::= '(' (patternElem (',' patternElem)* ','?)? ')'
patternElem   ::= '..' | pattern                  (* '..' rest: once, last *)
structPattern ::= identPath '{' (structPatField (',' structPatField)* ','? '..'? ','?)? '}'
structPatField ::= IDENT (':' pattern)?
variantPattern ::= identPath '(' (patternElem (',' patternElem)* ','?)? ')'
```

- Entered only for `val` when lookahead sees `(`, `Path {`, or `Path (`.
- `..` rest: at most once and must be the last element/field/arg (dedicated errors; `..` may also lex as two `.` tokens — both accepted). In struct patterns `..` appears as a bare field entry.
- `_` and bind names are plain idents (`_` by text).

### 9.2 `when` patterns

```ebnf
whenPat      ::= whenPatAtom ('|' whenPatAtom)*        (* or-pattern; else not allowed after | *)
whenPatAtom  ::= 'else'                                (* first alternative only *)
               | 'is' typeRef
               | tupleWhenPat
               | INT_LIT | CHAR_LIT | STRING_LIT
               | variantWhenPat | '_' | IDENT-bind | 'true' | 'false'
tupleWhenPat ::= '(' (whenPatElem (',' whenPatElem)* ','?)? ')'
whenPatElem  ::= '..' | whenPat
variantWhenPat ::= identPath ('(' (whenPatElem (',' whenPatElem)* ','?)? ')')?
```

- Literals: int/char/string only — **float literals are rejected** in patterns (recorded error, arm continues as wildcard). Bool patterns are the idents `true`/`false`; `_` wildcard is ident `_`.
- **Unqualified bare `Ident`** (no `(`): uppercase-first-letter → 0-arg variant pattern; otherwise → binding pattern. **Heuristic, normative.**
- **Qualified variant patterns:** `a.b.C` / `a.b.C(...)` (lookahead `IDENT . IDENT`); unqualified `Name(...)` also works; args may contain nested `whenPat`s and a final `..` rest.
- Or-patterns `A | B`: parsed at the arm level; `else` after `|` is syntactically blocked (allow_else=false); mixing `else` inside or-patterns is otherwise left to later phases.
- Guards: `pat if expr ->` (§8.5).
- The `is` infix-vs-arm-start ambiguity inside arm bodies: §8.5 note.

---

## 10. Removed constructs & dedicated diagnostics (normative negative grammar)

The new parser must reproduce these targeted diagnostics with the same codes (`scoop::parse::*`):

| construct | parser behavior | diagnostic code |
|---|---|---|
| `perform Effect.op(...)` | hard error at expression start | `scoop::parse::perform_keyword_removed` |
| `inline` modifier | consumed in `declPrefix`, error recorded, parsing continues | `scoop::parse::inline_modifier_removed` |
| `handle {..} with {..}` | whole `with {..}` (+optional `finally {..}`) consumed, then error | `scoop::parse::handler_with_keyword_removed` |
| `-> resume { .. }` in handle arm | block consumed, then error | `scoop::parse::handle_immediate_resume_removed` |
| `ref` / `value` in any `typeRef` position | hard error | `scoop::parse::bound_keyword_type_position` |
| assignment `=` in expression context | hard error | `scoop::parse::assignment_expression_not_allowed` |
| `*expr` outside call args | hard error | `scoop::parse::spread_arg_outside_call` |
| `name = expr` in array literal | hard error | `scoop::parse::named_arg_outside_call` |
| `@Unsafe { .. }` (no `do`) | hard error | `scoop::parse::unsafe_block_requires_do` |
| non-path `::class` receiver | hard error | `scoop::parse::class_literal_receiver_invalid` |
| `object` in expression position (anonymous object expression) | hard error | `scoop::parse::anonymous_object_unsupported` |
| effect op with `{ .. }` body | error recorded, body skipped | generic `Expected` |
| `var` destructuring / pattern without `=` / property without `:` where required | targeted `Expected` errors | — |

---

## 11. Constructs NOT part of the language (normative negative scope)

The new parser must **reject** the following with clear, targeted errors rather than silent misparses:

1. **Anonymous object expressions** (`object : Foo { ... }` in expression position) — out of scope; the spec (App. B.9) only defines named objects, and named `object` *declarations* are covered by §3.4. The parser must emit the dedicated error `scoop::parse::anonymous_object_unsupported` for `object` in expression position (§10).
2. **`try` with expression (non-block) bodies** — `try`/`catch`/`finally` bodies are always blocks (§8.6).
3. **Subject-less `when`** (`when { cond -> ... }`) — `when` always requires a parenthesized subject (§8.5).
4. **do-while** — only `while` exists (§7).
5. **Labels / `return@`, labeled break/continue** — no label syntax exists (§7).
6. **Local function/class statements** — declarations are not statements (§7).
7. **Compound assignment** (`+=`, `-=`, ...) — these tokens do not even lex (§1.5).
8. **`++` / `--`** — do not lex (§1.5).
9. **Unary `+`** — prefix operators are only `!`, `-`, `~` (§8.3).
10. **Backtick identifiers** — do not lex (§1.2).
11. **`for` with destructuring binder** — the `for` binder is a single `IDENT` only (§7; spec §16.2 desugaring also uses a single binder).

Spec↔grammar divergences that remain (informative):

- **Modifier set:** the grammar has no `data`, `value`, `inner`, `lateinit`, `const`, `tailrec` etc. (the spec also doesn't define them); `inline` is explicitly removed (§10).
- **`as?` tokenization:** whitespace-sensitive (§1.2 quirk 1).
- **Enum body separators:** variants separated by `,` (Kotlin uses `;` before member list); the parser eats at most one comma per variant and treats subsequent `fun`/`val`/etc. as members — an enum variant list followed by members works without an explicit `;`.

---

## 12. Known ambiguities & their resolution in the new parser

Each item states how the **new parser** (`scoop2_syntax`) resolves it; the resolution is the same heuristic as the legacy parser unless explicitly changed here or in §14.

1. **`Ident {` struct-literal vs trailing-lambda** — resolved by a 2-token-plus-scan heuristic (§8.2); `Point { x: 1 }` with a lambda-valued field containing `->` at brace depth 0 flips classification to lambda (e.g. `Point { f: { -> 1 } }` — the inner `->` is at depth ≥ 1 so it stays struct-lit; but `Point { f: g, h: x -> ... }` would misclassify). **New parser: same heuristic, normative as-is.**
2. **`expr <` comparison vs type application** — resolved by full type-args scan + follower-set/line-break rule (§8.4). `f<T> + 1` is a comparison, not type application, because `+` is not in the follower set and there's no newline. **New parser: same rule, normative as-is.**
3. **`>>` / `>=` vs `> >` / `> >=` in nested generics** — **RESOLVED.** The new lexer/parser splits `>>` into two `>` tokens **and** splits `>=` (`GtEq`) into `>` + `>=` when closing type arguments (§5.2), so `A<B<C>> >= x` parses. The legacy limitation (unparseable `>=` after nested generics) is removed.
4. **`is` infix vs when-arm start** — resolved by type-ref scan + `->` lookahead, only inside non-block arm bodies (§8.5). **New parser: same heuristic.**
5. **Extension receiver detection** (`fun T.name(`) — token-scan around the parameter-list `(`; the scan depends on bracket-depth bookkeeping that counts `<`/`>`/`>>` arithmetically and can be confused by unbalanced `<` inside default values in the header scan region. **New parser: same token-scan heuristic.**
6. **Statement boundaries without newlines** — rely on expression-parse exhaustion; two expressions in a row (`a b`) is an error at the second expression's first token where the grammar demands a boundary. **New parser: CHANGED (normative)** — in initializer/property/accessor contexts, unexpected tokens after a complete initializer/body are hard parse errors with targeted diagnostics, followed by error-recovery sync to the next item/statement boundary; each recovery is preceded by a recorded error and produces partial-but-valid AST nodes. There are no `Missing` placeholder nodes (the legacy skip-and-downgrade tolerance is rejected; §3.3, §3.4, §3.6).
7. **`when` arm `|` vs binary `|`** — `|` inside arm *patterns* is or-pattern; inside arm *bodies* it's bitwise-or. **New parser: same positional distinction, pinned as normative.**
8. **`get`/`set`/`by`/`init`/`constructor` contextual keywords** — all usable as ordinary identifiers elsewhere; accessor recognition requires the exact `name ( ... ) (=|{)` shape (§3.6). **New parser: same shapes.** (The contextual infix identifiers `until`/`downTo`/`step` follow the same lex-as-`IDENT` principle, §8.1.1.)
9. **Handler arm boundaries without separators** — `Effect.op(...) -> expr Effect.op2(...) -> expr` relies on arm-body parse exhaustion + recovery lookahead `looks_like_handle_arm_start_at` (scans for depth-0 `->` with a `.` before the first `(`). **New parser: same recovery lookahead.**
10. **Float token in field paths** — `with { 0.1: v }` reinterprets float token `0.1` as path `0 . 1` (§8.4); a side effect of forced-int lexing after `.`. **New parser: same split behavior.**

---

## 13. Production index (for counting)

Lexical (§1): `IDENT INT_LIT DEC_INT HEX_INT BIN_INT INT_SUFFIX FLOAT_LIT EXP FLOAT_SUFFIX CHAR_LIT STRING_LIT NORMAL_STR RAW_STR FSTRING ESC` (15)
File (§2): `file fileAnnotation packageDecl importDecl identPath` (5)
Declarations (§3): `item declPrefix modifier typealiasDecl funDecl receiverAndName effectAnn funBody valDecl valPattern typeDecl typeKind primaryCtor ctorParam superTypeList superType typeBody typeMember initBlock secondaryCtor companionObjectDecl objectDecl enumVariantDecl enumVariantFields effectOpDecl propertyDecl accessor accessorBody extensionPropertyDecl paramList param paramName` (31)
Annotations (§4): `annotationUse annotationArg` (2)
Generics (§5): `typeParamList typeParam variance effRowParam genericBound whereClause whereConstraint typeArgs typeArg` (9)
Types (§6): `typeRef functionTail receiverFnTail parenType parenTypeList pathType effectRowExpr effectRowTerm` (8)
Statements (§7): `block stmt exprStmt localValDecl` (4)
Expressions (§8): `contextualInfixOp atom structLit structField parenExpr arrayLit doBlock lambda lambdaParams prefixExpr postfixExpr postfix indexPostfix memberSeg callArgList callArg withField fieldPath ifExpr controlBody whenExpr whenArm handleExpr handleArm handleOp handleBinders tryExpr catchArm` (28)
Patterns (§9): `pattern tuplePattern patternElem structPattern structPatField variantPattern whenPat whenPatAtom tupleWhenPat whenPatElem variantWhenPat` (11)

**Total: 113 productions (15 lexical + 98 syntactic).**

Productions whose content changed vs the legacy extraction: `funBody` (expression bodies, §3.2), `propertyDecl` (optional `: T` with initializer, §3.6), `typeRef` (postfix `'?'*`, §6), `postfix` + new `indexPostfix` (indexing, §8.4), new `contextualInfixOp` (§8.1.1), `typeArgs` (`>=`-split, §5.2).

---

## 14. Changelog vs legacy parser

Review rulings applied on top of the legacy-parser extraction (each is normative for `scoop2_syntax`):

1. **Expression-body functions** `fun f(): T = expr` are now part of the grammar, for top-level and member functions (`funBody ::= block | '=' expr`, §3.2); body termination follows the accessor-body boundary rules (§3.6).
2. **Indexing** `a[i]` / `a[i] = v` is now part of the grammar: new `indexPostfix` postfix with multi-index support (§8.4), and `IndexAssign` as a legal statement-level assignment LHS (§7). `operator get`/`set` resolution is typecheck's concern.
3. **`until` / `downTo` / `step`** are now part of the grammar as contextual infix identifiers at the `..` precedence level (8,9), left-associative, desugared to ordinary method calls via an AST infix-call node (§8.1.1).
4. **Nested nullable `T??`** is now part of the grammar: `typeRef` accepts zero or more postfix `?`, one `Option` layer each (§6).
5. **Anonymous object expressions** are explicitly out of scope; `object` in expression position gets the dedicated error `scoop::parse::anonymous_object_unsupported` (§10, §11).
6. **Property type annotation** now follows spec §10.1: `: T` may be omitted on type-body properties with an `=` initializer; mandatory otherwise; always mandatory on extension properties (§3.6, §3.7).
7. **Legacy tolerances → hard errors:** trailing junk after initializers, header tails, and accessor bodies is now a hard parse error with a targeted diagnostic, followed by recorded-error recovery to the next item/statement boundary; the new AST has no `Missing` placeholder nodes (§3, §3.3, §3.4, §3.6, §12 item 6).
8. **`>=` after nested generics** is resolved: `GtEq` is split into `>` + `>=` when closing type arguments, analogously to the `>>` split (§5.2, §12 item 3).
9. **`..` precedence** stays at the comparison level (8,9) as normative for scoop2, deliberately differing from Kotlin: `a + b .. c` parses as `(a+b)..c` (§8.1).
