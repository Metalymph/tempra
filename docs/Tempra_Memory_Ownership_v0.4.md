# Tempra — Memory, Ownership, Borrowing & Slices

**Versione:** v0.4
**Data:** 15 novembre 2026
**Stato:** Specifica progettuale v0.4 approvata — riferimento unico per il modello di memoria di Tempra. Non equivale a implementazione o prova formale.
**Ambito:** Value semantics, ownership, borrowing, Range, Slice, control flow, closure/catture, receiver, task e actor, sospensione, risorse non duplicabili, effects, generics, FFI, contratti TIR; parità Tempra Script/Tempra Native.

**Cronologia:** v0.1 (ownership e Slice); v0.2 (borrow nel control flow); v0.3 (closure, receiver, concorrenza); **v0.4** (risorse uniche, copie ed effetti, aggregati borrowed, generics, FFI, TIR, Range ed estrazione). Le sezioni 1–18 preservano il percorso progettuale precedente, con i necessari aggiornamenti di coerenza; le sezioni 19–27 completano la specifica base. Le sezioni che descrivono confini/aperture di versioni precedenti sono storiche e non prevalgono sulle decisioni v0.4.

## 1. Filosofia

Tempra privilegia ergonomia, sintassi minimale, value semantics e memory safety statica.

Il linguaggio adotta un modello di borrowing intenzionalmente più ristretto di Rust e non espone annotazioni di lifetime.

Le restrizioni semantiche devono ridurre la complessità di compilatore, tooling e verifica senza compromettere le operazioni fondamentali su valori e memoria.

**Principio:** Tempra non deve esprimere ogni possibile relazione di lifetime. Deve permettere di risolvere gli stessi problemi attraverso API ergonomiche, ownership, ADT, Range e composizione funzionale.

Obiettivi principali:

- Tempra Script: scripting immediato, stack VM e ARC NO-CYCLE.
- Tempra Native: LLVM AOT con obiettivo zero GC/ARC.
- Un unico linguaggio con identiche garanzie semantiche.
- Compilazione, diagnostica e tooling rapidi e prevedibili.

## 2. Value semantics e ownership

`let` introduce un binding immutabile e `var` un binding mutabile.

I **valori duplicabili** possiedono semantica di copia indipendente: modificare una copia non può alterare l’originale. Le **risorse non duplicabili** restano valori con ownership, ma non ammettono copie semantiche implicite (§19). Nessuna di queste categorie introduce un secondo modello di lifetime.

L'implementazione fisica può utilizzare:

- Trasferimento di ownership quando l'ultimo utilizzo è dimostrabile.
- Riutilizzo di allocazioni.
- Condivisione interna con copy-on-write, dove consentita dal backend.
- Copie fisiche quando necessarie.

Il linguaggio non garantisce che una copia semantica abbia costo costante. **Il costo eventuale di una copia fisica o di una compattazione è un compromesso accettato** quando permette di semplificare il modello semantico, il type checker, le API e gli strumenti per sviluppatori e agenti. Questa preferenza non impone copie inutili: move e reuse restano ottimizzazioni desiderabili.

Un parametro duplicabile passato per valore non invalida automaticamente il valore del chiamante. Un move fisico è consentito quando preserva la semantica osservabile. Un valore non duplicabile richiede invece un contratto **esplicito** di trasferimento, `ref` o `inout`: l’ultimo uso non autorizza un consumo semantico implicito (§19).

## 3. Reference e accessi

`ref` fornisce accesso non proprietario in lettura.

`inout` fornisce accesso non proprietario esclusivo con capacità di mutazione.

Il compilatore verifica provenienza, validità, esclusività, aliasing incompatibile ed escape.

Le reference dinamiche sono destinate principalmente ad accessi temporanei e non costituiscono il modello ordinario delle relazioni persistenti.

### Accessi statici

Gli accessi a memoria immutabile con durata effettivamente statica possono persistere.

Il compilatore deve dimostrarne la provenienza. `let` e `comptime` non conferiscono automaticamente durata statica.

Non esistono annotazioni esplicite di lifetime nella sintassi.

## 4. Range

`Range` è un valore autonomo che descrive un intervallo.

Non contiene una reference al buffer e non ne prolunga la durata.

Può essere conservato, restituito, passato a funzioni, utilizzato per iterazione e applicato a collezioni.

La validità di un Range rispetto alla collezione viene verificata quando l'intervallo viene applicato.

**Range è preferibile a una coppia di indici quando l'intervallo rappresenta un unico concetto semantico.**

Non è tuttavia un sostituto universale di Slice o ownership.

## 5. Slice(T)

`Slice(T)` rappresenta una vista contigua, read-only e non proprietaria.

Non possiede né mantiene vivo il buffer.

### Accesso mutabile

`inout Slice(T)` consente di modificare gli elementi della memoria sottostante attraverso un accesso esclusivo.

Non viene introdotto un tipo pubblico distinto `MutableSlice(T)`.

Il compilatore deve verificare che il buffer sia effettivamente modificabile e che l'accesso esclusivo sia valido.

L'esclusività riguarda gli elementi accessibili, non semplicemente la possibilità di riassegnare il descrittore della slice.

### Provenienza

Il compilatore distingue internamente:

| Provenienza | Significato |
|---|---|
| Static | Memoria immutabile di durata statica |
| Borrowed(owner) | Vista dipendente da un proprietario dinamico |
| Temporary(scope) | Vista limitata a un accesso temporaneo |

Queste informazioni non richiedono nuove keyword pubbliche.

### Ritorni e persistenza

Una slice statica può persistere.

Una slice dinamica può essere restituita quando il compilatore dimostra la dipendenza da un singolo parametro identificabile e la validità dell'accesso nel chiamante.

Le slice dinamiche non possono essere conservate come relazioni persistenti tra oggetti indipendenti nella v1.

Le provenienze dinamiche alternative multiple non sono ammesse nei contratti di ritorno borrowed della v1.

## 6. Strategie idiomatiche per i risultati

**Non esiste una conversione universale da borrow persistente a Range.**

La scelta dipende dal contratto dell'API e dalla durata richiesta per i dati.

### 6.1 Risultati descrittivi — `FrameView`

Quando il chiamante conserva il buffer originale e la funzione deve soltanto descriverne una porzione, preferire un valore autonomo con Range.

```tempra
struct FrameView {
    version: UInt8
    payload: Range
}
```

Una funzione `inspect_frame(data: Slice(UInt8)) -> FrameView?` può restituire metadati senza copiare il payload.

La slice viene costruita temporaneamente applicando il Range al buffer del chiamante.

Questo modello non crea dipendenze di memoria persistenti. In questa convenzione il nome `View` significa **descrittore logico non proprietario**, non una `Slice` memorizzata nella struct: per leggere i dati serve applicare il `Range` a un buffer esplicito. Il compilatore deve comunque controllare i limiti al momento dell’accesso; il descrittore da solo non assicura che il contenuto del buffer non sia cambiato.

### 6.2 Risultati proprietari compatti — `OwnedPayloadFrame`

Quando il risultato deve conservare soltanto il payload, preferire una rappresentazione proprietaria:

```tempra
struct OwnedPayloadFrame {
    version: UInt8
    payload: Array(UInt8)
}
```

L'API restituisce un valore autonomo.

Il payload può richiedere una copia, una compattazione oppure un trasferimento di ownership, secondo la rappresentazione disponibile e l'analisi del compilatore.

L'estrazione di una sottoporzione da un array non implica automaticamente la possibilità di trasferire quella sottoporzione a costo zero.

### 6.3 Risultati proprietari zero-copy quando possibile — `OwnedFrame`

Quando è conveniente conservare l'intero buffer:

```tempra
struct OwnedFrame {
    version: UInt8
    data: Array(UInt8)
    payload: Range
}
```

La struct possiede il buffer e utilizza Range per descrivere il payload.

Se il buffer originale può essere trasferito fisicamente, si evita di duplicarlo.

Il compromesso è che l'intero buffer rimane allocato anche quando il payload ne occupa soltanto una parte.

### 6.4 Decisioni attraverso ADT

Quando una funzione deve selezionare tra sorgenti differenti, preferire un enum che descriva la scelta anziché restituire una reference con provenienza dinamica alternativa.

```tempra
enum Source {
    First
    Fallback
}
```

Il chiamante applica la decisione mentre possiede ancora le sorgenti.

Non servono lifetime multipli né copie dei dati.

### 6.5 Nomi che rendono esplicito il contratto

Gli esempi adottano nomi distintivi: `FrameView` (descrittore non proprietario), `OwnedFrame` (proprietario dell’intero buffer) e `OwnedPayloadFrame` (proprietario del solo payload estratto). Evitare un nome generico come `ParsedFrame` quando nasconde la modalità di ownership. Questa è una convenzione delle API, **non** una nuova keyword o un qualificatore di tipo; la semantica effettiva resta determinata da tipi, campi e contratti del compilatore.

### 6.6 Nessuna promozione proprietaria nascosta

Il type checker non deve trasformare automaticamente un borrow escaping in una copia proprietaria per rendere valido il programma.

L'ownership deve risultare dal contratto dell'API.

Il compilatore resta libero di ottimizzare copie e move fisici senza cambiare la semantica dei valori.

**Principio:** scegliere tra metadati, ownership e copia in base al significato del risultato, non in base a una regola meccanica di sostituzione delle reference.

## 7. Restrizioni semantiche

