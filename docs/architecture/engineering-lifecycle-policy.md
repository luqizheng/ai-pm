# Engineering Lifecycle Policy

> Status: Active
> Scope: Entire AI-PM repository

## 1. Project Stage

AI-PM is pre-production:

- No historical production data.
- No customers.
- No formally released version.
- No production compatibility obligations.

## 2. Greenfield Design Principle

Apply a pre-production greenfield policy:

- Refactor schemas, domain models, APIs, and internal contracts directly when needed.
- Remove obsolete tables, fields, indexes, and compatibility code after checking current references and dependencies.
- Rework migration history or establish a clean schema baseline when justified.
- Do not add compatibility layers, data migration, dual writes, fallback switches, or transitional code solely for never-released behavior.
- Prefer the long-term correct, clear, maintainable design over preserving accidental legacy implementation.

## 3. Quality and Safety Requirements

Greenfield does not relax engineering quality. Preserve:

- Database transaction integrity and concurrency safety.
- Unique execution ownership and safe process/workspace lifecycle.
- Credential and permission security.
- Auditable critical state transitions.
- Automated tests and CI validation.
- Complete initialization from an empty database.
- Repeatable deployment and recoverable failure handling.

## 4. Change Requirements

Before deleting or replacing an existing table, field, or behavior, inspect current code references and runtime dependencies.

Schema changes must be validated against a fresh, empty database initialization and relevant CI tests.

This policy does not itself authorize implementation or destructive operations; each task must stay within its explicit scope and approval.

## 5. Default for All Work

All architecture, database, API, refactoring, and implementation work in this repository must follow this policy by default.

When an existing implementation conflicts with the intended architecture, propose direct refactoring rather than automatically adding backward-compatibility machinery.
