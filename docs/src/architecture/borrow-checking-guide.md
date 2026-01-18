# Borrow Checking Guide for Architecture Redesign

This document addresses the most challenging borrow checking scenarios in the redesign, with practical solutions and explanations.

## Table of Contents

1. [Common Patterns](#common-patterns)
2. [Tricky Scenarios](#tricky-scenarios)
3. [Lifetime Elision Rules](#lifetime-elision-rules)
4. [Debugging Borrow Errors](#debugging-borrow-errors)

---

## Common Patterns

### Pattern 1: Arc for Shared Ownership

**Problem**: Multiple components need access to GPU context.

```rust
// ❌ BAD: Lifetime pollution
struct Renderer<'ctx> {
    context: &'ctx GraphicsContext,
}

struct UiSystem<'ctx> {
    renderer: Renderer<'ctx>,
}

// Every struct up the chain needs 'ctx lifetime!
```

**Solution**: Use Arc

```rust
// ✅ GOOD: No lifetimes
struct Renderer {
    context: Arc<GraphicsContext>,
}

struct UiSystem {
    renderer: Renderer,
}

// Clone is cheap (atomic ref count increment)
let ctx = GraphicsContext::new().await?;
let renderer = Renderer::new(ctx.clone());
let ui = UiSystem::new(ctx.clone());
```

**Why it works**:
- `Arc<T>` has no lifetime parameter
- Cloning increments ref count (very cheap)
- Last owner dropping decrements to zero, frees T
- All owners have equal status (no "primary" owner)

---

### Pattern 2: Interior Mutability for &self Methods

**Problem**: Trait method signature requires `&self`, but we need to mutate.

```rust
// Trait signature we must implement
trait RenderContext {
    fn create_buffer(&self, desc: &BufferDescriptor) -> Buffer;
    //              ^^^ Shared reference, but we want to track calls!
}
```

**Solution**: Interior mutability with Mutex

```rust
use parking_lot::Mutex;

struct MockRenderContext {
    // Mutex provides interior mutability
    calls: Mutex<Vec<RenderCall>>,
}

impl RenderContext for MockRenderContext {
    fn create_buffer(&self, desc: &BufferDescriptor) -> Buffer {
        // Lock mutex, mutate, unlock automatically
        self.calls.lock().push(RenderCall::CreateBuffer);

        // Return mock buffer
    }
}
```

**Why Mutex not RefCell?**
- `RefCell` is `!Sync` (not thread-safe)
- `Mutex` is `Send + Sync` (required for trait bounds)
- `parking_lot::Mutex` has better performance than `std::sync::Mutex`

**Borrow rules**:
```rust
// Each lock() creates a new scope
self.calls.lock().push(...);  // Lock, mutate, unlock

// Can't hold lock across await:
let guard = self.calls.lock();
// do_async_work().await;  // ❌ ERROR: guard held across await
drop(guard);
do_async_work().await;     // ✅ OK: guard dropped
```

---

### Pattern 3: Trait Objects with Box

**Problem**: Need to store different widget types in same container.

```rust
// ❌ Can't do this: types have different sizes
struct Container {
    children: Vec<Button | Text | Rect>,  // Not valid Rust
}
```

**Solution**: Trait objects with Box

```rust
// ✅ GOOD: Use Box<dyn Trait>
struct Container {
    children: Vec<Box<dyn Widget>>,
}

impl Container {
    fn add_child(&mut self, child: Box<dyn Widget>) {
        self.children.push(child);
    }

    fn iter_children(&self) -> impl Iterator<Item = &dyn Widget> {
        self.children.iter().map(|b| &**b)
        //                          ^^^ First * derefs Box, second * derefs dyn
    }

    fn iter_children_mut(&mut self) -> impl Iterator<Item = &mut dyn Widget> {
        self.children.iter_mut().map(|b| &mut **b)
    }
}
```

**Why Box?**
- `Box<dyn Trait>` is a fat pointer (pointer + vtable)
- All fat pointers are same size (can store in Vec)
- Box provides ownership of the trait object
- Deref coercion: `Box<T>` auto-derefs to `&T`

**Double deref explained**:
```rust
let boxed: &Box<dyn Widget> = &children[0];
// Type: &Box<dyn Widget>

let first_deref: &dyn Widget = &*boxed;
// Type: &dyn Widget
// Deref Box to get the trait object reference

let second_deref: &Widget = &**boxed;  // If Widget: Sized
// But dyn Widget is !Sized, so we stop at &dyn Widget
```

---

### Pattern 4: Generational Handles (Copy IDs)

**Problem**: Want type-safe references that are Copy.

```rust
// ❌ References aren't Copy
struct Handle<'a, T> {
    widget: &'a T,  // Not Copy (borrows T)
}

// ❌ Can't have multiple mutable references
let h1 = storage.get_mut(id1);  // &mut Widget
let h2 = storage.get_mut(id2);  // ERROR: storage already borrowed
```

**Solution**: Handles are just IDs (Copy)

```rust
// ✅ GOOD: Handle is just an ID
#[derive(Copy, Clone)]
struct WidgetHandle<T> {
    id: WidgetId,
    generation: u32,
    _phantom: PhantomData<*const T>,
}

// Usage:
let h1 = storage.add(button);  // Handle is Copy
let h2 = h1;  // h1 still valid!

// Get actual references separately:
let w1 = storage.get_mut(h1)?;
// let w2 = storage.get_mut(h2)?;  // ERROR: storage borrowed
drop(w1);
let w2 = storage.get_mut(h2)?;  // OK now
```

**Why generations?**
```rust
let handle = storage.add(widget);  // generation = 0

storage.remove(handle);  // Increments generation to 1

// Try to use old handle
storage.get(handle);  // generation mismatch, returns None

// Prevents use-after-free at runtime
```

**Why PhantomData<*const T>?**
```rust
// Without phantom data:
struct Handle {
    id: u64,
    // Compiler doesn't know about T
}

// With PhantomData<T>:
struct Handle1<T> {
    id: u64,
    _p: PhantomData<T>,  // Implies ownership of T
}
// Problem: Not Copy if T: !Copy

// With PhantomData<*const T>:
struct Handle2<T> {
    id: u64,
    _p: PhantomData<*const T>,  // Implies borrowed pointer
}
// Benefits: Always Copy, correct variance
```

---

## Tricky Scenarios

### Scenario 1: Mutable Access to Multiple Children

**Problem**: Want to update two children simultaneously.

```rust
fn swap_texts(parent: &mut dyn ParentWidget) {
    let children = parent.children_mut();

    // ❌ Can't get two mutable references to same Vec
    let text1 = children[0].as_text_widget_mut()?;
    let text2 = children[1].as_text_widget_mut()?;
    // ERROR: children borrowed twice mutably
}
```

**Solution 1**: Use `split_at_mut()`

```rust
fn swap_texts(parent: &mut dyn ParentWidget) {
    let children = parent.children_mut();

    // Split slice into two non-overlapping parts
    let (left, right) = children.split_at_mut(1);

    let text1 = left[0].as_text_widget_mut()?;
    let text2 = right[0].as_text_widget_mut()?;

    // text1 and text2 can coexist (different slices)
    std::mem::swap(&mut text1.text(), &mut text2.text());
}
```

**Solution 2**: Use indices

```rust
fn swap_texts(parent: &mut dyn ParentWidget) {
    let children = parent.children_mut();

    // Borrow separately
    {
        let text1 = children[0].as_text_widget_mut()?;
        let temp = text1.text().to_string();
        text1.set_text(&temp);
    }  // text1 borrow ends

    {
        let text2 = children[1].as_text_widget_mut()?;
        text2.set_text(&temp);
    }
}
```

**Solution 3**: Swap entire widgets

```rust
fn swap_widgets(parent: &mut dyn ParentWidget) {
    let children = parent.children_mut();
    children.swap(0, 1);  // No borrow issues
}
```

---

### Scenario 2: Self-Referential Structures

**Problem**: Widget needs to store shaped text that references its own text string.

```rust
// ❌ IMPOSSIBLE: Can't self-reference
struct TextWidget {
    text: String,
    shaped: ShapedText<'???>,  // Can't reference self.text
}

// Rust doesn't allow self-referential structs
```

**Solution 1**: Separate storage

```rust
// Store shaped text in external cache
struct TextWidget {
    text: String,
    shaped_id: Option<usize>,  // ID into cache
}

struct TextCache {
    shaped: HashMap<usize, ShapedText>,
}
```

**Solution 2**: Owned data

```rust
// ShapedText owns its data
struct TextWidget {
    text: String,
    shaped: Option<ShapedText>,  // Owns all glyph data
}

struct ShapedText {
    glyphs: Vec<ShapedGlyph>,  // Owns glyph data
    bounds: (f32, f32),
}

// When text changes, recreate shaped:
impl TextWidget {
    fn set_text(&mut self, text: String) {
        self.text = text;
        self.shaped = None;  // Invalidate cache
    }

    fn get_shaped(&mut self, font_renderer: &FontRenderer) -> &ShapedText {
        if self.shaped.is_none() {
            self.shaped = Some(font_renderer.shape(&self.text));
        }
        self.shaped.as_ref().unwrap()
    }
}
```

**Solution 3**: Arc for sharing

```rust
// Share shaped data across widgets
struct TextWidget {
    text: String,
    shaped: Option<Arc<ShapedText>>,
}

// Multiple widgets can share same shaped text (cheap clone)
```

---

### Scenario 3: Trait Objects with Lifetimes

**Problem**: Trait method returns references with lifetimes.

```rust
trait Widget {
    // Returns reference with same lifetime as self
    fn children(&self) -> &[Box<dyn Widget>];
}

// How to store this in a struct?
struct Container {
    current_children: &[Box<dyn Widget>],  // What lifetime?
}
```

**Solution 1**: Don't store borrowed references

```rust
// ✅ Don't store the reference
impl Container {
    fn process(&self) {
        let children = self.children();  // Borrow starts
        for child in children {
            child.update();
        }
        // Borrow ends when children goes out of scope
    }
}
```

**Solution 2**: Store owned data

```rust
// ✅ Store owned data
struct Container {
    children: Vec<Box<dyn Widget>>,  // Owns children
}

impl Widget for Container {
    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children  // Return slice of owned vec
    }
}
```

**Solution 3**: Use indices instead of references

```rust
struct Container {
    children: Vec<Box<dyn Widget>>,
    current_child_idx: Option<usize>,  // Store index, not reference
}

impl Container {
    fn current_child(&self) -> Option<&dyn Widget> {
        let idx = self.current_child_idx?;
        self.children.get(idx).map(|b| &**b)
    }
}
```

---

### Scenario 4: Async and Lifetimes

**Problem**: Can't hold references across await points.

```rust
async fn render(ctx: &GraphicsContext) {
    let buffer = ctx.create_buffer(&desc);

    // ❌ ERROR: Can't hold reference to ctx across await
    some_async_operation().await;

    ctx.write_buffer(&buffer, data);
}
```

**Why this fails**:
- Async functions become state machines
- Each await point is a yield point
- State machine must be Send (can move between threads)
- References aren't Send across threads (need 'static or Arc)

**Solution 1**: Clone Arc before await

```rust
async fn render(ctx: Arc<GraphicsContext>) {
    let buffer = ctx.create_buffer(&desc);

    some_async_operation().await;  // OK: ctx is owned (Arc)

    ctx.write_buffer(&buffer, data);
}
```

**Solution 2**: Complete borrow before await

```rust
async fn render(ctx: &GraphicsContext) {
    let buffer = ctx.create_buffer(&desc);
    drop(buffer);  // Drop reference before await

    some_async_operation().await;

    let buffer = ctx.create_buffer(&desc);  // Create again
    ctx.write_buffer(&buffer, data);
}
```

**Solution 3**: Don't await inside borrow scope

```rust
async fn render(ctx: &GraphicsContext) {
    // Synchronous work
    let buffer = ctx.create_buffer(&desc);
    ctx.write_buffer(&buffer, data);

    // Then async work
    some_async_operation().await;
}
```

---

## Lifetime Elision Rules

Understanding these helps read function signatures:

### Rule 1: Each reference gets a lifetime

```rust
// Written:
fn foo(x: &i32) -> &i32

// Compiler sees:
fn foo<'a>(x: &'a i32) -> &'a i32
//         ^^           ^^
//         Input lifetime is output lifetime
```

### Rule 2: Multiple inputs get different lifetimes

```rust
// Written:
fn foo(x: &i32, y: &i32) -> &i32

// Compiler sees:
fn foo<'a, 'b>(x: &'a i32, y: &'b i32) -> &??? i32
//                                         ^^^
// ERROR: Ambiguous which lifetime to return
```

### Rule 3: &self lifetime is used for output

```rust
// Written:
impl Foo {
    fn get(&self) -> &Bar
}

// Compiler sees:
impl Foo {
    fn get<'a>(&'a self) -> &'a Bar
}
// Output lifetime tied to &self
```

### Common Mistakes

```rust
// ❌ BAD: Returns reference to local
fn bad() -> &str {
    let s = String::from("hello");
    &s  // ERROR: s dropped at end of function
}

// ✅ GOOD: Return owned data
fn good() -> String {
    String::from("hello")
}

// ❌ BAD: Ambiguous lifetime
fn bad(x: &str, y: &str) -> &str {
    if condition {
        x  // Lifetime 'a
    } else {
        y  // Lifetime 'b
    }
    // ERROR: Return type could be either 'a or 'b
}

// ✅ GOOD: Explicit lifetime says "output lives as long as both inputs"
fn good<'a>(x: &'a str, y: &'a str) -> &'a str {
    if condition {
        x
    } else {
        y
    }
}
```

---

## Debugging Borrow Errors

### Strategy 1: Read the Error Carefully

```rust
let mut vec = vec![1, 2, 3];
let first = &vec[0];
vec.push(4);
println!("{}", first);

// Error message:
// error[E0502]: cannot borrow `vec` as mutable because it is also borrowed as immutable
//  --> src/main.rs:3:1
//   |
// 2 | let first = &vec[0];
//   |              --- immutable borrow occurs here
// 3 | vec.push(4);
//   | ^^^^^^^^^^^ mutable borrow occurs here
// 4 | println!("{}", first);
//   |                ----- immutable borrow later used here
```

**Fix**: Drop immutable borrow before mutable borrow

```rust
let mut vec = vec![1, 2, 3];
let first_value = vec[0];  // Copy value, don't hold reference
vec.push(4);
println!("{}", first_value);
```

### Strategy 2: Identify the Borrow Scope

```rust
fn process(storage: &mut WidgetStorage) {
    let widget = storage.get_mut(handle);  // Borrow starts

    // storage borrowed here
    // let other = storage.get_mut(other_handle);  // ERROR

    widget.update();
}  // Borrow ends here
```

**Fix**: Explicitly end borrow

```rust
fn process(storage: &mut WidgetStorage) {
    {
        let widget = storage.get_mut(handle);
        widget.update();
    }  // widget dropped, borrow ends

    // Can borrow again
    let other = storage.get_mut(other_handle);  // OK
}
```

### Strategy 3: Use Non-Lexical Lifetimes (NLL)

Modern Rust (2018+) has smart scoping:

```rust
let mut vec = vec![1, 2, 3];
let first = &vec[0];

if *first > 0 {
    println!("{}", first);
}  // first not used after this

vec.push(4);  // OK: borrow ended
```

### Strategy 4: Clone When Needed

Sometimes cloning is the clearest solution:

```rust
// Complicated borrow management
fn get_text(widget: &Widget) -> &str {
    widget.as_text_widget()?.text()
}
// Caller must keep widget borrowed

// vs.

// Simple owned data
fn get_text(widget: &Widget) -> String {
    widget.as_text_widget()?.text().to_string()
}
// Caller free to do anything with widget
```

### Strategy 5: Use Indices Instead of References

```rust
// ❌ Borrow conflicts
let children = parent.children_mut();
let first = &mut children[0];
let second = &mut children[1];  // ERROR: children borrowed twice

// ✅ Use indices
let children = parent.children_mut();
children[0].update();
children[1].update();
// Or use split_at_mut for simultaneous access
```

---

## Summary

### Key Principles

1. **Ownership**: Each value has exactly one owner
2. **Borrowing**: Can have many `&T` OR one `&mut T`, not both
3. **Lifetimes**: References must not outlive what they reference
4. **Move semantics**: Values move by default unless Copy

### Common Solutions

| Problem | Solution |
|---------|----------|
| Need shared ownership | `Arc<T>` |
| Need interior mutability | `Mutex<T>` or `RefCell<T>` |
| Need heterogeneous collection | `Vec<Box<dyn Trait>>` |
| Need Copy handles | Generational IDs with `PhantomData` |
| Can't hold ref across await | Clone Arc before await |
| Self-referential struct | Use owned data or external storage |
| Multiple mutable borrows | `split_at_mut()` or indices |

### Red Flags

- ❌ Returning `&` to local variable
- ❌ Storing `&` in struct without lifetime parameter
- ❌ Holding mutex lock across await
- ❌ Multiple `&mut` to same data
- ❌ `&` and `&mut` to same data simultaneously

### Green Lights

- ✅ Returning owned data (String, Vec, Arc)
- ✅ Taking `&self` and returning owned data
- ✅ Using `Arc` for shared ownership
- ✅ Cloning when borrow checker complains
- ✅ Splitting borrows with `split_at_mut()`