| Operazione | Regola v1 proposta |
|---|---|
| Borrow temporaneo | Ammesso |
| `inout` esclusivo | Ammesso |
| Slice statica persistente | Ammessa |
| Slice dinamica restituita da un parametro identificabile | Ammessa con prova |
| Slice dinamica persistente in oggetti indipendenti | Vietata |
| Slice dinamica attraverso sospensione async | Vietata |
| Slice dinamica catturata da closure escaping | Vietata |
| Ritorno borrowed da più proprietari alternativi | Vietato |
| Range conservato in struct | Ammesso |
| Array proprietario conservato in struct | Ammesso |
| Trasferimento fisico di buffer | Ammesso se semanticamente equivalente |
| Copia fisica necessaria per indipendenza | Ammessa secondo i contratti di effetti |

Le limitazioni non implicano necessariamente copie. Un'API può evitare completamente i borrow persistenti attraverso metadati, ADT o trasferimenti di valori proprietari.

## 8. Type checker

Il type checker deve garantire memory safety indipendentemente dal memory manager del backend.

Responsabilità:

- Provenienza degli accessi.
- Validità e durata dei proprietari.
- Esclusività di `inout`.
- Compatibilità degli alias.
- Terminazione dei borrow.
- Controllo degli escape.
- Contratti di ritorno borrowed.
- Chiusura e cattura delle closure.
- Attraversamento dei confini async e actor.
- Contratti di ownership tra moduli.

Le analisi devono essere conservative, modulari e deterministiche.

Se una relazione non può essere dimostrata valida, il compilatore la rifiuta.

Non viene introdotto un fallback runtime ARC per aggirare un errore statico di borrowing.

## 9. Tempra Script — Stack VM

Pipeline:

`Source → HIR / Type checking → TIR → Bytecode → Stack VM`

L'interprete TIR rimane disponibile per `comptime`.

Tempra Script utilizza ARC NO-CYCLE per i valori proprietari.

Priorità:

- Cold-start minimo.
- Parsing e type checking efficienti.
- Lowering leggero.
- Bytecode caching.
- Ottimizzazioni locali economiche.
- VM efficiente e manutenibile.

Non sono richieste ottimizzazioni globali costose per eseguire uno script.

ARC non sostituisce le prove statiche necessarie per reference, slice e accessi esclusivi.

## 10. Tempra Native — LLVM AOT

Tempra Native utilizza le stesse regole semantiche di Tempra Script.

Obiettivo architetturale: **zero GC e zero reference counting per la memoria gestita da Tempra nel codice nativo**.

Strategie candidate:

- Ownership statica.
- Trasferimento dei buffer.
- Uniqueness e reuse.
- Ottimizzazione e rimozione delle copie.
- Escape e storage analysis.
- Distruzione diretta.
- Allocazione aggregata e reclamation alternative, quando utili.

Nano-regions non sono un requisito.

Le copie fisiche restano legittime quando necessarie per preservare l'indipendenza semantica di più valori.

L'assenza completa di RC è un obiettivo da validare sull'intero runtime nativo, non una proprietà già dimostrata.

Librerie esterne e FFI possono adottare modelli di memoria differenti.

## 11. Effects e parità dei backend

Tempra Script e Tempra Native devono preservare comportamenti osservabili equivalenti.

L'implementazione fisica delle copie può differire.

Il compilatore non può utilizzare un'ottimizzazione contingente come unica giustificazione per accettare una funzione che vieta allocazioni.

La semantica definitiva di `@deny(alloc)` rispetto alle copie potenzialmente allocanti deve essere formalizzata.

Non sono ammesse copie proprietarie implicite introdotte esclusivamente per aggirare un errore di borrowing.

## 12. Diagnostica e tooling

Formatter, linter, LSP, MCP e Agent Skills devono operare sugli stessi contratti semantici del compilatore.

Le diagnostiche devono suggerire alternative appropriate:

| Problema | Alternative |
|---|---|
| Borrow dinamico da conservare | Valore proprietario |
| Occorre soltanto descrivere una porzione | Range |
| Selezione fra sorgenti dinamiche | Enum ADT |
| Occorre conservare il contenuto | Array proprietario |
| Accesso mutabile temporaneo | `inout` |
| Accesso a dati statici | Provenienza statica inferita |

Il suggerimento deve considerare il contratto desiderato, non imporre automaticamente una copia o una rappresentazione con Range. Quando il risultato è proprietario, il tooling dovrebbe renderlo riconoscibile anche nel naming delle API e proporre, se utile, una conversione esplicita in un valore owned.

## 13. Stato delle decisioni — consolidamento v0.4

La presente v0.4 è l'**autorità normativa unica** per il modello di memoria nel
perimetro definito. La lista seguente aggiorna lo stato dei punti storici:

1. **SPECIFICATO v0.2:** terminazione dei borrow nel control flow (§14).
2. **SPECIFICATO v0.4:** struct temporanee con slice borrowed (§21).
3. **SPECIFICATO v0.3:** closure e catture esplicite (§16).
4. **SPECIFICATO v0.3:** task, actor isolati, sospensione, cancellazione (§17);
   **actor handle DEFERRED POST NO-CYCLE** (§17.9, §26).
5. **SPECIFICATO v0.4:** copie, `@deny(alloc)` e obblighi dei backend (§20).
6. **SPECIFICATO v0.4:** generics e static duck interfaces (§21).
7. **SPECIFICATO v0.4 nel confine safe:** FFI e memoria esterna (§22);
   ABI e `@unsafe` concreti rimandati.
8. **SPECIFICATO v0.4 come contratto/verifier:** ownership-aware TIR (§23);
   opcode, layout e schema di serializzazione rimandati.
9. **SPECIFICATO v0.4 semanticamente:** Range half-open e controlli (§24);
   rappresentazione fisica rimandata.
10. **SPECIFICATO v0.4 semanticamente:** estrazione/compattazione dei buffer (§24).
11. **OBIETTIVO DA VALIDARE:** runtime Tempra Native RC-free, non ancora una prova (§26).

Un punto "specificato" non equivale a "implementato": parser, checker, VM,
backend, runtime, documentazione e test di parità devono ancora convergere.

---

## Principio conclusivo v0.1

**Ownership per conservare. Borrowing per accedere. Range per descrivere. ADT per rappresentare alternative.**

Le copie sono una possibilità, non un obbligo. I move sono un'ottimizzazione o un trasferimento di ownership, non una violazione delle value semantics.

La scelta della rappresentazione deve seguire la semantica dell'API, mantenendo minimale il modello di lifetime di Tempra. **Il costo occasionale di una copia è accettabile se evita complessità permanente nel linguaggio e nel tooling.**

---

## 14. v0.2 — Borrow Flow & Access Checking

**Stato:** approvato come specifica progettuale, non ancora prova formale o stato implementativo.

### 14.1 Scopo e principio

Il type checker verifica borrow attraverso `if`, `else`, `match`, `for`, chiamate e ritorni.
Non vengono introdotte keyword, annotazioni di lifetime o nuovi tipi di reference.
L'analisi segue il flusso del controllo e gli utilizzi effettivamente raggiungibili:
**il prestito termina dopo l'ultimo utilizzo su ciascun percorso**, non necessariamente
alla fine dello scope lessicale.

La regola fondamentale rimane: un borrow può restare attivo soltanto finché
il proprietario è valido e non si verificano accessi incompatibili.

```tempra
var data = [10, 20, 30, 40]
let view = data[0..2]
inspect(view)
data[0] = 99 // Ammesso: nessun altro utilizzo di view.
```

Viceversa:

```tempra
var data = [10, 20, 30, 40]
let view = data[0..2]
data[0] = 99 // Errore: la view resta viva.
inspect(view)
```

Il compilatore **non** trasforma implicitamente `view` in una copia per sanare il conflitto.

### 14.2 Prestiti, provenienza e modalità di accesso

Le provenienze della v0.1 rimangono:

- `Static`: memoria immutabile di durata statica dimostrata.
- `Borrowed(owner)`: vista dipendente da un proprietario dinamico identificato.
- `Temporary(scope)`: vista limitata a un accesso temporaneo.

Per controllare gli accessi il compilatore può rappresentare internamente:

```text
Loan {
    origin
    access_mode     // Read | Exclusive
    validity_boundary
    live_state
}
```

Questa struttura è concettuale, non una prescrizione sul layout dei dati del compilatore.
`Read` è condivisibile con altri accessi in lettura compatibili.
`Exclusive` permette mutazione, ma esclude accessi incompatibili allo stesso proprietario.

### 14.3 Branch `if`/`else`: controllo per percorso

I rami mutuamente esclusivi vengono verificati separatamente.
Un borrow creato e consumato interamente in un ramo non blocca accessi negli altri.

```tempra
var data = [10, 20, 30]

if condition {
    let view = data[0..2]
    inspect(view)
} else {
    data[0] = 99
}
process(data)
```

**Ammesso.** Alla convergenza non esiste alcun prestito residuo da quei rami.

#### Convergenza di valori borrowed

Quando una slice sopravvive al punto di convergenza, il suo contratto deve avere
una provenienza dimostrabile secondo una regola piccola e conservativa:

| Provenienza ramo A | Provenienza ramo B | Convergenza |
|---|---|---|
| `Static` | `Static` | `Static` |
| `Borrowed(X)` | `Borrowed(X)` | `Borrowed(X)` |
| `Static` | `Borrowed(X)` | `Borrowed(X)` (conservativamente) |
| `Borrowed(X)` | `Borrowed(Y)` con X ≠ Y | Rifiutata |
| Valore borrowed | Nessun valore borrowed | Ammessa solo se non occorre un risultato borrowed comune |

