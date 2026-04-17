# TYPES_TUTORIAL

Guida pratica per aggiungere un nuovo tipo in Fero, dal typechecker all'interprete runtime.

## 1. Aggiungi il tipo semantico (typechecker)

File principali:
- `crates/fero-typeck/src/types.rs`
- `crates/fero-typeck/src/check_expr.rs`
- `crates/fero-typeck/src/check_stmt.rs` (se impatta `for`, mutazioni, statement-level typing)

Checklist:
1. Inserisci il nuovo variant in `Type`.
2. Aggiorna `Type::is_numeric`, `is_integer`, `is_printable` se necessario.
3. Aggiorna `Display for Type`.
4. Registra il nome in `resolve_named_type` se il tipo deve essere nominabile (`let x: Tipo = ...`).

Esempio minimo:
```rust
pub enum Type {
    // ...
    Regex,
}
```

## 2. Tipizza costruttori e metodi

File:
- `crates/fero-typeck/src/check_expr.rs`

Pattern consigliato:
1. Intercetta `HirExpression::Call` su `Member(Name("Tipo"), "ctor")` per costruttori statici.
2. Intercetta `object_ty == Type::Tipo` per metodi istanza.
3. Crea helper dedicati (es. `check_regex_constructor`, `check_regex_method`).
4. Valida arità + tipi argomenti + casi invalidi.

Nota importante:
- Se il tipo introduce collezioni/slice/iterazione, aggiorna anche `check_for_expr` e `check_stmt::For`.

## 2.1 Type sugar nel parser

Se il nuovo tipo ha sugar sintattico (es. `T?` per `Option[T]`):
- implementa il parsing in `crates/fero-parser/src/parser.rs` nella parse dei tipi (`parse_type`)
- conserva la distinzione tra type-position e expression-position
- aggiorna AST/HIR (`crates/fero-ast`, `crates/fero-hir`) per rappresentare il sugar in modo esplicito

Caso concreto:
- `T?` in type position deve diventare semanticamente `Option[T]`
- `expr?` resta una feature separata (propagation) e non va reinterpretata
- `T!E` / `T!` in type position devono diventare semanticamente `Result[T, E]` / `Result[T, Error]`
- `expr!` resta unwrap/crash in expression position (non type sugar)

## 3. Aggiungi il valore runtime

File:
- `crates/fero-interpreter/src/value.rs`

Checklist:
1. Inserisci il variant in `Value` (es. `Value::Regex(...)`, `Value::UInt8(u8)`).
2. Se usi slice/array specializzate, aggiorna `SliceParent` / `SliceValue::get`.
3. Aggiorna `Display for Value`.
4. Aggiorna `to_repr()` se serve output debug coerente.

Esempi reali in repo:
- `String.bytes()` deve emettere `Value::UInt8` via `SliceParent::Bytes`.
- `Regex` ha IR dedicata (`RegexValue`) e `Display` leggibile.

## 4. Esegui il wiring nell'interprete

File:
- `crates/fero-interpreter/src/eval_expr.rs`

Checklist:
1. Nel dispatch delle call, intercetta il namespace statico (`Regex.xxx(...)`).
2. Implementa i metodi istanza (`re.is_match(...)`, `re.replace(...)`).
3. Se il nuovo tipo partecipa a operatori (`+`, `==`, `<`...), aggiorna i match binari/unari.
4. Mantieni errori runtime espliciti e coerenti con il typechecker.

Pattern utile:
- helper `eval_<tipo>_constructor(...)`
- helper `eval_<tipo>_method(...)`

Per tipi con semantica di scansione (come `Regex`):
- definisci una regola esplicita sui match zero-width
- in operazioni globali (`replace`), dopo un match di lunghezza 0 avanza sempre di almeno 1 Unicode scalar
- allinea tutte le entrypoint (`is_match`, `replace`, eventuali future API) alle stesse regole di avanzamento/anchor

## 5. Test obbligatori

Type checker:
- `crates/fero-typeck/tests/typeck_tests.rs`
- Aggiungi test `expect_ok` per casi validi.
- Aggiungi test `expect_err` per arità/tipi invalidi.

Interprete:
- `crates/fero-interpreter/tests/interpreter_tests.rs`
- Verifica semantica reale e output value-level.
- Copri edge case (Unicode, anchor/zero-width, ecc. se rilevante).

## 6. Esempi utente e documentazione

Aggiungi:
- esempi in `examples/*.fero`
- specifica in `docs/FERO_SURFACE_SPEC_v0.1.md`

Per tipi nuovi conviene documentare:
- costruttori
- metodi
- vincoli v0
- funzionalità esplicitamente non incluse

## 7. Regola pratica (anti-regressioni)

Quando aggiungi un tipo nuovo, verifica sempre che i tre livelli restino allineati:
1. `Type::*` (typechecker)
2. `Value::*` (runtime)
3. dispatch call/metodi in `eval_expr`

Se uno dei tre manca, avrai mismatch tra compilazione e runtime.
