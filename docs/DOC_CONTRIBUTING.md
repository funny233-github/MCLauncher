# Rust Documentation Standard (AI-enforced)

This document defines the mandatory documentation format for all Rust code in this project. Every AI agent contributing to or analyzing this codebase **MUST** follow this standard.

---

## Core Principles

- **One-line summary**: A concise description of the item’s purpose.
- **Detailed description**: Explain special behavior, return semantics, and parameter constraints not obvious from the signature.
- **Example-driven**: The `# Example` section is the primary way to show arguments and return values.
- **Explicit errors/panics**: Always document `# Errors` (for `Result`-returning functions) and `# Panics` (if applicable).
- **No separate `# Arguments` or `# Returns` sections**: Their information is merged into the detailed description and the example.
- **Struct fields**: Only a brief description of each field’s meaning; do not repeat the type or restate the field name’s obvious purpose.

---

## Specification

### 1. Function Documentation

#### Summary Line

A single sentence (present tense, third person) describing what the function does.

```rust
/// Creates a new user in the database.
```

#### Detailed Description

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

#### `# Example` Section

One or more code blocks demonstrating typical usage.  

- Must show how to call the function and handle its result.  
- Use `#` to hide boilerplate when needed.  
- Omit for private/non‑public items.

```rust
/// # Example
/// ```
/// let id = create_user("alice", "alice@example.com").unwrap();
/// println!("{}", id);
/// ```
```

#### `# Errors` Section (required for `Result`-returning functions)

Describe each error variant and the conditions that cause it.

```rust
/// # Errors
/// Returns `AuthError::InvalidInput` if username/email format is invalid.
/// Returns `AuthError::UserExists` if the username is already taken.
```

#### `# Panics` Section (if the function can panic)

Describe circumstances that lead to a panic. Omit if never panics.

```rust
/// # Panics
/// Panics if the database connection pool fails to initialize.
```

#### `# Safety` Section (required for `unsafe` functions)

Explain the preconditions the caller must uphold to avoid undefined behavior.

```rust
/// # Safety
/// `ptr` must be non‑null, aligned, and point to a valid `T`.
```

---

### 2. Struct Documentation

#### Struct Summary

A one‑line description of the struct’s purpose.

#### Field Documentation

Each field gets a **single line** (or very short phrase) describing its meaning. Do **not** mention the type (it is already in the code) and do **not** restate the field name if it is self‑explanatory. Focus on any constraints, units, or special semantics.

```rust
/// A user account in the system.
struct User {
    /// Unique identifier, assigned by the database.
    id: Uuid,
    /// Login name, 3–20 alphanumeric characters.
    username: String,
    /// Email address, must be unique.
    email: String,
    /// Timestamp of account creation (UTC).
    created_at: DateTime<Utc>,
}
```

- If a field’s purpose is completely obvious from its name (e.g., `age: u32`), the description can be omitted.
- For fields with complex constraints or relationships, a brief note is sufficient.

#### Example (Optional)

If the struct is non‑trivial or has invariants, an `# Example` section may be added to show construction or usage.

```rust
/// # Example
/// ```
/// let user = User {
///     id: Uuid::new_v4(),
///     username: "alice".to_string(),
///     email: "alice@example.com".to_string(),
///     created_at: Utc::now(),
/// };
/// ```
```

---

### 3. Enum Documentation

Similar to structs: a summary line, then each variant described briefly. If variants carry data, document the data fields concisely.

```rust
/// Errors that can occur in the authentication module.
enum AuthError {
    /// The provided username or email format is invalid.
    InvalidInput,
    /// The username is already taken.
    UserExists,
    /// A database operation failed. Contains the underlying error.
    Database(#[doc = "The original database error"] sqlx::Error),
}
```

---

## Complete Example (Function + Struct)

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
pub async fn register(username: &str, email: &str, password: &str) -> Result<Uuid, AuthError> { ... }

/// A user account in the system.
pub struct User {
    /// Unique identifier, assigned by the database.
    id: Uuid,
    /// Login name, 3–20 alphanumeric characters.
    username: String,
    /// Email address, must be unique.
    email: String,
    /// Timestamp of account creation (UTC).
    created_at: DateTime<Utc>,
}
```

---

**All AI agents interacting with this codebase MUST adhere to this documentation standard.**
