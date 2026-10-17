# JSF Air Vehicle coding baseline

This is a baseline for code in this repository. It is not a certification claim.

Source: *Joint Strike Fighter Air Vehicle C++ Coding Standards for the System Development and Demonstration Program*, Document Number 2RDU00001 Rev C, December 2005.

> The intent of this document is to provide direction and guidance to C++ programmers that will enable them to employ good programming style and proven programming practices leading to safe, reliable, testable, and maintainable code.

This repository is Rust, not C++. C++-specific rules do not apply as written. The safety intent does. A C++ rule is mapped below only where the same defect exists in Rust.

## Rule types

From section 4.2.1:

- Should rules are advisory.
- Will rules are mandatory and do not require verification.
- Shall rules are mandatory and require verification.

AV Rule 5: breaking a will or a shall rule requires approval. A deviation in this repository is a row in `docs/waivers.md`, not a silent allow.

## Mapped shall rules

| Rule | Text | How it holds here |
| --- | --- | --- |
| AV Rule 1 | A function will contain no more than 200 logical source lines. | A function over 200 lines fails review. |
| AV Rule 2 | There shall not be any self-modifying code. | No code writes its own text. A spec change is a bundle. |
| AV Rule 3 | All functions shall have a cyclomatic complexity of 20 or less. | A match with many arms is the documented exception. |
| AV Rule 119 | Functions shall not call themselves, directly or indirectly. | No recursion on the tick path. |
| AV Rule 186 | There shall be no unreachable code. | `cargo clippy -- -D warnings` fails on dead code. |
| AV Rule 203 | Evaluation of expressions shall not lead to overflow or underflow. | Wrapping arithmetic is named where it is used. |
| AV Rule 206 | Allocation from the free store shall not occur after initialization. | The switch does not allocate on the tick path. |
| AV Rule 207 | Unencapsulated global data will be avoided. | The switch has no global mutable state. |
| AV Rule 208 | C++ exceptions shall not be used. | The tick path returns `Result`. It does not panic. |
| AV Rule 217 | Compile-time and link-time errors should be preferred over run-time errors. | Types and the `Command` enum catch what a comment would not. |
| AV Rule 218 | Compiler warning levels will be set in compliance with project policies. | CI runs `cargo clippy --workspace --all-targets -- -D warnings`. |

## Not mapped

Virtual destructors, templates, namespaces, `explicit` constructors, and C-style casts are C++. They are not rewritten as Rust rules. The Rust style guide and `rustfmt` cover style.

## Moving forward

New code meets this table. A pull request that breaks a mapped shall rule names the rule and adds a waiver row, or it does not land.
