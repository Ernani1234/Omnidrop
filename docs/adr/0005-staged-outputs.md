# 0005 — Stage, validate, then publish every result

## Context

Conversions can fail midway, be cancelled, or produce damaged files. Users must never lose
originals or find half-written results.

## Decision

Every executor writes into a temporary file or folder created inside the destination folder. The
validator re-reads it against an explicit expectation (format, dimensions, pages, entries, streams).
Only then does the OutputManager publish it with an atomic rename, following the collision policy
with no-clobber semantics. Inputs of the job and earlier results of the same job are never replaced.
Temporary items are removed automatically on failure, cancellation or panic.

## Consequences

- A result either appears complete and verified, or not at all.
- Validation costs a re-read of each result, which is cheap compared to encoding.
