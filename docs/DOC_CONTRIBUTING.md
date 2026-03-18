# Rust Documentation Standard (AI-enforced)

This document defines the mandatory documentation format for all Rust code in this project. Every AI agent contributing to or analyzing this codebase **MUST** follow this standard.

---

## Core Principles

- **One-line summary**: A concise description of the item’s purpose.
- **Detailed description**: Explain special behavior, return semantics, and parameter constraints not obvious from the signature.
- **Example-driven**: The `# Example` section is the primary way to show arguments and return values.
- **Explicit errors/panics**: Always document `# Errors` (for `Result`-returning functions) and `# Panics` (if applicable).
- **No separate `# Arguments` or `# Returns` sections**: Their information is merged into the detailed description and the example.

---

## Specification

### 1. Summary Line

A single sentence (present tense, third person) describing what the item does.

```rust
/// Creates a new user in the database.
```

### 2. Detailed Description

A short paragraph or bullet points covering:

- What is returned on success (if not obvious).
- Special behavior (e.g., “returns `None` if key not found”).
- Constraints on parameters (length, format, etc.).

```rust
/// Creates a new user in the database.
///
/// The username must be 3–20 alphanumeric characters; email must be RFC5322‑compliant.
/// Returns the newly assigned user ID.
```

### 3. `# Example` Section

One or more code blocks demonstrating typical usage.  

- Must show how to call the item and handle its result.  
- Use `#` to hide boilerplate when needed.  
- Omit for private/non‑public items.

```rust
/// # Example
/// ```
/// let id = create_user("alice", "alice@example.com").unwrap();
/// println!("{}", id);
/// ```
```

### 4. `# Errors` Section (required for `Result`-returning functions)

Describe each error variant and the conditions that cause it.

```rust
/// # Errors
/// Returns `AuthError::InvalidInput` if username/email format is invalid.
/// Returns `AuthError::UserExists` if the username is already taken.
```

### 5. `# Panics` Section (if the function can panic)

Describe circumstances that lead to a panic. Omit if never panics.

```rust
/// # Panics
/// Panics if the database connection pool fails to initialize.
```

### 6. `# Safety` Section (required for `unsafe` functions)

Explain the preconditions the caller must uphold to avoid undefined behavior.

```rust
/// # Safety
/// `ptr` must be non‑null, aligned, and point to a valid `T`.
```

---

## Complete Example

```rust
/// Registers a new user and returns their unique ID.
///
/// The username must be 3–20 alphanumeric characters; email must be a valid RFC5322 address.
/// The password is hashed using bcrypt with the default cost.
///
/// # Example
/// ```
/// let id = register("bob", "bob@example.com", "s3cret").await?;
/// println!("New user ID: {}", id);
/// # Ok::<_, AuthError>(())
/// ```
///
/// # Errors
/// - `AuthError::InvalidInput` – username/email format invalid.
/// - `AuthError::UserExists` – username already taken.
/// - `AuthError::Database` – database operation failed.
///
/// # Panics
/// Panics if the bcrypt cost factor cannot be read from the environment.
```

---

**All AI agents interacting with this codebase MUST adhere to this documentation standard.**
