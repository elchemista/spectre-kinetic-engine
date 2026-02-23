Rust Best Practices

Apply these guidelines when writing or reviewing Rust code. Based on Apollo GraphQL's Rust Best Practices Handbook.
Best Practices Reference

Before reviewing, familiarize yourself with Apollo's Rust best practices. Read ALL relevant chapters in the same turn in parallel. Reference these files when providing feedback:

    Chapter 1 - Coding Styles and Idioms: Borrowing vs cloning, Copy trait, Option/Result handling, iterators, comments
    Chapter 2 - Clippy and Linting: Clippy configuration, important lints, workspace lint setup
    Chapter 3 - Performance Mindset: Profiling, avoiding redundant clones, stack vs heap, zero-cost abstractions
    Chapter 4 - Error Handling: Result vs panic, thiserror vs anyhow, error hierarchies
    Chapter 5 - Automated Testing: Test naming, one assertion per test, snapshot testing
    Chapter 6 - Generics and Dispatch: Static vs dynamic dispatch, trait objects
    Chapter 7 - Type State Pattern: Compile-time state safety, when to use it
    Chapter 8 - Comments vs Documentation: When to comment, doc comments, rustdoc
    Chapter 9 - Understanding Pointers: Thread safety, Send/Sync, pointer types

Quick Reference
Borrowing & Ownership

    Prefer &T over .clone() unless ownership transfer is required
    Use &str over String, &[T] over Vec<T> in function parameters
    Small Copy types (≤24 bytes) can be passed by value
    Use Cow<'_, T> when ownership is ambiguous

Error Handling

    Return Result<T, E> for fallible operations; avoid panic! in production
    Never use unwrap()/expect() outside tests
    Use thiserror for library errors, anyhow for binaries only
    Prefer ? operator over match chains for error propagation

Performance

    Always benchmark with --release flag
    Run cargo clippy -- -D clippy::perf for performance hints
    Avoid cloning in loops; use .iter() instead of .into_iter() for Copy types
    Prefer iterators over manual loops; avoid intermediate .collect() calls

Linting

Run regularly: cargo clippy --all-targets --all-features --locked -- -D warnings

Key lints to watch:

    redundant_clone - unnecessary cloning
    large_enum_variant - oversized variants (consider boxing)
    needless_collect - premature collection

Use #[expect(clippy::lint)] over #[allow(...)] with justification comment.
Testing

    Name tests descriptively: process_should_return_error_when_input_empty()
    One assertion per test when possible
    Use doc tests (///) for public API examples
    Consider cargo insta for snapshot testing generated output

Generics & Dispatch

    Prefer generics (static dispatch) for performance-critical code
    Use dyn Trait only when heterogeneous collections are needed
    Box at API boundaries, not internally

Type State Pattern

Encode valid states in the type system to catch invalid operations at compile time:

struct Connection<State> { /* ... */ _state: PhantomData<State> }
struct Disconnected;
struct Connected;

impl Connection<Connected> {
    fn send(&self, data: &[u8]) { /* only connected can send */ }
}

Documentation

    // comments explain why (safety, workarounds, design rationale)
    /// doc comments explain what and how for public APIs
    Every TODO needs a linked issue: // TODO(#42): ...
    Enable #![deny(missing_docs)] for libraries

Anti-Patterns

    Layer 2: Design Choices

Core Question

Is this pattern hiding a design problem?

When reviewing code:

    Is this solving the symptom or the cause?
    Is there a more idiomatic approach?
    Does this fight or flow with Rust?

Anti-Pattern → Better Pattern
Anti-Pattern 	Why Bad 	Better
.clone() everywhere 	Hides ownership issues 	Proper references or ownership
.unwrap() in production 	Runtime panics 	?, expect, or handling
Rc when single owner 	Unnecessary overhead 	Simple ownership
unsafe for convenience 	UB risk 	Find safe pattern
OOP via Deref 	Misleading API 	Composition, traits
Giant match arms 	Unmaintainable 	Extract to methods
String everywhere 	Allocation waste 	&str, Cow<str>
Ignoring #[must_use] 	Lost errors 	Handle or let _ =
Thinking Prompt

When seeing suspicious code:

    Is this symptom or cause?
        Clone to avoid borrow? → Ownership design issue
        Unwrap "because it won't fail"? → Unhandled case

    What would idiomatic code look like?
        References instead of clones
        Iterators instead of index loops
        Pattern matching instead of flags

    Does this fight Rust?
        Fighting borrow checker → restructure
        Excessive unsafe → find safe pattern

Trace Up ↑

To design understanding:

"Why does my code have so many clones?"
    ↑ Ask: Is the ownership model correct?
    ↑ Check: m09-domain (data flow design)
    ↑ Check: m01-ownership (reference patterns)

Anti-Pattern 	Trace To 	Question
Clone everywhere 	m01-ownership 	Who should own this data?
Unwrap everywhere 	m06-error-handling 	What's the error strategy?
Rc everywhere 	m09-domain 	Is ownership clear?
Fighting lifetimes 	m09-domain 	Should data structure change?
Trace Down ↓

To implementation (Layer 1):

"Replace clone with proper ownership"
    ↓ m01-ownership: Reference patterns
    ↓ m02-resource: Smart pointer if needed

"Replace unwrap with proper handling"
    ↓ m06-error-handling: ? operator
    ↓ m06-error-handling: expect with message

Top 5 Beginner Mistakes
Rank 	Mistake 	Fix
1 	Clone to escape borrow checker 	Use references
2 	Unwrap in production 	Propagate with ?
3 	String for everything 	Use &str
4 	Index loops 	Use iterators
5 	Fighting lifetimes 	Restructure to own data
Code Smell → Refactoring
Smell 	Indicates 	Refactoring
Many .clone() 	Ownership unclear 	Clarify data flow
Many .unwrap() 	Error handling missing 	Add proper handling
Many pub fields 	Encapsulation broken 	Private + accessors
Deep nesting 	Complex logic 	Extract methods
Long functions 	Multiple responsibilities 	Split
Giant enums 	Missing abstraction 	Trait + types
Common Error Patterns
Error 	Anti-Pattern Cause 	Fix
E0382 use after move 	Cloning vs ownership 	Proper references
Panic in production 	Unwrap everywhere 	?, matching
Slow performance 	String for all text 	&str, Cow
Borrow checker fights 	Wrong structure 	Restructure
Memory bloat 	Rc/Arc everywhere 	Simple ownership
Deprecated → Better
Deprecated 	Better
Index-based loops 	.iter(), .enumerate()
collect::<Vec<_>>() then iterate 	Chain iterators
Manual unsafe cell 	Cell, RefCell
mem::transmute for casts 	as or TryFrom
Custom linked list 	Vec, VecDeque
lazy_static! 	std::sync::OnceLock
Quick Review Checklist

    No .clone() without justification
    No .unwrap() in library code
    No pub fields with invariants
    No index loops when iterator works
    No String where &str suffices
    No ignored #[must_use] warnings
    No unsafe without SAFETY comment
    No giant functions (>50 lines)

