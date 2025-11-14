use std::{
    cell::UnsafeCell,
    mem::{self, MaybeUninit},
    num::NonZero,
    ops::Deref,
    ptr::NonNull,
    sync::atomic::{
        AtomicPtr, AtomicUsize,
        Ordering::{Relaxed, SeqCst},
    },
    task::Waker,
};

#[cfg(not(any(target_arch = "x86_64")))]
const CACHE_LINE_SIZE: usize = 0;
#[cfg(target_arch = "x86_64")]
const CACHE_LINE_SIZE: usize = 64;

const CACHE_ALIGNER_SIZE: usize = if size_of::<AtomicPtr<Waker>>() < CACHE_LINE_SIZE {
    CACHE_LINE_SIZE - size_of::<AtomicPtr<Waker>>()
} else {
    0
};

#[repr(C)]
struct WakerSlot {
    slot: MaybeUninit<Waker>,
    used: bool,
}

const CACHE_ALIGNER2_SIZE: usize = if size_of::<WakerSlot>() < CACHE_LINE_SIZE {
    CACHE_LINE_SIZE - size_of::<WakerSlot>()
} else {
    0
};
type CacheAligner = [u8; CACHE_ALIGNER_SIZE];
type CacheAligner2 = [u8; CACHE_ALIGNER2_SIZE];

#[cfg_attr(target_arch = "x86_64", repr(C, align(64)))]
pub struct WakerHazardPtr {
    waker: AtomicPtr<WakerSlot>,
    _pad_1: CacheAligner,
    current: AtomicUsize,
    _pad_2: CacheAligner,
    waker_slot_1: UnsafeCell<WakerSlot>,
    _pad_3: CacheAligner2,
    waker_slot_2: UnsafeCell<WakerSlot>,
    _pad_4: CacheAligner2,
    waker_slot_3: UnsafeCell<WakerSlot>,
    _pad_5: CacheAligner2,
    drop_delay: UnsafeCell<Option<NonNull<WakerSlot>>>,
}

struct WakerDropper {
    waker: Option<NonNull<WakerSlot>>,
}

impl WakerDropper {
    #[inline(always)]
    unsafe fn new(waker: Option<NonNull<WakerSlot>>) -> Self {
        Self { waker }
    }
}

impl Drop for WakerDropper {
    #[inline(always)]
    fn drop(&mut self) {
        unsafe {
            drop_waker(self.waker);
        }
    }
}

unsafe fn drop_waker(waker: Option<NonNull<WakerSlot>>) {
    if let Some(mut waker) = waker {
        unsafe {
            let slot = waker.as_mut();
            slot.slot.assume_init_drop();
            slot.used = false;
        }
    }
}

impl Drop for WakerHazardPtr {
    fn drop(&mut self) {
        fn drop_slot(slot: &mut WakerSlot) {
            if slot.used {
                unsafe {
                    slot.slot.assume_init_drop();
                }
            }
        }
        drop_slot(self.waker_slot_1.get_mut());
        drop_slot(self.waker_slot_2.get_mut());
        drop_slot(self.waker_slot_3.get_mut());
    }
}

unsafe impl Send for WakerHazardPtr {}
unsafe impl Sync for WakerHazardPtr {}

impl Default for WakerHazardPtr {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}

pub struct WakerGuard<'s> {
    current: &'s AtomicUsize,
    waker: &'s Waker,
}

impl<'s> Deref for WakerGuard<'s> {
    type Target = Waker;

    fn deref(&self) -> &Self::Target {
        self.waker
    }
}

impl<'s> Drop for WakerGuard<'s> {
    fn drop(&mut self) {
        self.current.store(0, SeqCst);
    }
}

