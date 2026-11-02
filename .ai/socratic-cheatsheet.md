# Socratic cheat sheet — directive -> question
Why: imperative fixes ("change line 42 to X") make the model comply, apologize, and bloat/regress ("apology spiral"). Questions about behavior make it simulate the scenario and find the cause. (Examples below are mine, in the spirit of the articles.)

| Instead of (directive) | Ask (question) |
|---|---|
| "Add a nil check on line 42." | "What happens at line 42 if the cache returns no entry while the request is cancelled?" |
| "Wrap this in a mutex." | "Which goroutines can touch `state` concurrently, and what orders their accesses?" |
| "Don't change other files." | "What else breaks if we change this signature? List every caller you verified." |
| "Add a retry loop." | "If this call fails twice, what state is the system in, and is repeating it safe?" |
| "Use `?` instead of `unwrap()`." | "What does this function do when the input is malformed? Where does the error end up?" |
| "Add more tests." | "Which test fails if I delete this branch? If none, what behavior is unguarded?" |
| "Fix the lint warning." | "What risk was this warning pointing at, and does your fix remove the risk or just the warning?" |

## Rules of thumb
- Ask about state, timing, and lifecycle; the agent traces and implements.
- Pre-mortem on scope: "What is the blast radius?" instead of "don't touch X".
- "OK" is a valid answer when the agent defends its design with evidence.
- Polluted session (apologies, flip-flopping)? Extract the invariant, discard the context, restart the turn with a fresh `!Request`.
