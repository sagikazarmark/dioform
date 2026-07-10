# Keep the adapter runtime facade over adapter state

Dioform will keep `AdapterRuntime` as a borrow-hiding facade over `Rc<RefCell<AdapterState>>` rather than
flattening its roughly twenty-five forwarding methods by exposing the inner state directly. The forwarders
(`is_active`, `set_parse_error`, `has_validation_tasks`, and the rest) look shallow (each is one
`self.state.borrow().foo()`), but deleting them moves `RefCell` borrow discipline into `FormHandle` at every
call site, where a stray borrow held across a notify or dispatch would panic. The facade concentrates that
discipline in one file, so removing it relocates complexity without concentrating it and is not a real seam.
`spawn_validation_task` must stay at the facade level regardless, because the completion future needs an
`Rc<RefCell<AdapterState>>` handle to re-enter state on completion, which a `&mut self` method on
`AdapterState` cannot manufacture. Merging the spawner into the state changes none of this.

What Dioform will do instead is lift adapter-owned **Raw Input State** (**Parse Errors** and **File
Selection**) out of `AdapterState` into their own `ParseBindings` and `FileSelections` types. Those methods
touch neither the task spawner, tasks, validation waiters, nor debounce state; they landed in `AdapterState`
only for a shared `RefCell` home. Splitting them removes the parse and file forwarders as a side effect and
makes the `AdapterState` name honest: it then holds only the async-validation runtime. Future reviews should
not re-suggest flattening the remaining forwarders unless the borrow discipline they hide moves elsewhere.
