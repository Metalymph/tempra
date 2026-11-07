# R.PROOF — Result obligations (draft)

**Not proved.** This document records desired properties, not typing
judgments or a formal transition system yet.

**Safety claim under investigation:** on every terminating/relevant
execution of an accepted program in the supported core, a produced
`Result` cannot be silently discarded without handling or propagation.
Recognizing `Err` is **not** equivalent to successful application-level
recovery. Aggregates, closures, exceptional termination, externally
managed Results and unbounded loops need explicit semantic treatment.

First define concrete traces for: produce, alias, copy, overwrite,
match, `?`, return, function call, nested branch and repeated loop site.
Then define an abstraction relation between traces and the analyzer's
`ResultOriginId`, `JoinedOrigin`, `NotProduced` and sticky pending evidence.
For the declared subset, prove soundness of transfer and join, loop
fixed-point convergence and the acceptance-implies-no-silent-drop theorem.
Separately connect the model to the Rust implementation. Existing R2/R3
tests are regression evidence, not formal evidence.
