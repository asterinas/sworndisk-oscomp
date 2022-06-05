use crate::prelude::*;
use kernel::rbtree::RBTree;

pub struct LruEntry<K, V>
where
    K: Ord + Copy + Debug,
    V: Debug,
{
    key: mem::MaybeUninit<K>,
    val: mem::MaybeUninit<V>,
    prev: *mut LruEntry<K, V>,
    next: *mut LruEntry<K, V>,
}

impl<K: Ord + Copy + Debug, V: Debug> Debug for LruEntry<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LruEntry")
            .field("key", &self.key)
            .field("value", &self.val)
            .finish()
    }
}

impl<K: Ord + Copy + Debug, V: Debug> LruEntry<K, V> {
    fn new(key: K, val: V) -> Self {
        LruEntry {
            key: mem::MaybeUninit::new(key),
            val: mem::MaybeUninit::new(val),
            prev: ptr::null_mut(),
            next: ptr::null_mut(),
        }
    }

    fn new_uninit() -> Self {
        LruEntry {
            key: mem::MaybeUninit::uninit(),
            val: mem::MaybeUninit::uninit(),
            prev: ptr::null_mut(),
            next: ptr::null_mut(),
        }
    }
}

pub struct LruCache<K, V>
where
    K: Ord + Copy + Debug,
    V: Debug,
{
    map: RBTree<K, Box<LruEntry<K, V>>>,
    len: usize,
    capacity: usize,
    head: *mut LruEntry<K, V>,
    tail: *mut LruEntry<K, V>,
}

impl<K: Ord + Copy + Debug, V: Debug> Debug for LruCache<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LruCache")
            .field("len", &self.len)
            .field("capacity", &self.capacity)
            .finish()
    }
}

impl<K: Ord + Copy + Debug, V: Debug> LruCache<K, V> {
    pub fn new(capacity: usize) -> Result<Self> {
        let cache = LruCache {
            capacity,
            len: 0,
            map: RBTree::new(),

            // sentinel node
            head: Box::into_raw(Box::try_new(LruEntry::new_uninit())?),
            tail: Box::into_raw(Box::try_new(LruEntry::new_uninit())?),
        };

        // SAFETY: Safe. `cache.head` and `cache.tail` is non-null.
        unsafe {
            (*cache.head).next = cache.tail;
            (*cache.tail).prev = cache.head;
        };

        Ok(cache)
    }

    pub fn put(&mut self, key: K, val: V) -> Result<Option<V>> {
        Ok(self.do_put(key, val)?.map(|(_, v)| v))
    }

    pub fn pop(&mut self, key: &K) -> Option<V> {
        match self.map.remove(key) {
            None => None,
            Some(mut old_node) => {
                unsafe {
                    ptr::drop_in_place(old_node.key.as_mut_ptr());
                };
                let node_ptr: *mut LruEntry<K, V> = &mut *old_node;
                self.detach(node_ptr);
                unsafe { Some(old_node.val.assume_init()) }
            }
        }
    }

    pub fn get<'a>(&'a mut self, k: &K) -> Option<&'a V> {
        if let Some(node) = self.map.get_mut(k) {
            let node_ptr: *mut LruEntry<K, V> = &mut **node;
            self.detach(node_ptr);
            self.attach(node_ptr);
            Some(unsafe { &(*(*node_ptr).val.as_ptr()) as &V })
        } else {
            None
        }
    }

    pub fn get_mut<'a>(&'a mut self, k: &K) -> Option<&'a mut V> {
        if let Some(node) = self.map.get_mut(k) {
            let node_ptr: *mut LruEntry<K, V> = &mut **node;
            self.detach(node_ptr);
            self.attach(node_ptr);
            Some(unsafe { &mut (*(*node_ptr).val.as_mut_ptr()) as &mut V })
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn cap(&self) -> usize {
        self.capacity
    }

    fn do_put(&mut self, key: K, mut val: V) -> Result<Option<(K, V)>> {
        let node_ref = self.map.get_mut(&key);
        match node_ref {
            Some(node_ref) => {
                let node_ptr: *mut LruEntry<K, V> = &mut **node_ref;

                // if the key is already in the cache, update its value and move to front.
                // SAFETY: Safe. `v` and `node_ptr.val` is valid and non-null
                unsafe { mem::swap(&mut val, &mut (*(*node_ptr).val.as_mut_ptr()) as &mut V) };

                self.detach(node_ptr);
                self.attach(node_ptr);

                Ok(Some((key, val)))
            }
            None => {
                let (replaced, mut node) = self.replace_or_create_node(key, val)?;
                let node_ptr: *mut LruEntry<K, V> = &mut *node;
                self.attach(node_ptr);
                self.len += 1;
                self.map.try_insert(key, node)?;

                Ok(replaced)
            }
        }
    }

    fn replace_or_create_node(
        &mut self,
        key: K,
        val: V,
    ) -> Result<(Option<(K, V)>, Box<LruEntry<K, V>>)> {
        if self.len() == self.cap() {
            let old_key = unsafe { &*(*(*self.tail).prev).key.as_ptr() };
            let mut old_node = self.map.remove(old_key).unwrap();

            // SAFETY: take the value in MaybeUninit
            let replaced = unsafe { (old_node.key.assume_init(), old_node.val.assume_init()) };

            old_node.key = mem::MaybeUninit::new(key);
            old_node.val = mem::MaybeUninit::new(val);
            let node_ptr: *mut LruEntry<K, V> = &mut *old_node;
            self.detach(node_ptr);

            Ok((Some(replaced), old_node))
        } else {
            Ok((None, Box::try_new(LruEntry::new(key, val))?))
        }
    }

    fn detach(&mut self, node: *mut LruEntry<K, V>) {
        unsafe {
            (*(*node).prev).next = (*node).next;
            (*(*node).next).prev = (*node).prev;
        }
    }

    fn attach(&mut self, node: *mut LruEntry<K, V>) {
        unsafe {
            (*node).next = (*self.head).next;
            (*node).prev = self.head;
            (*self.head).next = node;
            (*(*node).next).prev = node;
        }
    }
}