impl WakerHazardPtr {
    pub unsafe fn waker(&self) -> Option<WakerGuard<'_>> {
        let mut waker = self.waker.load(SeqCst);
        loop {
            let current = waker.addr();
            self.current.store(current, SeqCst);
            waker = self.waker.load(SeqCst);
            if current == waker.addr() {
                break NonNull::new(waker).map(|w| WakerGuard {
                    current: &self.current,
                    waker: unsafe { w.as_ref().slot.assume_init_ref() },
                });
            } else {
            }
        }
    }
    /**
     * set a new waker
     * # SAFETY
     * Must always be called from the same thread
     * If another thread needs to replace the waker, this handoff must establish a happens before relationship
     *   */
    pub unsafe fn replace_waker(&self, new_waker: &Waker) {
        //this can be relaxed because it was either written by the current thread or the move to another thread did the synchronization
        let old_waker_ptr = self.waker.load(Relaxed);
        if let Some(c_waker) = unsafe { old_waker_ptr.as_ref() }
            && unsafe { c_waker.slot.assume_init_ref().will_wake(new_waker) }
        {
            //nothing to do
        } else {
            let new_waker_ptr = if unsafe { !(&*self.waker_slot_1.get()).used } {
                self.waker_slot_1.get()
            } else if unsafe { !(&*self.waker_slot_2.get()).used } {
                self.waker_slot_2.get()
            } else if unsafe { !(&*self.waker_slot_3.get()).used } {
                self.waker_slot_3.get()
            } else {
                panic!("All slots are currently in use. Memory leak detected")
            };
            unsafe {
                let slot = &mut *new_waker_ptr;
                slot.slot.write(new_waker.clone());
                slot.used = true;
            };
            self.waker.store(new_waker_ptr, SeqCst);
            let old_waker = NonNull::new(old_waker_ptr);
            let drop_delay = unsafe { &mut *self.drop_delay.get() };
            let current = NonZero::new(self.current.load(SeqCst));

            if let Some(current) = current {
                if let Some(drop_delay_filled) = drop_delay {
                    if current.get() == drop_delay_filled.as_ptr().addr() {
                        //old waker is already released
                        unsafe {
                            drop_waker(old_waker);
                        }
                    } else if current.get() == old_waker_ptr.addr() {
                        unsafe {
                            drop_waker(mem::replace(drop_delay, old_waker));
                        }
                    } else {
                        // current is invalid
                        unsafe {
                            let _drop = WakerDropper::new(old_waker);
                            drop_waker(drop_delay.take());
                        }
                    }
                } else if current.get() == old_waker_ptr.addr() {
                    *drop_delay = old_waker;
                } else {
                    unsafe {
                        // current is invalid
                        drop_waker(old_waker);
                    }
                }
            } else {
                unsafe {
                    //this ensures old_waker is dropped if dropping drop_delay panics
                    let _drop = WakerDropper::new(old_waker);
                    drop_waker(drop_delay.take());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, atomic::AtomicBool},
        task::{Wake, Waker},
        thread::JoinHandle,
    };

    use crate::hazard::WakerHazardPtr;

    #[derive(Debug, Default)]
    struct DebugWaker {
        flag: AtomicBool,
    }
    impl Wake for DebugWaker {
        fn wake(self: Arc<Self>) {
            self.flag.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    fn wake_loop(ptr: Arc<WakerHazardPtr>) -> JoinHandle<()> {
        std::thread::spawn(move || {
            for _ in 0..1024 {
                if let Some(waker) = unsafe { ptr.waker() } {
                    waker.wake_by_ref();
                }
            }
        })
    }

    #[test]
    fn test_replace() {
        let ptr: Arc<WakerHazardPtr> = Arc::default();
        let waker1: Arc<DebugWaker> = Arc::default();
        let waker2: Arc<DebugWaker> = Arc::default();
        let waker1_w = Waker::from(waker1.clone());
        let waker2_w = Waker::from(waker2.clone());
        let wake_handle = wake_loop(ptr.clone());
        for _ in 0..64 {
            unsafe {
                ptr.replace_waker(&waker1_w);
            }
            unsafe {
                ptr.replace_waker(&waker2_w);
            }
        }
        wake_handle.join().expect("wake should not panic");
        drop(waker1_w);
        drop(waker2_w);
        drop(ptr);
        assert_eq!(1, Arc::strong_count(&waker1));
        assert_eq!(1, Arc::strong_count(&waker2));
    }
}
