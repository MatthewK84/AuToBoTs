# Review

DoWI 8430.01 section 3.6: developers remain accountable for code generated or modified by AI. That code gets the same review and security testing as handwritten code.

A pull request states whether the change was generated or handwritten, the human reviewer, and the tests run. The reviewer is not the agent that wrote the change. An agent commit is not a review.

The coding baseline is `docs/jsf-av.md`, JSF Air Vehicle C++ Coding Standards, 2RDU00001 Rev C. A mapped shall rule that is broken needs a waiver row.

## AI-generated code

DoWI 8430.01 section 3.6: developers remain accountable for code generated or modified by AI. It gets the same review and security testing as handwritten code.

A pull request carries:

```
Generated or modified by AI: yes/no
Reviewer: <name>
Tests run: <list>
```

A review cannot be the same agent that wrote the change. No workflow treats an AI commit as reviewed by itself.

This is the acceptance record for M141.5.