Se tutti i rami restituiscono viste riconducibili allo stesso parametro proprietario,
la funzione può esportare un unico contratto borrowed:

```tempra
fn select_part(data: Slice(UInt8), first: Bool) -> Slice(UInt8) {
    if first {
        data[0..2]
    } else {
        data[2..4]
    }
}
```

*Gli intervalli sono schematici e richiedono indici validi; il codice non promette
automaticamente assenza di panic per input corti.*

La funzione che restituisce alternativamente `first` o `second`, provenienti
da due parametri dinamici indipendenti, viene rifiutata in v0.2.

**Pattern alternativo:** restituire un enum (`Source.First` o `Source.Second`)
e applicare la decisione nel contesto dove entrambi i proprietari sono validi.

### 14.4 `match` e ADT

Ogni arm di `match` è analizzato come un ramo separato.
Se un'espressione `match` produce una slice borrowed che sopravvive,
si applicano le regole di convergenza della sezione 14.3.

Un `match` non estende la vita del proprietario e non permette di inserire
slice dinamiche in ADT persistenti vietati dalla v0.1.

Gli ADT proprietari e i Range restano valori autonomi.

### 14.5 Cicli: prestiti per iterazione

I borrow dinamici creati durante una singola iterazione devono terminare
prima dell'iterazione successiva, salvo uscita immediata valida (`return`, `break`
che non trasporta una vista borrowed illegale).

```tempra
var data = [10, 20, 30, 40]

for i in 0..data.len() {
    let view = data[i..i+1]
    inspect(view)
    data[i] = 0 // Ammesso: ultimo uso di view terminato.
}
```

Un borrow in lettura creato prima del ciclo può essere mantenuto durante
tutto il ciclo, a condizione che non venga invalidato.

```tempra
let view = data[0..data.len()]
for value in view {
    inspect_byte(value)
}
```

È invece rifiutata la mutazione della memoria posseduta da `data` mentre
un `for` la legge attraverso una slice ancora attiva.

Per la v0.2 il compilatore **non** deve dimostrare in generale
la disgiunzione di due Range per autorizzare accessi concorrenti
allo stesso proprietario.

La vista dinamica creata in un'iterazione non può essere conservata
in uno stato borrowed usato nell'iterazione successiva.
Un `Range` value-based può essere mantenuto e applicato al buffer dopo il ciclo:

```tempra
var selected = 0..0

for i in 0..data.len() {
    if matches(data[i]) {
        selected = i..i+1
    }
}
inspect(data[selected])
```

### 14.6 Ritorno anticipato dai cicli

Il ritorno di una slice borrowed direttamente da un'iterazione è ammesso
quando la provenienza del risultato è un unico parametro identificabile.

```tempra
fn before_zero(data: Slice(UInt8)) -> Slice(UInt8)? {
    for i in 0..data.len() {
        if data[i] == 0 {
            return Some(data[0..i])
        }
    }
    None
}
```

Non viene trasportata una vista borrowed tra iterazioni: il ritorno
termina immediatamente il ciclo.

### 14.7 Chiamate e prestiti temporanei

Il tipo del parametro stabilisce l'accesso richiesto:

- `Slice(T)`: lettura non proprietaria.
- `inout Slice(T)`: accesso esclusivo alla memoria sottostante.

```tempra
fn inspect(data: Slice(UInt8)) {
    // Lettura.
}

fn normalize(inout data: Slice(UInt8)) {
    // Scrittura degli elementi.
}
```

Se la funzione chiamata non restituisce né conserva prestiti, l'accesso
temporaneo necessario termina al completamento della chiamata.

```tempra
inspect(data[0..4])
normalize(inout data[0..4])
```

Le chiamate sono sequenzialmente compatibili quando il proprietario è mutabile
e non restano altri prestiti vivi.

**Aliasing degli argomenti:** due argomenti provenienti dallo stesso
proprietario non possono rappresentare accessi incompatibili simultanei.
Per la v0.2 l'analisi dei buffer dinamici è conservativa a livello
di proprietario, senza prova generale della disgiunzione fra intervalli.

### 14.8 Contratti inferiti dei ritorni borrowed

Una funzione può restituire una `Slice(T)` che dipende da un solo parametro
riconoscibile. Il contratto inferito contiene la posizione della sorgente
e la modalità dell'accesso restituito.

```tempra
fn prefix(data: Slice(UInt8), count: Int64) -> Slice(UInt8) {
    data[0..count]
}
```

Contratto interno concettuale:

```text
ReturnBorrow {
    source = parameter(0)
    mode = Read
}
```

Il chiamante eredita l'obbligo di preservare il proprietario finché usa
la vista restituita.

Una funzione può modificare una slice attraverso un parametro `inout`
e restituire una vista read-only sullo stesso proprietario, purché termini
l'accesso esclusivo e il chiamante rispetti il nuovo prestito:

```tempra
fn normalize_prefix(inout data: Slice(UInt8)) -> Slice(UInt8) {
    normalize(inout data)
    data[0..2]
}
```

*Esempio schematico con precondizione di lunghezza valida.*

Non è consentito restituire una slice che punta a un array locale
destinato alla distruzione:

```tempra
fn invalid() -> Slice(UInt8) {
    let data = [10, 20, 30]
    data[0..2] // Errore: provenienza locale non più valida al ritorno.
}
```

Una slice con provenienza `Static` può invece essere restituita e conservata.

### 14.9 Compilazione modulare e riepiloghi di borrow

Il compilatore deve poter esportare un riepilogo semantico sufficiente
a verificare una chiamata importata senza analizzare nuovamente il corpo.

```text
BorrowSummary {
    parameter_accesses
    return_provenance
    return_access_mode
    static_requirements
}
```

I contratti inferiti fanno parte dei metadata semantici del modulo.
Modifiche ai contratti invalidano le cache dipendenti.

Il formato definitivo, i casi ricorsivi e l'interazione avanzata
con generics e static duck interfaces appartengono alle specifiche successive.

### 14.10 Strategia del type checker

Il checker usa informazioni di control flow e di utilizzo:

1. Identifica proprietari e provenienze.
2. Identifica prestiti `Read` e `Exclusive`.
3. Determina gli usi raggiungibili dei valori borrowed.
4. Termina i prestiti dopo l'ultimo uso valido su ogni percorso.
5. Rileva conflitti, mutazioni e invalidazioni.
6. Verifica i join dei branch.
7. Verifica ingressi, iterazioni e uscite dei cicli.
8. Inferisce i contratti di chiamata e di ritorno.

Non sono richiesti nella v0.2: annotazioni di lifetime, generics sui lifetime,
solver SMT, provenienze multi-owner generali, dimostrazione arbitraria
della disgiunzione di Range o copie automatiche per correggere prestiti invalidi.

In assenza di prova secondo queste regole il codice viene rifiutato.

### 14.11 Diagnostica

L'errore deve riportare proprietario, origine del prestito, accesso incompatibile,
utilizzo che mantiene vivo il prestito e possibile trasformazione idiomatica.

```text
error: cannot mutate 'data' while a borrowed slice is live

borrow created:
  let view = data[0..2]

conflicting mutation:
  data[0] = 99

borrow remains live because:
  inspect(view)

help: move the mutation after the last use of 'view'
```

Gli stessi dati devono essere disponibili a LSP, MCP e tool per agenti.

### 14.12 Matrice minima di conformità

| Caso | Esito |
|---|---|
| Lettura e successiva mutazione dopo ultimo uso | Accettato |
| Mutazione prima dell'ultimo uso borrowed | Rifiutato |
| Borrow consumati in branch separati | Accettato |
| Join/return borrowed con singolo proprietario | Accettato |
| Join/return borrowed da proprietari indipendenti | Rifiutato |
| Borrow statico persistente | Accettato |
| Borrow creato e consumato per iterazione | Accettato |
| Borrow stabile preesistente durante il ciclo | Accettato |
| Borrow dinamico trasportato tra iterazioni | Rifiutato |
| Borrow restituito da un parametro durante il ciclo | Accettato |
| Ritorno borrowed da memoria locale | Rifiutato |
| Chiamate sequenziali read/inout compatibili | Accettate |
| Argomenti contemporaneamente borrowed incompatibili | Rifiutati |
| Ritorno read-only dopo parametro inout | Accettato, con vincoli |

I test su Tempra Script e Tempra Native devono verificare lo stesso comportamento
osservabile sul perimetro implementato.

### 14.13 Confini della v0.2 (storico)

**Questa lista registra lo stato alla v0.2, non lo stato attuale v0.4.**

Questa versione **non decideva ancora**:

- Struct temporanee che contengono slice dinamiche.
- Closure che catturano valori proprietari o borrowed.
- Actor, task, sospensione e cancellazione.
- Effects delle copie e interazione completa con `@deny(alloc)`.
- Contratti avanzati di generics e static duck interfaces.
- FFI e accessi a memoria esterna.
- Formato e operazioni finali dell'ownership-aware TIR.
- Forma definitiva del tipo `Range` e relativa indicizzazione.
- Prova formale e performance misurate di un backend RC-free.

## 15. Principio conclusivo v0.2

**Ownership per conservare. Borrowing per accedere. Range per descrivere.
ADT per rappresentare alternative.**

**I borrow seguono gli ultimi utilizzi sui percorsi del control flow; il
compilatore verifica provenienza ed esclusività senza richiedere lifetime
espliciti o relazioni multi-owner arbitrarie.**

