# TODO

- [x] **Verify Component Scanning**: Ensure `scan_components` in `framework/src/components.rs` correctly identifies new Python components.
- [x] **Update `file_watcher`**: Review `framework/src/actors/file_watcher.rs` to confirm it monitors for `.py` file changes and triggers a component scan.
- [x] **Ensure `template_renderer` Propagates Changes**: Check that `framework/src/actors/template_renderer.rs` correctly updates and propagates the component list.
- [x] **Confirm Request Handler Logic**: Verify that the request handler can identify the new component and dispatch an `ExecuteFunction` message to the interpreter.