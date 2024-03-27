use std::cell::RefCell;

use crate::MockContext;

thread_local!(static CONTEXT: RefCell<Option<MockContext>> = const { RefCell::new(None) });

/// Inject the given context to be used in the current thread.
#[inline]
pub fn inject(ctx: MockContext) {
    CONTEXT.with(|f| {
        *f.borrow_mut() = Some(ctx);
    });
}

/// Return the mutable reference to the context of the current thread.
#[inline]
#[allow(clippy::transmute_ptr_to_ref)]
pub fn get_context() -> &'static mut MockContext {
    CONTEXT.with(|cell| {
        let borrow = cell.borrow();
        let ctx = borrow.as_ref().expect(
            "Can not find the context for thread.\
            Are you using .inject() on the MockContext?",
        );
        unsafe {
            let const_ptr = ctx as *const MockContext;
            let mut_ptr = const_ptr as *mut MockContext;
            ::std::mem::transmute::<*mut MockContext, &mut MockContext>(mut_ptr)
        }
    })
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::get_context;
    use crate::{Context, MockContext};

    #[test]
    fn separate_context_for_thread() {
        let mut handles = Vec::new();

        for i in 0..100 {
            let handle = thread::spawn(move || {
                let id = i * 10000;
                let ctx = MockContext::new()
                    .with_balance(id)
                    .with_msg_cycles(5000)
                    .inject();

                assert_eq!(ctx.balance(), id);

                for j in 1..500 {
                    assert_eq!(ctx.balance(), id + j - 1);
                    ctx.msg_cycles_accept(1);
                    assert_eq!(ctx.balance(), id + j);
                    // Get it again.
                    let ctx2 = get_context();
                    assert_eq!(ctx2.balance(), id + j);
                }
            });

            handles.push(handle)
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }
}