La v0.2 è una decisione progettuale approvata: implementazione e prove
formali rimangono milestone distinte.

---

## 16. v0.3 — Closure & Capture Semantics

**Stato:** approvato come contratto progettuale. La sintassi di riferimento è la
*Tempra Surface Syntax v0.1*, §22. Non si introduce una sintassi alternativa,
né un modello specifico di lifetime per le closure.

### 16.1 Identità del modello

Una closure è un valore callable con un ambiente. Le sue catture usano
**le stesse** operazioni di accesso e ownership delle normali funzioni:

- `ref`: accesso non proprietario in lettura;
- `inout`: accesso non proprietario, esclusivo e mutabile;
- `consume`: **trasferimento semantico** di ownership nella closure; il binding
  sorgente non può più essere utilizzato dal contesto esterno dopo la cattura.

**Non esiste un default di cattura.** Tutti i nomi esterni utilizzati da una
closure devono essere dichiarati esplicitamente in `fn[...]`, con la modalità
prevista dalla grammatica. Non si introduce una keyword `captures`.

```tempra
let offset = 10
let add_offset = fn[ref offset](x: Int) -> Int {
    offset + x
}
```

Una closure senza catture usa `fn(...)`; `fn[]` resta vietato, come da Surface
Syntax. Le funzioni nominate `fn name(...)` non hanno un ambiente lessicale
catturato. Il tipo callable pubblico rimane `fn(Params...) -> Return`.

### 16.2 Regole delle catture `ref` e `inout`

Il prestito catturato è soggetto a provenienza, esclusività, ultimo utilizzo,
validità del proprietario e controllo degli escape definiti in v0.1 e v0.2.

Le closure che catturano dati **dinamici** mediante `ref` o `inout` devono essere
**non-escaping** rispetto al confine di accesso. Possono essere invocate più
volte finché tale accesso resta valido: una cattura `inout` può quindi aggiornare
il valore esterno, e l'aggiornamento persiste tra chiamate nello stesso
confine non-escaping. Non può però essere conservata per un uso dopo la fine
dell'accesso.

```tempra
var total = 0
let accumulate = fn[inout total](x: Int) {
    total = total + x
}

accumulate(2)
accumulate(3)
// La mutazione riguarda il binding esterno total.
// La closure non deve sopravvivere al prestito esclusivo.
```

*Esempio semantico, non attestazione che il percorso di compilazione delle
closure mutabili sia già completo.*

Una cattura `ref` con provenienza realmente `Static` può persistere secondo
la regola generale della v0.1. L'immutabilità di `let` **non** equivale
alla durata statica.

### 16.3 `consume`: proprietà, non borrowing

La cattura `consume` esegue il trasferimento semantico già previsto nella
Surface Syntax. **Dopo la creazione della closure il binding sorgente viene
invalidato**. Non bisogna reinterpretare `consume` come una copia indipendente
che lascia automaticamente disponibile il sorgente.

```tempra
let data = load_data()
let process_data = fn[consume data]() {
    process(data)
}
// data non è più utilizzabile qui.
```

Il compilatore può realizzare il trasferimento tramite move/reuse del buffer,
oppure mediante altre trasformazioni lecite. La disponibilità della sorgente
resta determinata dalla semantica `consume`, **non** dall'ottimizzazione scelta.

Una closure che possiede il proprio ambiente può essere persistente, restituita
o memorizzata quando il suo contenuto soddisfa gli ordinari vincoli di
ownership, NO-CYCLE e copiabilità dei valori. In particolare, la presenza di
risorse non duplicabili vieta la duplicazione implicita della closure; si applicano
le regole generali della §19.

### 16.4 Parametri per valore: move *fisico* inferito

Per chiamate di funzioni, metodi e closure, un parametro non qualificato da
`ref` o `inout` conserva le normali **value semantics**.

```tempra
fn process(data: Array(UInt8)) {
    // ...
}

let bytes = load_data()
process(bytes)
// Se bytes non viene più osservato, il buffer può essere trasferito fisicamente.
```

Il compilatore **può** sostituire una copia fisica con un trasferimento quando
dimostra che la sorgente non sarà più osservata e non esistono prestiti che
richiedano la stabilità di quella rappresentazione. Questo è un fatto
dell'implementazione, non una conversione semantica del parametro in `consume`.

Anche all'ultimo uso, copiare in registri una struct piccola può essere più
efficiente che spostare indirettamente il suo storage. Non esiste una soglia
di dimensione nella semantica pubblica e l'ottimizzatore può scegliere secondo
ABI e target, preservando il risultato osservabile.

Per contro, un `consume` **esplicito** invalida il sorgente per contratto.
Questa distinzione deve apparire nella diagnostica e nel lowering.

### 16.5 Escape delle closure e contratti dei callback

Il compilatore deve distinguere se una funzione usa il callback esclusivamente
durante la chiamata oppure se può restituirlo, conservarlo o trasferirlo.
Il contratto di non-escape può essere inferito e pubblicato nei metadata
semantici del modulo, analogamente ai `BorrowSummary` della v0.2.

| Callback | Catture dinamiche `ref` / `inout` |
|---|---|
| Provato non-escaping | Ammesse nel confine del prestito |
| Escaping | Vietate |
| Escape non dimostrabile | Trattato come escaping, quindi rifiutato |
| Catture proprietarie `consume` | Consentite, se rispettano gli altri vincoli |
| Cattura `ref` statica e immutabile | Consentita se la provenienza è provata |

La stdlib deve poter offrire API come `for_each` con contratti verificabili;
non si introducono annotazioni di lifetime nell'API pubblica. I callback
provenienti da FFI senza contratto affidabile sono trattati conservativamente.

### 16.6 Ricorsione esplicita con `fn[self]`

La Surface Syntax già prevede **closure ricorsive esplicite**:

```tempra
let factorial = fn[self](n: Int) -> Int {
    if n <= 1 {
        1
    } else {
        n * self(n - 1)
    }
}
```

`self` nella lista di cattura è il marcatore speciale **già esistente** per
la ricorsione; non è una quarta modalità di cattura e non si scrive `ref self`
o `consume self` per la ricorsione diretta.

La ricorsione `fn[self]` deve mantenere identità e comportamento garantiti
dalla Surface Syntax, **senza materializzare un arco proprietario forte dalla
closure verso se stessa**. Una possibile implementazione è il richiamo
all'ambiente corrente tramite un riferimento interno non proprietario o
una ricorsione diretta attraverso il codice generato, ma il layout resta
implementation-defined.

Non è consentito introdurre cicli forti mediante normali catture `consume`
o catene di ambienti proprietari. Questo è un requisito di NO-CYCLE generale,
non una seconda disciplina di memoria per le closure.

Il `self` ricorsivo non si propaga implicitamente alle closure annidate.
L'uso annidato resta soggetto alla sintassi esplicita già documentata e
alle regole di provenienza/escape; il caso `fn[ref self]` annidato non
conferisce automaticamente un lifetime persistente.

Il trattamento di casi avanzati in cui il valore ricorsivo `self` viene
restituito o immagazzinato resta vincolato all'invariante aciclica ed è da
verificare nell'implementazione: non autorizza un ciclo forte.

### 16.7 Closure e backend

La rappresentazione ordinaria può essere vista come codice invocabile +
ambiente, ma non è un nuovo tipo pubblico o una garanzia di layout.

- **Tempra Script:** l'ambiente proprietario può usare ARC NO-CYCLE; un prestito
  non-escaping resta controllato staticamente. La VM non deve dipendere da
  escape analysis globale costosa per eseguire una closure.
- **Tempra Native:** ambienti con ownership verificabile possono utilizzare move,
  copie fisiche, reuse e distruzione diretta. Nessun riferimento catturato
autorizza l'introduzione silenziosa di un runtime RC nel programma AOT.

Le closure senza ambiente possono essere abbassate come callable senza
storage ambientale, quando le condizioni del backend lo permettono.

### 16.8 Conformità closure

| Scenario | Esito v0.3 |
|---|---|
| Cattura implicita di un nome esterno | Rifiutata |
| `fn[ref x]` dinamica non-escaping | Ammessa |
| `fn[inout x]` dinamica non-escaping, sorgente mutabile | Ammessa |
| `fn[ref x]` statica immutabile escaping | Ammessa |
| `fn[ref x]` dinamica escaping | Rifiutata |
| `fn[inout x]` dinamica escaping | Rifiutata |
| `fn[consume x]` | Trasferisce ownership e invalida x |
| Passaggio di parametro `T` a ultimo uso | Move fisico possibile; semantica by-value invariata |
| `fn[self]` ricorsiva | Ammessa senza strong self-cycle |
| Strong ownership cycle tramite capture | Rifiutato |
| Copia di closure con risorse non duplicabili | Vietata; possibile trasferimento esplicito della closure |

## 17. v0.3 — Async, Actors, Tasks & Method Receivers

**Stato:** approvato nel perimetro qui esplicitato. La rappresentazione degli
*actor handle* è **DEFERRED POST NO-CYCLE**.

### 17.1 Una sola semantica anche nella concorrenza

Task, actor, callback e metodi non introducono un nuovo modello di
ownership: valori proprietari restano proprietari; `ref` resta un accesso
in lettura; `inout` resta esclusivo; `consume` resta trasferimento esplicito;
il compilatore può inferire move **fisici** per passaggi by-value al loro
ultimo uso valido.

Il runtime e il modello di scheduling non possono rendere validi borrow che
il type checker avrebbe rifiutato nel codice sincrono.

