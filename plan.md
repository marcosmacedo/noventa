# Plan to Fix Template Reloading Issue

The current implementation of `FileWatcherActor` in [`file_watcher.rs`](framework/src/actors/file_watcher.rs:1) uses `futures::executor::block_on`, which blocks the asynchronous runtime and prevents file change events from being processed correctly. To fix this, we need to refactor the actor to handle these operations asynchronously.

## 1. Create a `messages.rs` File

First, we'll create a new file, `framework/src/actors/messages.rs`, to define the messages used for communication between actors. This will help decouple the actors and make the system more modular and easier to maintain.

## 2. Refactor `file_watcher.rs`

Next, we'll refactor [`file_watcher.rs`](framework/src/actors/file_watcher.rs:1) to remove the blocking calls and handle file change events asynchronously. This will involve the following changes:

- **Introduce `Watch` and `FileChanged` Messages**: We'll define two new messages, `Watch` and `FileChanged`, to handle file watching and change events in a non-blocking way.
- **Implement Asynchronous Handlers**: We'll implement handlers for these messages to process file changes without blocking the runtime.
- **Update Actor Implementation**: We'll modify the `FileWatcherActor` to use these new messages and handlers, ensuring that all operations are performed asynchronously.

## 3. Update `ws_server.rs`

Finally, we'll update `framework/src/actors/ws_server.rs` to handle the `FileChangeEvent` message and broadcast a reload message to all connected clients. This will ensure that the UI is automatically updated when a file change is detected.