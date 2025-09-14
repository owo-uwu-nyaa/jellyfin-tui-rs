use std::{
    ptr::{NonNull, null_mut},
    sync::atomic::{AtomicPtr, Ordering::SeqCst},
};

use parking_lot::Mutex;
use tokio::{sync::mpsc::UnboundedSender, task::JoinSet};
use tracing::{Instrument, Span, warn};

type Sender = UnboundedSender<Box<dyn FnOnce(&mut JoinSet<()>)>>;

static SENDER: AtomicPtr<Sender> = AtomicPtr::new(null_mut());

struct HazardEntry {
    next: AtomicPtr<HazardEntry>,
    current: AtomicPtr<Sender>,
}

static HAZARD_LIST: AtomicPtr<HazardEntry> = AtomicPtr::new(null_mut());

struct SenderPtr {
    inner: NonNull<Sender>,
}
unsafe impl Send for SenderPtr {}

static OLD_SENDERS: Mutex<Vec<SenderPtr>> = Mutex::new(Vec::new());

pub struct Spawner {
    sender: NonNull<Sender>,
    entry: NonNull<HazardEntry>,
}

unsafe impl Send for Spawner {}
unsafe impl Sync for Spawner {}

pub fn read_spawner() -> Spawner {
    let mut ptr = NonNull::new(SENDER.load(SeqCst)).expect("No current sender");
    let mut hazard_list = &HAZARD_LIST;
    let entry = loop {
        if let Some(list) = unsafe { hazard_list.load(SeqCst).as_ref() } {
            match list
                .current
                .compare_exchange(null_mut(), ptr.as_ptr(), SeqCst, SeqCst)
            {
                Ok(_) => break NonNull::from_ref(list),
                Err(_) => {
                    hazard_list = &list.next;
                }
            }
        } else {
            let new_list = Box::new(HazardEntry {
                next: AtomicPtr::new(null_mut()),
                current: AtomicPtr::new(ptr.as_ptr()),
            });
            let new_list = unsafe { NonNull::new_unchecked(Box::into_raw(new_list)) };
            loop {
                match hazard_list.compare_exchange_weak(
                    null_mut(),
                    new_list.as_ptr(),
                    SeqCst,
                    SeqCst,
                ) {
                    Ok(_) => break,
                    Err(list_ptr) => {
                        if let Some(list) = unsafe { list_ptr.as_ref() } {
                            hazard_list = &list.next;
                        }
                    }
                }
            }
            break new_list;
        }
    };
    loop {
        let new_ptr = SENDER.load(SeqCst);
        if new_ptr == ptr.as_ptr() {
            break;
        } else {
            ptr = NonNull::new(new_ptr).expect("No current sender");
            unsafe { &entry.as_ref().current }.store(ptr.as_ptr(), SeqCst);
        }
    }
    Spawner { sender: ptr, entry }
}

impl Spawner {
    pub fn spaen_bare(&self, fut: impl Future<Output = ()> + Send + 'static) {
        let _ = unsafe { self.sender.as_ref() }.send(Box::new(move |join_set| {
            join_set.spawn(fut);
        }));
    }
    pub fn spawn(&self, fut: impl Future<Output = ()> + Send + 'static, span: Span) {
        self.spaen_bare(fut.instrument(span));
    }
    pub fn spawn_res<T>(
        &self,
        fut: impl Future<Output = color_eyre::Result<T>> + Send + 'static,
        span: Span,
    ) {
        self.spawn(
            async move {
                if let Err(e) = fut.await {
                    warn!("error returned from task: {e:?}")
                }
            },
            span,
        );
    }
}

pub fn spawn_bare(fut: impl Future<Output = ()> + Send + 'static) {
    read_spawner().spaen_bare(fut);
}

pub fn spawn(fut: impl Future<Output = ()> + Send + 'static, span: Span) {
    read_spawner().spawn(fut, span);
}
pub fn spawn_res<T>(fut: impl Future<Output = color_eyre::Result<T>> + Send + 'static, span: Span) {
    read_spawner().spawn_res(fut, span);
}

impl Drop for Spawner {
    fn drop(&mut self) {
        unsafe { &self.entry.as_ref().current }.store(null_mut(), SeqCst);
    }
}

pub(crate) fn set_sender(sender: Sender) {
    if !SENDER
        .swap(Box::into_raw(Box::new(sender)), SeqCst)
        .is_null()
    {
        panic!("Another sender is already active")
    }
}

fn can_remove_sender(sender_ptr: &mut SenderPtr) -> bool {
    let mut current = &HAZARD_LIST;
    while let Some(list_entry) = NonNull::new(current.load(SeqCst)) {
        let list_entry = unsafe { list_entry.as_ref() };
        if list_entry.current.load(SeqCst) == sender_ptr.inner.as_ptr() {
            return false;
        } else {
            current = &list_entry.next;
        }
    }
    true
}

pub(crate) fn remove_current_sender() {
    let sender =
        NonNull::new(SENDER.swap(null_mut(), SeqCst)).expect("Sender has already been removed");
    let mut destroy_queue = OLD_SENDERS.lock();
    destroy_queue.push(SenderPtr { inner: sender });
    for removed in destroy_queue.extract_if(.., can_remove_sender) {
        drop(unsafe { Box::from_raw(removed.inner.as_ptr()) })
    }
}