### 17.2 Sospensione `.await()`: niente prestiti dinamici attivi

Un prestito dinamico ancora vivo non può attraversare un punto di
sospensione `.await()` nella v1. Il divieto si applica a `ref`, `inout`,
`Slice(T)` dinamiche e a closure non-escaping che le catturano.

Sono invece ammessi attraverso la sospensione valori **proprietari**,
metadati come `Range`, closure con ambiente proprietario e viste immutabili
con provenienza realmente statica.

```tempra
let header = 0..16
let response = request().await()
process(buffer[header]) // Nuova vista creata dopo la sospensione.
```

Un prestito utilizzato soltanto prima di `.await()` è già terminato secondo
la v0.2 e non blocca la sospensione:

```tempra
let view = buffer[0..16]
inspect(view) // Ultimo uso di view.
let response = request().await()
process(buffer)
```

È invece rifiutato il caso in cui il borrow sia ancora vivo al momento della
sospensione e debba essere usato dopo `.await()`.

Questo requisito riguarda i **borrow**, non l'uso di un oggetto proprietario
che rimane vivo e posseduto durante la sospensione.

### 17.3 `spawn`: valori autonomi, nessun borrow dinamico catturato

Un task spawned può acquisire valori proprietari e accessi immutabili statici.
Non può acquisire `ref` o `inout` di memoria dinamica esterna, anche quando
il task è inserito in un gruppo strutturato: è una restrizione deliberata v1.

Se `spawn` avvia una closure, le catture rimangono **esplicite**; non compare
una modalità di cattura implicita specifica della concorrenza.

I valori by-value inviati al task mantengono la loro semantica di valore.
Il backend può riutilizzare il buffer o trasferirlo fisicamente quando
l'ultimo utilizzo è provato. Per risorse non duplicabili è necessario
un contratto di trasferimento che ne preservi l'unicità.

### 17.4 Actor: stato isolato e messaggi proprietari

Ogni actor possiede uno stato mutabile isolato. Nessun altro actor può
conservare o esercitare una reference dinamica diretta su quello stato.

I messaggi che attraversano un confine actor sono valori semanticamente
autonomi, mai borrow dinamici su storage appartenente al mittente.
Il runtime può ottimizzare mediante move fisico senza alterare la semantica.

Il modello actor-only e l'isolamento degli accessi evitano data race
sullo stato gestito secondo il contratto; **non dimostrano l'assenza
di deadlock logici**, attese cicliche o starvation.

### 17.5 Actor e sospensione

Un actor non può mantenere un `ref`, `inout` o una slice borrowed dinamica
sul proprio stato attraverso `.await()`.

Può elaborare lo stato prima della sospensione, ricavare risultati
proprietari o metadati, sospendere, quindi creare nuovi accessi dopo il
resume. Se l'actor può processare altri messaggi durante la sospensione,
lo stato potrebbe essere cambiato: la memory safety **non** implica
l'atomicità logica di operazioni che attraversano `.await()`.

La policy di reentrancy/scheduling dell'actor non è decisa dalla presente
specifica e non modifica i divieti di borrow dinamico sospeso.

### 17.6 Structured concurrency e cancellazione

I task figli di un gruppo strutturato devono essere completati o terminati
secondo il contratto del gruppo prima che l'ownership del gruppo venga
distrutta. Non devono sopravvivere arbitrariamente allo scope senza essere
trasferiti a un supervisore proprietario esplicito.

La cancellazione può essere cooperativa: richiedere `cancel` non autorizza
la distruzione immediata di risorse che il task potrebbe ancora usare.
Il runtime deve terminare l'esecuzione pertinente e completare il cleanup
prima di liberare storage proprietario.

Il risultato di un task è un valore autonomo o un accesso statico valido,
non una vista borrowed sullo stack di un task ormai terminato.

Le policy specifiche di propagazione degli errori e di cancellazione del
gruppo richiedono una specifica API successiva.

### 17.7 Metodi e receiver `self`

**`self` di un metodo non ha una disciplina di memoria speciale.**
I metodi applicano le stesse regole dei parametri ordinari; in particolare:

| Receiver | Contratto | Stato della grafia |
|---|---|---|
| `self` | Passaggio per valore (value semantics) | Documentato nella Surface Syntax |
| `inout self` | Accesso esclusivo e mutabile | Documentato nella Surface Syntax |
| `ref self` | Accesso in lettura, borrowed | Concettualmente coerente; grafia da verificare nel parser |
| `consume self` | Trasferimento semantico del receiver, sorgente invalidata | Concettualmente approvato per ownership di risorse; grafia da verificare nel parser |

`self` in un metodo **non** è il `self` speciale di ricorsione `fn[self]`.
Quest'ultimo è già definito nella grammatica delle closure (§16.6).

I receiver by-value possono essere ottimizzati con copia in registri,
move fisico o reuse quando equivalenti. Il compilatore non è obbligato
a effettuare move e non deve invalidare il binding originale di un
normale parametro per valore.

### 17.8 I/O asincrono: Reader e Writer

Una sospensione non consente di trasformare un parametro `inout self` in
un accesso proprietario. Se un'operazione deve mantenere un `Reader` o un
`Writer` dinamico durante `.await()`, lo stato utilizzato dalla continuazione
deve essere **posseduto**, non soltanto preso in prestito.

Per un reader o writer composto da valori ordinariamente duplicabili,
un receiver by-value può seguire le regole generali e beneficiare di
move fisico all'ultimo utilizzo.

Per file descriptor, socket e altre risorse **non duplicabili**, un
contratto di trasferimento semantico (`consume`) permette invece di
cedere l'ownership a un task o a un oggetto che ne custodisca lo stato.
Non è sufficiente che l'ottimizzatore rilevi l'ultimo uso di un normale
by-value per legittimare la duplicazione semantica di una risorsa
intrinsecamente unica.

**Schema di API illustrativo, non firma di stdlib né grafia di receiver
attualmente attestata dal parser:**

```tempra
extend Reader {
    fn read(consume self, count: UInt64) -> Task((Reader, Result(Array(UInt8), IoError))) {
        // La continuazione possiede Reader durante la sospensione.
        // A conclusione valida restituisce Reader e il risultato.
    }
}
```

Tale schema mette in evidenza il trasferimento del receiver e il suo
recupero all'uscita ordinaria. Non determina ancora il modo in cui
un task annullato, un'operazione fallita o una risorsa chiusa
restituisca oppure distrugga il reader. Questi percorsi devono essere
specificati nel modello generale delle risorse non duplicabili e
nell'API I/O, senza eccezioni nascoste per `self`.

Un metodo `inout self` **sincrono** rimane normale e utile. Può anche
creare o registrare un'operazione che non conservi il borrow dopo il
ritorno. Non può invece mantenere il prestito attivo nell'esecuzione
sospesa.

### 17.9 Actor handle — DEFERRED POST NO-CYCLE

La presente specifica **non stabilisce** la semantica degli handle actor,
la loro lifetime policy, l'eventuale separazione fra identità e ownership,
la validazione degli identificativi, i supervisori o la gestione di
mailbox e messaggi pendenti.

Motivo: questi dettagli interagiscono con la garanzia NO-CYCLE e con il
lifetime degli actor. Saranno progettati **dopo** la stabilizzazione di
ARC NO-CYCLE, con una decisione indipendente.

Nessun riferimento a un actor in questo documento autorizza a
supporre che un handle sia una strong reference oppure un identificatore
non proprietario. Nessuna di queste alternative è qui approvata.

### 17.10 Conformità concorrenza e receiver

| Scenario | Esito v0.3 |
|---|---|
| Valore proprietario attraverso `.await()` | Ammesso |
| `Range` attraverso `.await()` | Ammesso |
| `Slice(T)` con provenienza `Static` attraverso `.await()` | Ammessa |
| Borrow dinamico terminato prima di `.await()` | Ammesso |
| Borrow dinamico ancora attivo attraverso `.await()` | Rifiutato |
| Task spawned che conserva `ref` dinamico esterno | Rifiutato |
| Task spawned che riceve valori proprietari | Ammesso |
| Messaggio actor con slice dinamica borrowed | Rifiutato |
| Actor che mantiene `inout` sullo stato attraverso `.await()` | Rifiutato |
| Metodo `inout self` sincrono | Ammesso |
| Metodo che possiede il receiver durante una sospensione | Ammesso con contratto valido |
| Copia implicita di handle esterno non duplicabile | Non autorizzata |
| Actor handle e policy di lifetime | DEFERRED POST NO-CYCLE |

## 18. Stato della specifica dopo v0.3 (registro storico)

### 18.1 Decisioni consolidate

La v0.3 integra le decisioni approvate senza introdurre un nuovo modello
pubblico di memoria:

- Catture closure esplicite, senza default, con `ref`, `inout`, `consume`;
- `consume` semantico distinto da move fisico automatico sui parametri by-value;
- `fn[self]` ricorsivo conservato senza cicli forti;
- closure borrowed solo nei confini consentiti dalle regole di provenienza;
- valori proprietari oltre la sospensione; borrow dinamici attivi vietati oltre `.await()`;
- task spawned senza borrow dinamici esterni e actor con stato isolato;
- structured concurrency e cancellazione rispettose dell'ownership;
- receiver di metodi regolati come normali parametri.

### 18.2 Decisioni che erano ancora aperte nella v0.3

**Registro storico:** per lo stato normativo dopo la v0.4 consultare §13 e §26.

- **Actor handle: DEFERRED POST NO-CYCLE**, separato dalla chiusura del
  modello base qui adottato.
- Struct temporanee con `Slice(T)` borrowed dinamiche.
- Risorse non duplicabili, distruzione, cancellazione e FFI.
- Effects delle copie, soprattutto `@deny(alloc)` e la parità tra backend.
- Generics e static duck interfaces con contratti borrowed avanzati.
- Forma ABI e metadata definitivi per ownership-aware TIR.
- Convenzioni sintattiche non ancora attestate per `ref self`/`consume self`.
- Definizione finale di Range, estrazione/compattazione dei buffer.
- Implementazione, test di conformità e prove di NO-CYCLE/RC-free.

### 18.3 Autorità delle specifiche e compatibilità

La *Tempra Surface Syntax v0.1* resta il riferimento per la grammatica
attualmente documentata. In particolare, `fn[ref x]`, `fn[inout x]`,
`fn[consume x]` e `fn[self]` sono sintassi esistenti nella specifica.
La presente specifica di memoria determina i vincoli progettuali di
validità, escape e ownership e non modifica automaticamente lexer o parser.

**Nota di allineamento:** la Surface Syntax v0.1 descrive ancora alcune
operazioni sulle slice come mutabili direttamente; questa Memory Spec
adotta invece `Slice(T)` read-only e `inout Slice(T)` per la mutazione.
È un aggiornamento semantico progettuale da sincronizzare con la Surface
Spec e con l'implementazione, **non** una dichiarazione che il parser/type
checker attuale lo applichi già.

Le sezioni storiche v0.1/v0.2 conservate qui non devono essere lette come
un'approvazione di feature marcate "aperte" prima della v0.3: dove esiste
un conflitto di stato, prevalgono le decisioni v0.3 esplicitamente
approvate nel relativo perimetro.

### 18.4 Principio conclusivo v0.3 (storico)

**Ownership per conservare. Borrowing per accedere. Range per descrivere.
ADT per rappresentare alternative.**

Funzioni, metodi, closure, task e actor applicano **un'unica semantica**.
Il compilatore sceglie liberamente la strategia fisica di copie e move
quando il programma mantiene lo stesso comportamento osservabile.
Non si introducono lifetime espliciti, modalità di cattura implicite,
nuovi modelli di reference per async o fallback ARC per sanare borrow
invalidi.

L'obiettivo zero GC/RC per Tempra Native rimane da dimostrare, mentre Tempra Script
mantiene ARC NO-CYCLE e privilegia cold-start, semplicità della VM e
tempi di compilazione contenuti.

---

## 19. v0.4 — Risorse non duplicabili, trasferimento e distruzione

**Stato:** decisione semantica approvata. Si applica a funzioni, metodi,
closure, contenitori, task, actor e interfacce statiche senza un secondo
modello di ownership.

### 19.1 Un solo modello, due capacità dei valori

I tipi posseduti sono classificati rispetto alla possibilità di **duplicare
semanticamente** un valore:

| Capacità | Semantica | Esempi concettuali |
|---|---|---|
| Duplicabile | La copia produce un valore indipendente | Numeri, `Range`, `Array(T)` di elementi duplicabili, struct duplicabili |
| Non duplicabile | Esiste una sola ownership della specifica risorsa; nessuna copia semantica implicita | File descriptor posseduto, socket proprietario, handle esterno con release unico |
| Borrowed | Accesso non proprietario; nessuna duplicazione della risorsa | `ref`, `inout`, `Slice(T)` dinamiche |

La capacità di duplicare è una **proprietà semantica del tipo**, non una
richiesta di un puntatore o un nuovo memory manager. Un numero intero che
codifica un file descriptor è copiabile come numero; un *wrapper che possiede*
il descriptor **non** lo è automaticamente. Copiarne soltanto il numero non
realizza una seconda risorsa indipendente.

La composizione propaga i vincoli: una struct, un enum con payload attivo, un
Array proprietario o l'ambiente di una closure che contengono risorse non
duplicabili non sono liberamente duplicabili. Il checker deve conoscere la
capacità dei componenti senza basarsi sul layout fisico.

Il modo in cui un tipo viene dichiarato non duplicabile, soprattutto per
wrapper della stdlib/FFI, è parte del successivo allineamento della Surface
Syntax e delle primitive sicure del compilatore. **Questa specifica non
introduce una nuova keyword pubblica.**

### 19.2 By-value, `ref`, `inout` e `consume`

- Un parametro ordinario `T` con valore duplicabile conserva le **value
  semantics** della v0.1: il chiamante mantiene un valore valido.
- Il compilatore può effettuare un **move fisico** su un ultimo uso provato,
  ma questo non cambia la semantica by-value e non invalida il binding per
  contratto.
- `ref` legge senza acquisire ownership; `inout` muta con accesso esclusivo.
- `consume` è un **trasferimento semantico esplicito**: il binding sorgente
  diventa inutilizzabile dopo l'operazione. È già definito per le catture
  `fn[consume value]`; la sua grafia per parametri/receiver va allineata alla
  Surface Syntax prima di essere dichiarata implementata.
- Il normale passaggio di un valore **non duplicabile** non può inferire un
  `consume` semantico dal solo ultimo uso. Deve usare accesso borrowed oppure
  un contratto esplicito di trasferimento.

Una copia o un trasferimento fisico sono strategie di compilazione; il
**consumo semantico** è un fatto osservabile dal type checker. Nessuna scelta
dell'optimizer può fare apparire una seconda ownership della stessa risorsa.

### 19.3 Distruzione e percorsi di controllo

La risorsa posseduta viene rilasciata una sola volta sul percorso semantico
che termina la sua ownership: fine validità, consumo da parte di un diverso
owner, esplicita chiusura o completamento/cancellazione del task. Un
trasferimento non provoca cleanup da parte del precedente owner.

Il checker e il runtime devono garantire:

- nessun *double release*, uso dopo release o rilascio mentre il runtime
  mantiene accessi ancora attivi;
- distruzione dell'owner residuo in uscita normale, `return`, propagazione di
  errore e cancellazione **completata**;
- gestione coerente delle varianti di enum e dei campi contenenti risorse;
- assenza di cicli proprietari forti secondo l'invariante NO-CYCLE.

**Cleanup automatico:** il cleanup non fallibile può essere integrato nella
distruzione. Un'operazione che deve comunicare un errore di chiusura all'utente
richiede invece un'API esplicita, ad esempio una `close()` con `Result`.
Un cleanup automatico non può nascondere o ignorare un errore richiesto dal
contratto pubblico; i dettagli di API della stdlib rimangono da progettare.

La distruzione che richiede capacità (es. I/O, processo o altre operazioni)
contribuisce al contratto di effetti applicabile (§20). Non si possono
nascondere operazioni vietate dentro un cleanup implicito.

**Garanzia delimitata:** il rilascio deterministico copre le esecuzioni e le
cancellazioni completate secondo il runtime; non promette cleanup applicativo
in seguito a kill non gestibile, crash del processo o terminazione brutale.

### 19.4 Cancellazione e risorse I/O pendenti

La richiesta di cancellazione **non** trasferisce immediatamente il diritto
di distruggere un buffer o un handle ancora usato da un'operazione del sistema
operativo. Il runtime deve attendere una condizione di completamento,
annullamento effettivo o presa in carico sicura, prima di liberare storage.

L'esito di un task che consuma una risorsa deve specificare chi è proprietario
della risorsa in ciascun percorso: risultato normale, errore e cancellazione.
Il semplice schema `Task((Reader, Result(...)))` della §17.8 è illustrativo;
non impone che una cancellazione restituisca materialmente il reader al
chiamante. Può invece distruggerlo in sicurezza secondo un contratto
esplicito, senza perdita o doppia liberazione.

## 20. v0.4 — Copie, capacità `Alloc` ed effects

### 20.1 Controllo degli effetti indipendente dai backend

`@deny(alloc)` vieta operazioni le cui implementazioni ammesse richiedano o
possano richiedere allocazioni dinamiche nel perimetro coperto dal contratto.
Il checker usa una classificazione **conservativa e stabile**: una funzione
non diventa valida soltanto perché un backend particolare riesce talvolta
ad eliminare un'allocazione con copy elision, escape analysis o inlining.

| Operazione | Requisito conservativo `Alloc` |
|---|---|
| Copia di scalare o aggregato fisso interamente inline | Nessuno, se il lowering non alloca |
| Copia indipendente di buffer dinamico | Potenziale `Alloc` |
| Acquisizione tramite `consume` senza duplicazione | Nessun `Alloc` per il solo trasferimento, salvo altri effetti dell'operazione |
| Costruzione di un nuovo buffer dinamico | Potenziale `Alloc` |
| Estrazione proprietaria di sottoporzione compatta | Potenziale `Alloc` |
| Creazione di ambiente closure che richiede storage dinamico | Potenziale `Alloc` |
| Uso di `Range` come descrittore di soli indici | Nessuno per il solo descrittore |

Queste classificazioni non promettono che tutte le operazioni su un tipo
siano allocation-free. `Array` può essere riutilizzato e una closure può essere
stack-allocated, ma questo **non** autorizza una garanzia di assenza di
allocazioni che dipenda da un optimizer opzionale o da un singolo backend.

### 20.2 Non nascondere allocazioni nel lowering

Per certificare `@deny(alloc)`, compilatore e backend devono preservare la
garanzia sul percorso eseguibile supportato, compresi gli effetti delle
operazioni generate dal lowering, dai helper runtime e dal cleanup. Se una
rappresentazione scelta dalla VM o dal backend impone una allocazione
dinamica incompatibile e non esiste un percorso conforme, la funzione non
può essere certificata `@deny(alloc)` su quel percorso.

Le normali strutture di controllo come un frame sullo stack **non** sono
per definizione allocazioni heap. Il confine preciso fra memoria di servizio
del runtime/VM e allocazioni attribuibili al programma dovrà essere verificato
con test di conformità: nessuna allocazione nascosta può essere esclusa
arbitrariamente per far risultare valida una funzione. La specifica non
asserisce che la VM attuale abbia già tale certificazione.

### 20.3 Interazione con gli altri effects

I contratti esistenti nella Surface Syntax restano quelli correnti:
`@deny(alloc, panic, io, process, spawn, nondeterminism)` e `@pure`.
`@pure` nega IO, Process, Spawn e NonDeterminism, **non** Alloc né Panic,
salvo ulteriori `@deny`. La distruzione e il cleanup partecipano ai
requisiti transitivi, senza eccezioni nascoste per FFI o risorse.

I controlli di bounds e i fallimenti controllati seguono la disciplina
`Panic` già documentata; `@deny(panic)` non può dipendere da una presunzione
non provata di indici validi.

**Non è autorizzata** una copia proprietaria implicita introdotta unicamente
per rendere valido un borrow escaping. L'utente deve cambiare il contratto
in modo esplicito quando desidera ownership persistente.

## 21. v0.4 — Aggregati borrowed, generics e interfacce

### 21.1 Struct temporanee con `Slice(T)` dinamica

Una struct locale può contenere una vista borrowed dinamica se l'intero
aggregato resta **non-escaping** e il checker conosce proprietario e loan
relativi al campo. Non si introduce alcun lifetime generico pubblico.

```tempra
struct PacketWindow {
    data: Slice(UInt8)
    payload: Range
}

let window = PacketWindow {
    data: buffer[0..16],
    payload: 2..12,
}
inspect_window(window)
// window.data conserva la dipendenza da buffer sino all'ultimo uso valido.
```

La forma di literal struct riprende la Surface Syntax; l'esempio non dichiara
che il checker attuale implementi già l'escape analysis dell'aggregato.

| Operazione | Regola v0.4 |
|---|---|
| Creare aggregato borrowed locale | Ammessa |
| Passarlo a callback/funzione non-escaping | Ammessa |
| Mutare il suo proprietario con prestito conflittuale vivo | Vietata |
| Conservarlo in oggetto persistente indipendente | Vietata |
| Restituire un aggregato che contiene borrow dinamico | Vietato nella v1, anche dove la singola Slice sarebbe ritornabile |
| Catturarlo in closure escaping | Vietato |
| Attraversare `.await()` con borrow ancora vivo | Vietato |

Un'eventuale copia del descrittore aggregato non estende il lifetime dei
borrow contenuti. L'invariante segue l'intera catena dei campi fino al loro
ultimo utilizzo e impedisce escape indiretti tramite ADT, tuple o callback.

### 21.2 Tipi specializzati e capacità derivate

La specializzazione dei generics utilizza i tipi concreti per derivare:

- duplicabilità e necessità di distruzione dei valori;
- presenza di borrow dinamici e provenienza dei risultati;
- modi di accesso (`Read`, `Exclusive`) dei parametri;
- ritorni borrowed con un unico owner identificabile;
- proprietà non-escaping dei callback;
- requisiti conservativi di effetti e cleanup.

```tempra
fn identity(type T, value: T) -> T {
    value
}
```

Per `T` duplicabile vale la normale semantica by-value, con possibili move
fisici all'ultimo utilizzo. Se `T` è `Slice(UInt8)` (o altro valore borrowed),
il risultato deve preservare la provenienza originaria, senza fingere che il
buffer sia stato copiato. Se `T` è una risorsa non duplicabile, il passaggio
ordinario by-value non autorizza un consumo implicito: occorre un contratto
di accesso o trasferimento esplicito compatibile, oppure rifiutare la
specializzazione. La stessa regola vale quando il vincolo è contenuto in
una struct, enum, array o closure.

### 21.3 Static duck interfaces

La conformità strutturale di un'interfaccia è compatibile con la presente
specifica solo se firme, modalità di accesso, provenienze, effetti e
capacità di duplicazione richieste risultano soddisfatti. Un'interfaccia
non trasforma un tipo non duplicabile in duplicabile e non produce una
reference persistente a partire da un borrow temporaneo.

Le interfacce di Tempra rimangono **static-only** secondo la Surface Syntax:
non vengono introdotti existential runtime o vtable per risolvere questi
contratti. I riepiloghi inferiti delle specializzazioni devono essere
modulari, deterministici, riproducibili e invalidare le cache dipendenti
quando cambia la semantica.

I dettagli di sintassi di eventuali vincoli di duplicabilità non sono fissati
qui: l'implementazione può rifiutare specializzazioni che richiederebbero
copie illegali, senza introdurre keyword o trait pubblici non approvati.

### 21.4 Borrow e istanziazione generica

Le funzioni generiche restano soggette alle stesse restrizioni degli altri
callable: niente ritorni borrowed multi-owner dinamici, niente slice borrowed
persistenti in aggregati indipendenti, niente borrow dinamici attraverso
sospensioni. La specializzazione non è una scappatoia dalle verifiche.

## 22. v0.4 — FFI e memoria esterna

### 22.1 Confine safe/foreign

Un puntatore, handle o indirizzo proveniente da FFI **non** diventa una
`ref`, una `inout` o una `Slice(T)` sicura Tempra per il solo fatto di essere
numericamente valido. Il wrapper deve dimostrare o garantire il lifetime, la
stabilità della memoria, l'aliasing, la mutabilità e il contratto di cleanup.

API safe devono esporre, quando possibile, **valori proprietari** oppure
**accessi temporanei controllati**. Sono vietati ritorni o storage di
borrow dinamici che non rispettano la provenienza prevista dal linguaggio.

Una funzione esterna con callback privo di un contratto non-escaping
affidabile viene considerata **escaping**: non riceve closure con catture
`ref`/`inout` dinamiche.

### 22.2 Operazioni raw e `@unsafe`

La memoria esterna con invarianti non verificabili staticamente richiede un
confine di implementazione auditabile. La Surface Syntax riserva `@unsafe`
per una specifica successiva; la v0.4 **non** dichiara oggi disponibile una
sintassi raw sicura né implementa il confine `@unsafe`.

Un wrapper safe può chiamare operazioni interne validate senza esporre le
relative assunzioni al chiamante. Tale wrapper deve continuare a rispettare
`@deny(...)`, `@pure`, proprietà della risorsa e regole di borrowing.

La forma definitiva dell'ABI FFI, della rappresentazione degli handle e
della tipizzazione di puntatori raw è una decisione implementativa separata.

### 22.3 I/O asincrono e storage stabile

Un kernel, driver o API esterna può mantenere un indirizzo di buffer fino a
un completamento asincrono. Il runtime deve **possedere** tale buffer per la
durata necessaria e garantire che il suo indirizzo fisico rimanga valido e
stabile finché l'operazione esterna può accedervi. L'obbligo può essere
realizzato con storage stabilizzato o pinning **interno al runtime**, senza
introdurre una nuova modalità pubblica di borrowing.

Il fatto che il runtime detenga un puntatore tecnico verso memoria che
possiede **non** consente al codice safe Tempra di mantenere un borrow
dinamico attraverso `.await()`; il divieto della §17.2 resta assoluto.

Errore, completamento, cancellazione e rilascio rispettano la §19.4:
nessuna liberazione anticipata mentre un sistema esterno possa ancora
accedere al buffer.

## 23. v0.4 — Contratti ownership-aware TIR e backend

### 23.1 Autorità semantica condivisa

`Source → HIR/typecheck → TIR → [verifier] → backend` mantiene un unico
modello di ownership tra Tempra Script e Tempra Native. Né VM né AOT possono introdurre
comportamenti semantici differenti per rendere valido codice rifiutato dal
checker.

**Contratti minimi verificabili**, indipendenti dai nomi degli opcode:

| Concetto | Invariante |
|---|---|
| `ValueCopy` | Semantica indipendente per duplicabili; vietata su risorse non duplicabili |
| `OwnershipTransfer` | Solo per consumo semantico autorizzato o move fisico equivalente e non osservabile |
| `BorrowRead` | Owner/provenienza validi, nessuna invalidazione fino all'ultimo uso |
| `BorrowExclusive` | Esclusività garantita rispetto agli accessi concorrenti |
| `BorrowEnd` | Compatibile con liveness e control flow |
| `Drop/Close` | Cleanup una volta sola e solo dopo fine accessi pendenti |
| `Suspend/Resume` | Nessun loan dinamico vivo oltre la sospensione |
| `ClosureEnvironment` | Catture valide, nessun escape illegale, nessun ciclo forte |
| `CallSummary` | Parametri, risultati, callback, effetti e provenienze preservati |

I nomi sono **categorie concettuali**, non opcode esistenti.

### 23.2 Verifier: controlli locali e metadata

Il verifier deve rifiutare TIR malformato che violi gli invarianti
normalizzati di ownership, definitezza, terminazione dei borrow, stati di
consumo/distruzione e sospensione. Può utilizzare i metadata emessi dal
type checker e vincoli semplici di CFG. Non è richiesto duplicare nel
verifier un solver completo del linguaggio sorgente.

I riepiloghi modulari estendono `BorrowSummary` concettualmente con:

```text
OwnershipSummary {
    parameter_accesses
    copyability_requirements
    return_provenance
    callback_escape
    resource_transfer_and_cleanup
    capability_requirements
}
```

Forma di serializzazione, ABI, layout e opcode concreti rimangono da definire.
Il cambiamento delle informazioni semantiche deve invalidare gli artefatti
cache non più affidabili, anche se il formato del bytecode non cambia.

### 23.3 Tempra Script, Tempra Native e `comptime`

- **Tempra Script (stack VM):** i valori proprietari possono usare ARC NO-CYCLE;
  le reference e le slice sicure restano protette dal checker. La VM non può
  usare ARC come fallback per accettare un borrow semanticamente invalido.
- **Tempra Native (AOT LLVM):** perseguire release dirette, copia/move/reuse e
  implementazioni RC-free; **zero GC/ARC è ancora un obiettivo da provare**,
  non una garanzia del backend oggi esistente.
- **`comptime`:** nessuna eccezione alle regole pubbliche di ownership, alle
  durate statiche o agli effects. L'utilizzo futuro della VM per l'esecuzione
  di comptime deve preservare gli stessi invarianti. L'audit della
  `COMPTIME_SPEC.md` rispetto alla v0.4 è un'attività successiva, non una
  verifica già compiuta.

## 24. v0.4 — Range, slicing ed estrazione proprietaria

### 24.1 Intervalli half-open

`Range` rappresenta semanticamente un intervallo **`[start, end)`**:
include `start`, esclude `end`. Rimane un valore autonomo senza puntatore al
buffer e senza lifetime borrowed. La rappresentazione fisica non è imposta.

Quando si applica il Range a un buffer, il compilatore/runtime verifica le
condizioni di validità degli indici per la collezione interessata:
`0 <= start <= end <= len(buffer)` dove applicabile. La costruzione del
Range, da sola, non mantiene vivo né certifica il buffer. Range fuori bounds
non autorizzano accessi non sicuri. I fallimenti e la loro capacità `Panic`
seguono i contratti di error handling stabiliti dalla Surface Syntax.

La scelta del tipo numerico concreto degli indici e l'API definitiva di
indicizzazione sono da allineare alla Surface Syntax, senza mutare il
significato half-open.

### 24.2 Separare descrizione, owner e compattezza

| Necessità | Contratto |
|---|---|
| Descrivere una porzione senza trattenerne i byte | `Range` autonomo |
| Conservare integralmente la sorgente e la porzione | `OwnedFrame` o owner + `Range` |
| Conservare soltanto il payload in modo indipendente | `OwnedPayloadFrame` o `Array(T)` proprietario |
| Elaborare temporaneamente senza acquisire ownership | `Slice(T)` non-escaping |

L'estrazione di una porzione in un valore proprietario compatto può
richiedere copia, allocazione o compattazione. Il trasferimento zero-copy è
possibile se rappresentazione, unicità e semantica dell'intero owner lo
consentono; **non** è garantito per sottosequenze arbitrarie.

Per buffer non duplicabili o contenenti elementi non duplicabili,
un'estrazione deve esplicitare che cosa venga trasferito e come restino
validi gli elementi e l'owner residuo. Nessun `consume` implicito di una
sottoparte è autorizzato dal solo fatto che il compilatore può muoverla.

### 24.3 Nessun bypass di `inout`

L'uso di un `Range` per ricreare una vista **non** autorizza una mutazione
in presenza di un prestito incompatibile, né permette di aggirare
l'esclusività di `inout Slice(T)`. La possibilità di conservare un Range è
indipendente dalla possibilità di accedere a un dato owner in quel momento.

## 25. v0.4 — Matrice di conformità e diagnostica minima

| Caso | Comportamento richiesto |
|---|---|
| By-value su tipo duplicabile ancora usato dal chiamante | Sorgente ancora valida; copia semanticamente indipendente |
| By-value su tipo duplicabile all'ultimo uso | Stessa semantica; possibile move fisico |
| Passaggio ordinario che copierebbe risorsa non duplicabile | Rifiutato |
| `consume` esplicito su risorsa posseduta | Trasferimento, sorgente invalidata |
| Doppia distruzione o accesso dopo release | Rifiutato/impedito |
| Cleanup fallibile nascosto senza contratto | Non autorizzato |
| `@deny(alloc)` con copia potenzialmente allocante | Rifiutato conservativamente |
| `@deny(io)` con cleanup che richiede IO | Non certificabile senza contratto conforme |
| Struct locale contenente `Slice(T)` dinamica non-escaping | Ammessa |
| Ritorno borrowed di aggregato contenente Slice dinamica | Rifiutato nella v1 |
| Specializzazione generica che richiede copia di risorsa unica | Rifiutata |
| Interface che aggira vincoli di accesso/copiabilità | Rifiutata |
| Callback FFI potenzialmente escaping con cattura dinamica | Rifiutata |
| Raw pointer esterno trattato come Slice safe senza prova | Rifiutato |
| I/O pendente con distruzione anticipata del buffer | Rifiutato/impedito |
| Range `[start,end)` fuori bounds all'applicazione | Errore controllato, mai accesso invalido |
| `Range` persistente senza buffer | Ammesso |
| TIR che usa owner dopo consume/drop | Rifiutato dal verifier |
| Borrow dinamico vivo durante `Suspend` | Rifiutato |
| Closure ricorsiva `fn[self]` senza strong self-cycle | Ammessa |
| Actor handle e lifetime policy | **DEFERRED POST NO-CYCLE** |

Diagnostiche e test devono identificare accesso sorgente, proprietario,
cattura, invalidazione, effetto o risorsa responsabile, con alternativa
idiomatica compatibile (borrow, transfer, Range, owner esplicito). I test
parity devono esercitare gli stessi programmi e contratti su ogni backend
supportato, separando le feature non ancora implementate dai mismatch.

## 26. Autorità della v0.4, confini e lavoro differito

### 26.1 Spec unica di memory semantics

**Questa v0.4 è il riferimento normativo unico per il modello di memoria di
Tempra.** Sostituisce la v0.3 e incorpora tutte le decisioni delle v0.1–v0.3.
La Surface Syntax rimane riferimento della grammatica finché non verrà
aggiornata, ma **non** può sovrascrivere le regole semantiche di memoria qui
approvate. In caso di divergenza, implementazione e documentazione devono
essere allineate alla v0.4, non viceversa.

Le liste storiche (§14.13, §18.2) hanno funzione di cronologia. Il
checker, la VM, i backend e il runtime devono adottare l'insieme dei
contratti qui consolidati, senza comportamenti ad hoc per categorie di
funzioni, closure, metodi o task.

### 26.2 Decisioni intenzionalmente rinviate

- **Actor handle e relativo lifetime: DEFERRED POST NO-CYCLE.** Nessuna
  posizione su strong reference, identità non proprietaria o policy mailbox.
- **Sintassi** definitiva per parametri e receiver proprietari (`consume`),
  dichiarazione di tipi non duplicabili e allineamento delle slice con
  `inout`: revisione complessiva della Surface Syntax in seguito.
- **`@unsafe` e ABI FFI raw:** definizione in uno step dedicato, quando
  esisterà una primitiva concreta da rendere auditabile.
- **Implementazione:** rappresentazione TIR, layout fisici, verifier concreto,
  caching e conformance suite sui backend.
- **Tempra Native RC-free:** obiettivo sperimentale da validare sul codice generato
  e sul runtime, non un'asserzione già provata.
- **Policy API:** cancellazione di task, modalità di restituzione/cleanup di
  risorse I/O, reentrancy actor e gestione dettagliata dei fallimenti.
- **Audit `comptime`:** da confrontare con questa specifica; futura
  esecuzione via VM non certifica automaticamente la compatibilità.

Queste attività non riaprono i principi fondamentali della v0.4; possono
richiedere specifiche di API o implementazione compatibili.

### 26.3 Allineamento documentale successivo

La successiva revisione del repository `tempra/docs/` dovrà eliminare o
marcare come storiche le regole superate, specialmente:

- la descrizione delle slice direttamente mutabili presente nella Surface
  Syntax precedente, incompatibile con `Slice(T)` read-only + `inout`;
- riferimenti a lifetime, proprietà o escape delle closure non allineati;
- passaggio di risorse non duplicabili senza consumo esplicito;
- dichiarazioni che confondono obiettivi sperimentali RC-free con capacità
  implementate;
- implementazioni o backend dismessi e altre differenze di roadmap.

La **pulizia effettiva della documentazione viene dopo il completamento
della VM**, come pianificato. In questa v0.4 si registrano solo gli obblighi
di allineamento; non si dichiara modificato il repository.

## 27. Principio finale della Memory Specification v0.4

**Ownership per conservare. Borrowing per accedere. Range per descrivere.
ADT per rappresentare alternative.**

Il codice usa **un'unica semantica** attraverso funzioni, closure, metodi,
concorrenza, `comptime`, VM e AOT. `ref` e `inout` forniscono accessi; `consume`
trasferisce semanticamente la proprietà; il by-value duplicabile conserva
l'indipendenza semantica. Copia, move fisico, COW, reuse e distruzione diretta
sono scelte di implementazione subordinate agli stessi contratti.

L'ambito implementativo resta distinto dalle regole approvate: **Tempra Script mira
ad ARC NO-CYCLE; Tempra Native persegue l'obiettivo GC/RC-free ancora da
validare.** Nessuna ottimizzazione o eccezione runtime può trasformare un
programma semanticamente invalido in un programma valido.
